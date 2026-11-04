//! Cached/dynamic no-context collision queries and explicit contextual block rules.
//! Context-sensitive families override the no-context cache explicitly.
use crate::{
    block_collision_scan::CollisionBlock,
    collision::CachedCollisionRegistry,
    collision_context::CollisionContext,
    dynamic_support::{DynamicSupport, DynamicSupportError, DynamicSupportWorld},
    liquid_collision::{LiquidCollision, LiquidCollisionWorld},
    piston_shape::PistonBlockBindings,
    powder_snow_collision::{PowderSnowWorld, powder_snow_collision_shape},
    registry::{BlockRegistry, BlockStateId},
};
use horizon_core::geometry::{Aabb, Axis, Direction, Vec3, VoxelShape};
use std::{collections::HashMap, io, sync::Arc};

#[derive(Clone, Copy, Debug)]
pub struct ShulkerCollisionState {
    pub previous_progress: f32,
    pub progress: f32,
}
impl ShulkerCollisionState {
    pub fn bounding_box(self, direction: Direction) -> Aabb {
        // Native getProgress(1) retains this interpolation, including rounding.
        let progress = 0.5f32 * (self.previous_progress + (self.progress - self.previous_progress));
        let maximum = if progress.is_nan() {
            progress
        } else {
            (-1.0f32).max(progress)
        } as f64;
        let minimum = if progress.is_nan() {
            progress
        } else {
            (-1.0f32).min(progress)
        } as f64;
        let [x, y, z] = direction.normal();
        Aabb::new(Vec3::new(-0.5, 0.0, -0.5), Vec3::new(0.5, 1.0, 0.5))
            .expand_towards(Vec3::new(
                f64::from(x) * maximum,
                f64::from(y) * maximum,
                f64::from(z) * maximum,
            ))
            .contract(Vec3::new(
                f64::from(-x) * (1.0 + minimum),
                f64::from(-y) * (1.0 + minimum),
                f64::from(-z) * (1.0 + minimum),
            ))
            .move_by(Vec3::new(0.5, 0.0, 0.5))
    }
}

pub trait DynamicCollisionWorld: DynamicSupportWorld {
    /// One block-entity lookup. None denotes absence or another entity type.
    fn shulker_collision_state(
        &mut self,
        position: [i32; 3],
    ) -> Result<Option<ShulkerCollisionState>, Self::Error>;
}

#[derive(Debug)]
pub enum CollisionQueryError<E> {
    Data(io::Error),
    Dynamic(DynamicSupportError<E>),
    Context(E),
    Unsupported(BlockStateId),
}
impl<E: std::fmt::Display> std::fmt::Display for CollisionQueryError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Data(error) => error.fmt(f),
            Self::Dynamic(error) => error.fmt(f),
            Self::Context(error) => error.fmt(f),
            Self::Unsupported(state) => write!(f, "unsupported block collision state {}", state.0),
        }
    }
}
impl<E: std::error::Error + 'static> std::error::Error for CollisionQueryError<E> {}

