//! The Java mod bridge's additions to the mesher: block states whose block entity draws the
//! whole block, mods' fluids drawn with their own sprites, and mods' tint sources. Set from
//! the game (game/java_entities.rs, java_mods.rs); read once per table build or section.
use super::Tint;
use crate::blocks::StateFacts;
use horizon_world::{
    fluid::{FluidKind, FluidState},
    registry::BlockStateId,
};

/// Block states whose chunk model isn't drawn because their block entity's renderer draws the
/// whole block (a Java mod's model that, in a world, has no geometry of its own: Create's shafts
/// and cogwheels, which their renderers turn). By state id; the Java mods' entities say which.
static DRAWN_BY_BLOCK_ENTITY: std::sync::LazyLock<std::sync::RwLock<std::sync::Arc<Vec<bool>>>> =
    std::sync::LazyLock::new(Default::default);

/// Marks a state's chunk model as drawn by its block entity (see [`DRAWN_BY_BLOCK_ENTITY`]):
/// true when it wasn't already (the sections holding it need meshing again).
pub fn hide_model_drawn_by_block_entity(state: BlockStateId) -> bool {
    let index = state.0 as usize;
    let mut set = DRAWN_BY_BLOCK_ENTITY
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if set.get(index).copied().unwrap_or(false) {
        return false;
    }
    let mut next = (**set).clone();
    if next.len() <= index {
        next.resize(index + 1, false);
    }
    next[index] = true;
    *set = std::sync::Arc::new(next);
    true
}

/// A Java mod's fluid as its client model draws it: still and flowing sprite names
/// (`create:fluid/honey_still`), tint (ARGB, -1 none) and whether it is translucent.
#[derive(Clone, Debug, PartialEq)]
pub struct ModFluidLook {
    pub still: String,
    pub flow: String,
    pub tint: u32,
    pub translucent: bool,
}

/// [`ModFluidLook`]s by state id: a mod's liquid block (carried with lava as its template)
/// drawn with its own sprites instead of lava's.
#[allow(clippy::type_complexity)]
static MOD_FLUIDS: std::sync::LazyLock<
    std::sync::RwLock<std::sync::Arc<std::collections::HashMap<u32, ModFluidLook>>>,
> = std::sync::LazyLock::new(Default::default);

/// Sets the mods' fluids' looks by state id; true when they changed (sections to mesh again).
pub fn set_mod_fluids(looks: std::collections::HashMap<u32, ModFluidLook>) -> bool {
    let mut set = MOD_FLUIDS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if **set == looks {
        return false;
    }
    *set = std::sync::Arc::new(looks);
    true
}

/// A mod fluid's sprite layers and colour, resolved for one section ([`ModFluidLook`]).
#[derive(Clone, Copy, Debug)]
pub(crate) struct ModFluid {
    pub still: u16,
    pub flow: u16,
    pub tint: u32,
    pub translucent: bool,
}

/// Mods' blocks' tint sources by block id (their BlockColors registrations, as the Java mod
/// bridge exports them): what `MeshTables` takes for blocks vanilla's table doesn't name.
static MOD_TINTS: std::sync::RwLock<Option<std::collections::HashMap<String, Vec<Tint>>>> =
    std::sync::RwLock::new(None);

/// Sets the mods' tint sources: `{"mod:block": ["foliage", "none", "#rrggbb", ...]}`, one a
/// tint index (`grass`, `foliage`, `dry_foliage`, `water`, `none` or a constant colour).
pub fn set_mod_tints(json: &str) -> crate::Result<usize> {
    let map: std::collections::HashMap<String, Vec<String>> = serde_json::from_str(json)?;
    let tints: std::collections::HashMap<String, Vec<Tint>> = map
        .into_iter()
        .map(|(block, layers)| {
            let layers = layers
                .iter()
                .map(|l| match l.as_str() {
                    "grass" => Tint::Grass,
                    "foliage" => Tint::Foliage,
                    "dry_foliage" => Tint::DryFoliage,
                    "water" => Tint::Water,
                    c => c
                        .strip_prefix('#')
                        .and_then(|h| u32::from_str_radix(h, 16).ok())
                        .map_or(Tint::None, Tint::Constant),
                })
                .collect();
            (block, layers)
        })
        .collect();
    let n = tints.len();
    *MOD_TINTS.write().unwrap_or_else(|e| e.into_inner()) = Some(tints);
    Ok(n)
}

/// The mods' tint sources, read for one `MeshTables` build ([`mod_tint`]).
pub(super) fn mod_tints()
-> std::sync::RwLockReadGuard<'static, Option<std::collections::HashMap<String, Vec<Tint>>>> {
    MOD_TINTS.read().unwrap_or_else(|e| e.into_inner())
}

/// The tint sources of a block vanilla's table doesn't name: a mod's, else none.
pub(super) fn mod_tint(
    mod_tints: &Option<std::collections::HashMap<String, Vec<Tint>>>,
    name: &str,
) -> ([Tint; 2], u8) {
    mod_tints
        .as_ref()
        .and_then(|m| m.get(name))
        .map_or(([Tint::None; 2], 0), |layers| {
            let mut tints = [Tint::None; 2];
            for (t, l) in tints.iter_mut().zip(layers) {
                *t = *l;
            }
            (tints, layers.len().min(2) as u8)
        })
}

