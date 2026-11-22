//! Placed features the generator can't run itself, placed by someone who can.
//!
//! A mod's feature of its own type (a Fabric 1.20.1 mod's `farmersdelight:wild_crop`) has no
//! Rust implementation: its code is the mod's. When the game runs those mods somewhere (the
//! 1.20.1 runtime, `horizon-jvm-remote`), it installs a [`ForeignFeatures`] and the decoration
//! step hands it each such feature where vanilla's `applyBiomeDecoration` would place it: the
//! feature's name, what seeds vanilla's random for it, and the region being decorated as a
//! [`ForeignWorld`] (blocks, biomes and heights by name; writes inside the region only, as
//! `WorldGenRegion.ensureCanWrite` allows). Without one, such features are skipped as before.
use std::sync::{Arc, RwLock};

/// What seeds vanilla's random for one placement: `WorldgenRandom.setDecorationSeed(world_seed,
/// chunk x * 16, chunk z * 16)`, then `setFeatureSeed(that, index, step)`; and the origin, the
/// chunk's corner at the world's minimum height, as `applyBiomeDecoration` passes it.
#[derive(Clone, Debug, PartialEq)]
pub struct ForeignPlacement<'a> {
    /// The placed feature's id.
    pub feature: &'a str,
    pub world_seed: i64,
    /// The chunk being decorated.
    pub chunk: [i32; 2],
    /// The feature's index in its step, and the step.
    pub index: i32,
    pub step: i32,
    pub origin: [i32; 3],
}

/// The region being decorated, as the code placing a foreign feature sees it.
pub trait ForeignWorld {
    /// The state at a position, as `minecraft:oak_log[axis=y]`; `None` outside the region.
    fn block(&mut self, position: [i32; 3]) -> Option<String>;
    /// The biome at a position, by id.
    fn biome(&mut self, position: [i32; 3]) -> Option<String>;
    /// A heightmap's height at a column (`MOTION_BLOCKING`, ...): the y above the top block.
    fn height(&mut self, heightmap: &str, x: i32, z: i32) -> Option<i32>;
    /// Sets a state (by name); false where it can't be written (outside the region, unknown).
    fn set_block(&mut self, position: [i32; 3], state: &str) -> bool;
    /// Keeps a block entity's NBT (as vanilla saves it) at a position.
    fn set_block_entity(&mut self, position: [i32; 3], nbt: horizon_core::nbt::Compound) -> bool;
}

/// Whoever places the features the generator can't.
pub trait ForeignFeatures: Send + Sync {
    /// Whether it places this feature at all (asked once per generator, per feature).
    fn places(&self, feature: &str) -> bool;
    /// Places one; whether it placed anything.
    fn place(&self, placement: &ForeignPlacement<'_>, world: &mut dyn ForeignWorld) -> bool;
}

static FOREIGN: RwLock<Option<Arc<dyn ForeignFeatures>>> = RwLock::new(None);

/// Whether a placer of foreign features and structures is on its way (a Java mods' runtime that
/// starts beside the game): until it is, chunks wait for it ([`wait_for_foreign`]) rather than
/// generate without the mods' features and structures for good.
static PENDING: (std::sync::Mutex<bool>, std::sync::Condvar) =
    (std::sync::Mutex::new(false), std::sync::Condvar::new());

/// How long a chunk waits for a placer on its way, at most.
const PENDING_WAIT: std::time::Duration = std::time::Duration::from_secs(120);

/// Says a placer is on its way (`true`), or that it came or never will (`false`).
pub fn set_foreign_pending(pending: bool) {
    let (lock, ready) = &PENDING;
    *lock.lock().unwrap_or_else(|e| e.into_inner()) = pending;
    if !pending {
        ready.notify_all();
    }
}

/// Waits while a placer is on its way ([`set_foreign_pending`]), two minutes at most.
pub fn wait_for_foreign() {
    let (lock, ready) = &PENDING;
    let pending = lock.lock().unwrap_or_else(|e| e.into_inner());
    if !*pending {
        return;
    }
    let (_pending, timeout) = ready
        .wait_timeout_while(pending, PENDING_WAIT, |p| *p)
        .unwrap_or_else(|e| e.into_inner());
    if timeout.timed_out() {
        log::warn!(
            "Horizon: the Java mods' features weren't ready in time; this chunk goes on without them"
        );
    }
}

/// Installs (or, with `None`, removes) the placer of foreign features.
pub fn set_foreign_features(placer: Option<Arc<dyn ForeignFeatures>>) {
    *FOREIGN.write().unwrap_or_else(|e| e.into_inner()) = placer;
}

/// The placer installed now.
pub fn foreign_features() -> Option<Arc<dyn ForeignFeatures>> {
    FOREIGN.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// `minecraft:oak_log[axis=y]` for a state of `registry`.
pub fn state_string(
    registry: &crate::registry::BlockRegistry,
    id: crate::registry::BlockStateId,
) -> Option<String> {
    let state = registry.state(id)?;
    if state.properties.is_empty() {
        return Some(state.name.clone());
    }
    let props: Vec<String> = state
        .properties
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    Some(format!("{}[{}]", state.name, props.join(",")))
}

/// The state `minecraft:oak_log[axis=y]` names in `registry`: its block's default with the
/// properties given (unknown properties or values: `None`).
pub fn parse_state_string(
    registry: &crate::registry::BlockRegistry,
    text: &str,
) -> Option<crate::registry::BlockStateId> {
    let (name, props) = match text.split_once('[') {
        Some((n, rest)) => (n, rest.strip_suffix(']')?),
        None => (text, ""),
    };
    let default = registry.default_state(name)?;
    let mut state = registry.state(default)?.clone();
    for pair in props.split(',').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=')?;
        let slot = state.properties.get_mut(k)?;
        *slot = v.to_owned();
    }
    registry.id(&state)
}