pub struct BlockCollisionRegistry<'r> {
    cached: Arc<CachedCollisionRegistry<'r>>,
    dynamic: HashMap<BlockStateId, DynamicSupport>,
    contextual: HashMap<BlockStateId, ContextualRule>,
    /// States with neither a cached shape nor a rule (mod blocks only): a full cube.
    fallback: HashMap<BlockStateId, VoxelShape>,
    scan_flags: Vec<u8>,
    /// Piston and piston-head states for moving-piston collision; None in
    /// registries without pistons.
    pistons: Option<PistonBlockBindings<'r>>,
}
enum ContextualRule {
    Scaffolding(ScaffoldingCollision),
    Liquid(LiquidCollision),
    PowderSnow,
}
impl<'r> BlockCollisionRegistry<'r> {
    pub fn new(cached: Arc<CachedCollisionRegistry<'r>>) -> io::Result<Self> {
        let mut dynamic = HashMap::new();
        let mut contextual = HashMap::new();
        let mut fallback = HashMap::new();
        let mut scan_flags = Vec::with_capacity(cached.registry().len());
        for index in 0..cached.registry().len() {
            let id = BlockStateId(index as u32);
            let named = cached.registry().state(id).unwrap();
            let cached_shape = cached.cached_shape(id)?;
            let large = cached_shape.is_none_or(|shape| {
                Axis::ALL
                    .into_iter()
                    .any(|axis| shape.min(axis) < 0.0 || shape.max(axis) > 1.0)
            });
            // A mod block copying a vanilla template (a content root's) follows
            // the template's rule: Create's scaffoldings are vanilla scaffolding
            // (`ScaffoldingBlock` subclasses). Its own properties may not fit the
            // rule: then it has none, rather than the registry failing.
            let template = cached
                .template(&named.name)
                .map(|template| crate::chunk::BlockState {
                    name: template.to_owned(),
                    properties: named.properties.clone(),
                });
            let (state, lenient) = match &template {
                Some(state) => (state, true),
                None => (named, false),
            };
            let rule = |result: io::Result<Option<ContextualRule>>| match result {
                Err(_) if lenient => Ok(None),
                other => other,
            };
            if let Some(rule) = rule(
                ScaffoldingCollision::from_state(state).map(|r| r.map(ContextualRule::Scaffolding)),
            )?
            .or(rule(
                LiquidCollision::from_state(state).map(|r| r.map(ContextualRule::Liquid)),
            )?)
            .or((state.name == "minecraft:powder_snow").then_some(ContextualRule::PowderSnow))
            {
                contextual.insert(id, rule);
            }
            if cached_shape.is_none() {
                match DynamicSupport::from_state(state) {
                    Ok(Some(rule)) => {
                        dynamic.insert(id, rule);
                    }
                    Err(_) if lenient => {}
                    Err(error) => return Err(error),
                    Ok(None) => {}
                }
            }
            // No cached shape and no rule: a mod block whose shape depends on
            // context in a way Horizon doesn't know. A full cube, never a stop
            // (players walk on it and can't walk through it).
            if cached_shape.is_none() && !dynamic.contains_key(&id) {
                fallback.insert(id, VoxelShape::block());
            }
            // No contextual rule and an empty cached shape: empty everywhere.
            let empty =
                !contextual.contains_key(&id) && cached_shape.is_some_and(VoxelShape::is_empty);
            scan_flags.push(
                u8::from(large)
                    | (u8::from(state.name == "minecraft:moving_piston") << 1)
                    | (u8::from(empty) << 2)
                    | (u8::from(contextual.contains_key(&id)) << 3),
            );
        }
        if !fallback.is_empty() {
            let mut names: Vec<&str> = fallback
                .keys()
                .filter_map(|id| cached.registry().state(*id))
                .map(|state| state.name.as_str())
                .collect();
            names.sort_unstable();
            names.dedup();
            // Once a process: the client, the server and worldgen each build a registry.
            static LOGGED: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
            let message = format!(
                "{} block states have no collision rule; they collide as full blocks: {}",
                fallback.len(),
                names.join(", ")
            );
            let mut logged = LOGGED.lock().unwrap_or_else(|e| e.into_inner());
            if !logged.contains(&message) {
                log::warn!("Horizon: {message}");
                logged.push(message);
            }
        }
        let pistons = PistonBlockBindings::new(cached.registry()).ok();
        Ok(Self {
            cached,
            dynamic,
            contextual,
            fallback,
            scan_flags,
            pistons,
        })
    }
    pub fn registry(&self) -> &'r BlockRegistry {
        self.cached.registry()
    }
    /// The piston states behind `PistonMovingBlockEntity.getCollisionShape`.
    pub fn pistons(&self) -> Option<&PistonBlockBindings<'r>> {
        self.pistons.as_ref()
    }
    pub fn cached_shapes(&self) -> &CachedCollisionRegistry<'r> {
        &self.cached
    }
    /// Immutable cursor-filter metadata, prepared once for this registry.
    pub fn collision_block(&self, state: BlockStateId) -> io::Result<CollisionBlock> {
        let flags = *self
            .scan_flags
            .get(state.0 as usize)
            .ok_or_else(|| crate::compression::invalid("collision scan state outside registry"))?;
        Ok(CollisionBlock {
            state,
            large_collision_shape: flags & 1 != 0,
            moving_piston: flags & 2 != 0,
            empty_shape: flags & 4 != 0,
        })
    }
    /// Whether `state` can have a collision shape: false for states whose shape is always
    /// empty, whatever the context. States outside the registry count.
    pub fn may_collide(&self, state: BlockStateId) -> bool {
        self.scan_flags
            .get(state.0 as usize)
            .is_none_or(|flags| flags & 4 == 0)
    }
    /// Whether a scan keeps `state` in the cells around its box: a large
    /// collision shape or a moving piston. States outside the registry count.
    pub fn reaches_out(&self, state: BlockStateId) -> bool {
        self.scan_flags
            .get(state.0 as usize)
            .is_none_or(|flags| flags & 3 != 0)
    }
    pub fn collision_shape<W: DynamicCollisionWorld>(
        &self,
        world: &mut W,
        state: BlockStateId,
        position: [i32; 3],
    ) -> Result<VoxelShape, CollisionQueryError<W::Error>> {
        if let Some(shape) = self
            .cached
            .cached_shape(state)
            .map_err(CollisionQueryError::Data)?
        {
            return Ok(shape.clone());
        }
        if let Some(rule) = self.dynamic.get(&state) {
            return rule
                .collision_shape(world, position)
                .map_err(CollisionQueryError::Dynamic);
        }
        self.fallback
            .get(&state)
            .cloned()
            .ok_or(CollisionQueryError::Unsupported(state))
    }

    /// Ordinary entity/position/placement contexts. The remaining pinned block
    /// families share their no-context geometry; the native registry audit checks
    /// cached states with rejecting context/world providers before using that path.
    /// Experimental minecart position exclusions are a separate context layer.
    pub fn collision_shape_with_context<W, E>(
        &self,
        world: &mut W,
        state: BlockStateId,
        position: horizon_core::position::BlockPos,
        context: &CollisionContext<'_>,
    ) -> Result<VoxelShape, CollisionQueryError<E>>
    where
        W: DynamicCollisionWorld<Error = E>
            + LiquidCollisionWorld<Error = E>
            + PowderSnowWorld<Error = E>,
    {
        // Most states have no contextual rule: their flag spares the lookup.
        let contextual = self
            .scan_flags
            .get(state.0 as usize)
            .is_none_or(|flags| flags & 8 != 0);
        match contextual.then(|| self.contextual.get(&state)).flatten() {
            Some(ContextualRule::Scaffolding(rule)) => Ok(rule.shape(context, position)),
            Some(ContextualRule::Liquid(rule)) => rule
                .shape(world, position, context)
                .map_err(CollisionQueryError::Context),
            Some(ContextualRule::PowderSnow) => {
                powder_snow_collision_shape(world, position, context)
                    .map_err(CollisionQueryError::Context)
            }
            None => self.collision_shape(world, state, [position.x, position.y, position.z]),
        }
    }
}

