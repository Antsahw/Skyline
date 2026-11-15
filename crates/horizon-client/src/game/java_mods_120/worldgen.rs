//! The 1.20.1 mods' own features in Horizon's generator (phase 6b). A feature of a mod's own
//! type (Farmer's Delight's `wild_crop`) has no Rust implementation, so the generator hands it
//! to the runtime (horizon-world's `worldgen::foreign`), which places it with the mod's own
//! code on the chunks being decorated: reads from the region, writes into it as they happen.

use super::{DATA_VERSION_120, worldgen_runtime};
use horizon_jvm_remote::mirror120::{PlaceFeature, Region120};
use horizon_world::worldgen::foreign::{ForeignFeatures, ForeignPlacement, ForeignWorld};
use horizon_world::worldgen::structure::foreign::{
    ForeignStart, ForeignStartWorld, ForeignStructures,
};
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
        let Some(mut guard) = worldgen_runtime() else {
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

/// The mods' own structure types, made and placed by the runtime (phase 6h).
struct Structures120 {
    types: HashSet<String>,
    /// The mods' own placement types of structure sets (YUNG's), decided by their code.
    placements: HashSet<String>,
    /// Their answers by set and chunk: a chunk's sets are asked again for each neighbour's
    /// exclusion zones and for `/locate`.
    answered: std::sync::Mutex<std::collections::HashMap<(String, i64, [i32; 2]), bool>>,
}

impl ForeignStructures for Structures120 {
    fn generates(&self, structure_type: &str) -> bool {
        self.types.contains(structure_type)
    }

    fn places(&self, placement_type: &str) -> bool {
        self.placements.contains(placement_type)
    }

    fn placement_chunk(&self, set: &str, world_seed: i64, chunk: [i32; 2]) -> bool {
        let key = (set.to_owned(), world_seed, chunk);
        if let Some(&known) = self
            .answered
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&key)
        {
            return known;
        }
        let answer = {
            let Some(mut guard) = worldgen_runtime() else {
                return false;
            };
            let Some(runtime) = guard.as_mut() else {
                return false;
            };
            match runtime.placement_chunk(set, world_seed, chunk) {
                Ok(placed) => placed,
                Err(e) => {
                    failed_once(set, &e);
                    // Not remembered: a busy runtime may answer next time.
                    return false;
                }
            }
        };
        let mut answered = self.answered.lock().unwrap_or_else(|e| e.into_inner());
        if answered.len() > 200_000 {
            answered.clear();
        }
        answered.insert(key, answer);
        answer
    }

    fn start(
        &self,
        structure: &str,
        world_seed: i64,
        chunk: [i32; 2],
        world: &mut dyn ForeignStartWorld,
    ) -> Option<ForeignStart> {
        let mut guard = worldgen_runtime()?;
        let runtime = guard.as_mut()?;
        let mut region = StartRegion { world };
        match runtime.structure_start(
            &mut region,
            "minecraft:overworld",
            structure,
            world_seed,
            chunk,
        ) {
            Ok(start) => start.map(|s| ForeignStart {
                handle: s.handle,
                position: s.position,
                boxes: s.boxes,
            }),
            Err(e) => {
                failed_once(structure, &e);
                None
            }
        }
    }

    fn place(
        &self,
        handle: i64,
        world_seed: i64,
        chunk: [i32; 2],
        step: i32,
        world: &mut dyn ForeignWorld,
    ) -> bool {
        let Some(mut guard) = worldgen_runtime() else {
            return false;
        };
        let Some(runtime) = guard.as_mut() else {
            return false;
        };
        let mut region = Region { world };
        runtime
            .structure_place(
                &mut region,
                "minecraft:overworld",
                handle,
                world_seed,
                chunk,
                0,
                step,
            )
            .unwrap_or_else(|e| {
                failed_once("a structure start", &e);
                false
            })
    }
}

/// A start being made, as the runtime's side asks it: heights of Horizon's terrain.
struct StartRegion<'a> {
    world: &'a mut dyn ForeignStartWorld,
}

