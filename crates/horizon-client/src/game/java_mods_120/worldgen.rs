//! The 1.20.1 mods' own features in Horizon's generator (phase 6b). A feature of a mod's own
//! type (Farmer's Delight's `wild_crop`) has no Rust implementation, so the generator hands it
//! to the runtime (horizon-world's `worldgen::foreign`), which places it with the mod's own
//! code on the chunks being decorated: reads from the region, writes into it as they happen.

use super::{DATA_VERSION_120, wait_for_runtime};
use horizon_jvm_remote::mirror120::{PlaceFeature, Region120};
use horizon_world::worldgen::foreign::{ForeignFeatures, ForeignPlacement, ForeignWorld};
use std::collections::HashSet;
use std::sync::Arc;

struct Features120 {
    features: HashSet<String>,
}

impl ForeignFeatures for Features120 {
    fn places(&self, feature: &str) -> bool {
        self.features.contains(feature)
    }

    fn place(&self, placement: &ForeignPlacement<'_>, world: &mut dyn ForeignWorld) -> bool {
        let place = PlaceFeature {
            dimension: "minecraft:overworld".into(),
            feature: placement.feature.to_owned(),
            world_seed: placement.world_seed,
            chunk: placement.chunk,
            index: placement.index,
            step: placement.step,
            origin: placement.origin,
        };
        let Some(mut guard) = wait_for_runtime() else {
            return false;
        };
        let Some(runtime) = guard.as_mut() else {
            return false;
        };
        let mut region = Region { world };
        let placed = match runtime.place_feature(&mut region, &place) {
            Ok(placed) => placed,
            Err(e) => {
                failed_once(placement.feature, &e);
                false
            }
        };
        count(placed);
        placed
    }
}

/// How many placements were asked and how many placed something, said now and then.
fn count(placed: bool) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static ASKED: AtomicU64 = AtomicU64::new(0);
    static PLACED: AtomicU64 = AtomicU64::new(0);
    let asked = ASKED.fetch_add(1, Ordering::Relaxed) + 1;
    let placed = PLACED.fetch_add(u64::from(placed), Ordering::Relaxed) + u64::from(placed);
    if asked.is_power_of_two() || asked.is_multiple_of(1000) {
        log::info!("1.20.1 mods' features: {placed} of {asked} placements placed something");
    }
}

/// Said once per feature: one that fails fails in every chunk.
fn failed_once(feature: &str, e: &str) {
    static SAID: std::sync::Mutex<Option<HashSet<String>>> = std::sync::Mutex::new(None);
    let mut said = SAID.lock().unwrap_or_else(|e| e.into_inner());
    if said
        .get_or_insert_with(HashSet::new)
        .insert(feature.to_owned())
    {
        log::warn!("1.20.1 mods: their feature {feature} was not placed: {e}");
    }
}

/// The region as the runtime's side asks it.
struct Region<'a, 'w> {
    world: &'a mut (dyn ForeignWorld + 'w),
}

impl Region120 for Region<'_, '_> {
    fn block_state(&mut self, x: i32, y: i32, z: i32) -> Option<String> {
        self.world.block([x, y, z])
    }
    fn height(&mut self, heightmap: &str, x: i32, z: i32) -> Option<i32> {
        self.world.height(heightmap, x, z)
    }
    fn biome(&mut self, x: i32, y: i32, z: i32) -> Option<String> {
        self.world.biome([x, y, z])
    }
    fn set_block(&mut self, pos: [i32; 3], state: &str) {
        let state = state_from_120(state);
        if !self.world.set_block(pos, &state) {
            static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
                log::info!("1.20.1 mods: a feature's {state} at {pos:?} was not set (said once)");
            }
        }
    }
    fn set_block_entity(&mut self, pos: [i32; 3], snbt: &str) {
        if let Ok((horizon_world::nbt::Tag::Compound(nbt), _)) =
            horizon_world::nbt::parse_snbt(snbt)
        {
            self.world.set_block_entity(pos, nbt);
        }
    }
}

/// A 1.20.1 state's name as 26.3 names it, through vanilla's own fixes (`minecraft:grass` is
/// `minecraft:short_grass` since 1.20.3).
pub(super) fn state_from_120(state: &str) -> String {
    use horizon_world::datafix::{self, schema::Ty};
    use horizon_world::nbt::Tag;
    let mut tag = Tag::String(state.into());
    datafix::update(
        Ty::FlatBlockState,
        &mut tag,
        DATA_VERSION_120,
        horizon_core::version::DATA_VERSION,
    );
    match tag {
        Tag::String(fixed) => fixed.to_string_lossy(),
        _ => state.to_owned(),
    }
}

/// The runtime is up: Horizon's generator hands it the mods' own features from now on.
pub(super) fn install(runtime: &mut horizon_jvm_remote::boot120::Runtime120) {
    let features: HashSet<String> = match runtime.mod_features() {
        Ok(features) => features.into_iter().collect(),
        Err(e) => {
            log::warn!("1.20.1 mods: their features can't be listed: {e}");
            return;
        }
    };
    log::info!(
        "1.20.1 mods: {} feature(s) of theirs for the generator",
        features.len()
    );
    horizon_world::worldgen::foreign::set_foreign_features(Some(Arc::new(Features120 {
        features,
    })));
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_1_20_1_state_is_named_as_26_3_names_it() {
        assert_eq!(
            super::state_from_120("minecraft:grass"),
            "minecraft:short_grass"
        );
        assert_eq!(
            super::state_from_120("farmersdelight:wild_carrots"),
            "farmersdelight:wild_carrots"
        );
        assert_eq!(
            super::state_from_120("minecraft:oak_log[axis=y]"),
            "minecraft:oak_log[axis=y]"
        );
    }
}