/// Entity-context scaffold selection, independent of world queries. Construction
/// validates the descriptor so missing properties cannot silently change behavior.
#[derive(Clone, Copy, Debug)]
pub struct ScaffoldingCollision {
    supported: bool,
    bottom: bool,
}
impl ScaffoldingCollision {
    pub fn from_state(state: &crate::chunk::BlockState) -> io::Result<Option<Self>> {
        if state.name != "minecraft:scaffolding" {
            return Ok(None);
        }
        let distance = state
            .properties
            .get("distance")
            .and_then(|v| crate::registry::java_integer(v))
            .filter(|v| (0..=7).contains(v))
            .ok_or_else(|| crate::compression::invalid("invalid scaffold distance"))?;
        let bottom = match state.properties.get("bottom").map(String::as_str) {
            Some("true") => true,
            Some("false") => false,
            _ => return Err(crate::compression::invalid("invalid scaffold bottom")),
        };
        Ok(Some(Self {
            supported: distance == 0,
            bottom,
        }))
    }

    pub fn shape(
        &self,
        context: &crate::collision_context::CollisionContext<'_>,
        position: horizon_core::position::BlockPos,
    ) -> VoxelShape {
        use std::sync::OnceLock;
        if context.is_placement() {
            return VoxelShape::empty();
        }
        if context.is_above(&VoxelShape::block(), position, true) && !context.is_descending() {
            return crate::dynamic_support::scaffolding_stable_shape();
        }
        if !self.supported && self.bottom {
            static BELOW: OnceLock<VoxelShape> = OnceLock::new();
            let below = BELOW.get_or_init(|| {
                VoxelShape::block()
                    .moved(Vec3::new(0.0, -1.0, 0.0))
                    .optimized()
                    .expect("bounded scaffold below shape")
            });
            if context.is_above(below, position, true) {
                static BOTTOM: OnceLock<VoxelShape> = OnceLock::new();
                return BOTTOM
                    .get_or_init(|| {
                        VoxelShape::from_box(Aabb::new(Vec3::ZERO, Vec3::new(1.0, 0.125, 1.0)))
                            .expect("bounded scaffold bottom")
                    })
                    .clone();
            }
        }
        VoxelShape::empty()
    }
}