/// A state's fluid, with a Java mod's liquid seen as one.
pub(super) fn mod_liquid(f: FluidState, s: &StateFacts, prop: impl Fn(&str) -> u8) -> FluidState {
    match f {
        // A Java mod's liquid (Create's honey: a LiquidBlock carried with lava as
        // its template): a fluid of lava's family at its `level`, drawn with the
        // mod's own sprites once Java's models give them (set_mod_fluids).
        f if f.kind() == FluidKind::Empty
            && !s.name.starts_with("minecraft:")
            && s.has_class("LiquidBlock") =>
        {
            match prop("level") {
                0 => FluidState::new(FluidKind::Lava, 8, false),
                l if l >= 8 => FluidState::new(FluidKind::FlowingLava, 8, true),
                l => FluidState::new(FluidKind::FlowingLava, 8 - l, false),
            }
        }
        f => f,
    }
}

/// What a section's meshing reads of the above, taken once per section.
pub(super) struct SectionJava<'a> {
    drawn_by_block_entity: std::sync::Arc<Vec<bool>>,
    mod_fluids: std::sync::Arc<std::collections::HashMap<u32, ModFluidLook>>,
    sprite_layers: &'a std::collections::BTreeMap<String, u32>,
    drawn_by_code: std::sync::Arc<Vec<bool>>,
    #[allow(clippy::type_complexity)]
    code_models: std::sync::Arc<
        std::collections::HashMap<[i32; 3], std::sync::Arc<crate::models::catalog::BakedModel>>,
    >,
}

impl<'a> SectionJava<'a> {
    pub(super) fn new(sprite_layers: &'a std::collections::BTreeMap<String, u32>) -> Self {
        Self {
            drawn_by_block_entity: DRAWN_BY_BLOCK_ENTITY
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            mod_fluids: MOD_FLUIDS
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            sprite_layers,
            drawn_by_code: DRAWN_BY_CODE
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
            code_models: CODE_MODELS
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone(),
        }
    }

    /// The model its mod's code drew the block at `pos` (state `id`) with, when its state is one
    /// drawn by code and the code has drawn it.
    #[inline]
    pub(super) fn code_model(
        &self,
        id: u32,
        pos: [i32; 3],
    ) -> Option<&crate::models::catalog::BakedModel> {
        if self.drawn_by_code.get(id as usize) != Some(&true) {
            return None;
        }
        self.code_models.get(&pos).map(|m| &**m)
    }

    /// Whether state `id`'s block entity draws its whole block (its chunk model is skipped).
    #[inline]
    pub(super) fn drawn_by_block_entity(&self, id: u32) -> bool {
        self.drawn_by_block_entity.get(id as usize) == Some(&true)
    }

    /// State `id`'s mod fluid look, when it has one whose sprites are loaded.
    #[inline]
    pub(super) fn fluid(&self, id: u32) -> Option<ModFluid> {
        if self.mod_fluids.is_empty() {
            return None;
        }
        let look = self.mod_fluids.get(&id)?;
        let layer = |name: &str| self.sprite_layers.get(name).map(|&l| l as u16);
        Some(ModFluid {
            still: layer(&look.still)?,
            flow: layer(&look.flow)?,
            tint: look.tint,
            translucent: look.translucent,
        })
    }
}

/// Block states a Java mod's code draws per position (a model made from the block entity:
/// LibMultipart's container), by state id: drawn with the model of their position
/// ([`set_code_model`]) where there is one.
static DRAWN_BY_CODE: std::sync::LazyLock<std::sync::RwLock<std::sync::Arc<Vec<bool>>>> =
    std::sync::LazyLock::new(Default::default);

/// The models those blocks' code drew, by position (in the dimension the game shows).
#[allow(clippy::type_complexity)]
static CODE_MODELS: std::sync::LazyLock<
    std::sync::RwLock<
        std::sync::Arc<
            std::collections::HashMap<[i32; 3], std::sync::Arc<crate::models::catalog::BakedModel>>,
        >,
    >,
> = std::sync::LazyLock::new(Default::default);

/// Sets which states their mods' code draws per position.
pub fn set_drawn_by_code(states: &[u32]) {
    let mut drawn = Vec::new();
    for &s in states {
        let s = s as usize;
        if drawn.len() <= s {
            drawn.resize(s + 1, false);
        }
        drawn[s] = true;
    }
    *DRAWN_BY_CODE
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = std::sync::Arc::new(drawn);
}

/// The model the block at `pos` was drawn with by its mod's code (`None`: none any more).
pub fn set_code_model(pos: [i32; 3], model: Option<crate::models::catalog::BakedModel>) {
    let mut models = CODE_MODELS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let mut next = (**models).clone();
    match model {
        Some(model) => next.insert(pos, std::sync::Arc::new(model)),
        None => next.remove(&pos),
    };
    *models = std::sync::Arc::new(next);
}

/// Forgets every model drawn by code (another world or dimension).
pub fn clear_code_models() {
    *CODE_MODELS
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Default::default();
}

/// `ns:block/path` for every block texture of a namespace not vanilla's under a content
/// root's `assets/<ns>/textures/block/`.
pub fn mod_block_sprites(root: &std::path::Path) -> Vec<String> {
    fn walk(dir: &std::path::Path, base: &std::path::Path, ns: &str, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, base, ns, out);
            } else if path.extension().is_some_and(|e| e == "png")
                && let Ok(rel) = path.strip_prefix(base)
            {
                let rel = rel.with_extension("");
                let rel = rel.to_string_lossy().replace('\\', "/");
                out.push(format!("{ns}:block/{rel}"));
            }
        }
    }
    let mut out = Vec::new();
    let Ok(namespaces) = std::fs::read_dir(root.join("assets")) else {
        return out;
    };
    for ns in namespaces.flatten() {
        let name = ns.file_name().to_string_lossy().into_owned();
        if name == "minecraft" {
            continue;
        }
        let base = ns.path().join("textures/block");
        walk(&base, &base, &name, &mut out);
    }
    out.sort();
    out
}