impl Region120 for StartRegion<'_> {
    fn block_state(&mut self, _: i32, _: i32, _: i32) -> Option<String> {
        None
    }
    fn height(&mut self, heightmap: &str, x: i32, z: i32) -> Option<i32> {
        self.world.height(heightmap, x, z)
    }
    fn biome(&mut self, _: i32, _: i32, _: i32) -> Option<String> {
        None
    }
    fn set_block(&mut self, _: [i32; 3], _: &str) {}
    fn set_block_entity(&mut self, _: [i32; 3], _: &str) {}
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
    // The mods' recipes of their own types, for recipe viewers (they're not Horizon's).
    match runtime.mod_recipes() {
        Ok(json) => {
            let recipes = foreign_recipes(&json);
            if !recipes.is_empty() {
                log::info!(
                    "1.20.1 mods: {} recipe(s) of their own types for recipe viewers",
                    recipes.len()
                );
            }
            horizon_client::mod_list::set_foreign_recipes(recipes);
        }
        Err(e) => log::warn!("1.20.1 mods: their recipes can't be listed: {e}"),
    }
    let types = runtime.mod_structure_types().unwrap_or_else(|e| {
        log::warn!("1.20.1 mods: their structure types can't be listed: {e}");
        Vec::new()
    });
    let placements = runtime.mod_placement_types().unwrap_or_else(|e| {
        log::warn!("1.20.1 mods: their structure placements can't be listed: {e}");
        Vec::new()
    });
    if !types.is_empty() {
        log::info!(
            "1.20.1 mods: their structure types for the generator: {}",
            types.join(", ")
        );
    }
    if !placements.is_empty() {
        log::info!(
            "1.20.1 mods: their structure placements for the generator: {}",
            placements.join(", ")
        );
    }
    if !types.is_empty() || !placements.is_empty() {
        horizon_world::worldgen::structure::foreign::set_foreign_structures(Some(Arc::new(
            Structures120 {
                types: types.into_iter().collect(),
                placements: placements.into_iter().collect(),
                answered: Default::default(),
            },
        )));
    }
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

/// The runtime's `modRecipes` answer as recipes; anything unreadable is left out.
fn foreign_recipes(json: &str) -> Vec<horizon_client::mod_list::ForeignRecipe> {
    let Ok(serde_json::Value::Array(all)) = serde_json::from_str::<serde_json::Value>(json) else {
        return Vec::new();
    };
    let text = |v: &serde_json::Value| v.as_str().map(str::to_owned);
    all.iter()
        .filter_map(|r| {
            Some(horizon_client::mod_list::ForeignRecipe {
                id: text(&r["id"])?,
                kind: text(&r["type"])?,
                inputs: r["inputs"]
                    .as_array()?
                    .iter()
                    .map(|slot| {
                        slot.as_array()
                            .map(|items| items.iter().filter_map(text).collect())
                            .unwrap_or_default()
                    })
                    .collect(),
                outputs: r["outputs"]
                    .as_array()?
                    .iter()
                    .filter_map(|o| {
                        Some((text(&o["item"])?, o["count"].as_u64().unwrap_or(1) as u32))
                    })
                    .collect(),
                station: text(&r["station"]),
                ticks: r["time"].as_u64().unwrap_or(0) as u32,
                experience: r["experience"].as_f64().unwrap_or(0.0) as f32,
                container: text(&r["container"]),
            })
        })
        .collect()
}

#[cfg(test)]
mod recipe_tests {
    #[test]
    fn the_runtimes_recipes_are_read_and_bad_ones_left_out() {
        let json = r#"[
            {"id":"farmersdelight:cooking/beef_stew","type":"farmersdelight:cooking",
             "inputs":[["minecraft:beef"],["minecraft:carrot","minecraft:potato"],[]],
             "outputs":[{"item":"farmersdelight:beef_stew","count":2}],
             "station":"farmersdelight:cooking_pot","time":200,"experience":1.0,
             "container":"minecraft:bowl"},
            {"id":"broken","inputs":[]}
        ]"#;
        let recipes = super::foreign_recipes(json);
        assert_eq!(recipes.len(), 1);
        let r = &recipes[0];
        assert_eq!(r.kind, "farmersdelight:cooking");
        assert_eq!(r.inputs[1], ["minecraft:carrot", "minecraft:potato"]);
        assert!(r.inputs[2].is_empty());
        assert_eq!(r.outputs, [("farmersdelight:beef_stew".to_owned(), 2)]);
        assert_eq!(r.station.as_deref(), Some("farmersdelight:cooking_pot"));
        assert_eq!((r.ticks, r.experience), (200, 1.0));
        assert_eq!(r.container.as_deref(), Some("minecraft:bowl"));
        assert!(super::foreign_recipes("not json").is_empty());
    }
}
