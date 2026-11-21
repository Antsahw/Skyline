//! The 1.20.1 mods' fluids in the world (phase 6e), run by their own code. A mod's fluid block
//! (Tech Reborn's oil) is carried with water's or lava's template, so Horizon's fluid code ran
//! it as that: oil spread as `minecraft:water`. Now its states are foreign to Horizon's fluids
//! (`LevelAccess::foreign_fluid`): Horizon's water and lava leave them, and where Horizon would
//! schedule their tick (placed, a neighbour changed) it says so here instead. Each server tick
//! those changes go to the runtime, which schedules the fluid's tick at the mod's own delay, and
//! the ticks due run there (`FluidState.tick`: the mod's spread and drop-off); the blocks they
//! set land in Horizon's world, which tells this module again, and so it flows. As the 26.3
//! bridge does its mods' fluids (`java_server/blocks.rs`, `JAVA_FLUID_STATES`).

use super::{Level, RUNTIME};
use horizon_core::position::BlockPos;
use horizon_world::registry::BlockStateId;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

/// By block state id: a mod's fluid.
static FLUID_STATES: OnceLock<Vec<bool>> = OnceLock::new();

/// Where Horizon would have scheduled a mod fluid's tick since the last server tick.
static CHANGED: Mutex<Vec<[i32; 3]>> = Mutex::new(Vec::new());

/// By dimension: how many of the mods' fluid ticks the runtime has pending.
static PENDING: Mutex<Option<HashMap<String, i32>>> = Mutex::new(None);

/// `LevelAccess::foreign_fluid`, for the 1.20.1 mods: whether `state` is one of their fluids.
pub(in crate::game) fn is_fluid(state: BlockStateId) -> bool {
    FLUID_STATES
        .get()
        .and_then(|s| s.get(state.0 as usize).copied())
        .unwrap_or(false)
}

/// `LevelAccess::foreign_fluid_changed`, for them: a fluid of theirs at `pos` was placed, or
/// a neighbour of it changed.
pub(in crate::game) fn changed(pos: BlockPos) {
    let mut changed = CHANGED.lock().unwrap_or_else(|e| e.into_inner());
    if changed.len() < 65_536 {
        changed.push([pos.x, pos.y, pos.z]);
    }
}

impl super::super::Game<'_> {
    /// Which states are the mods' fluids, once the runtime can say.
    fn java120_fluid_states(&self) -> bool {
        if FLUID_STATES.get().is_some() {
            return true;
        }
        let Ok(mut guard) = RUNTIME.try_lock() else {
            return false;
        };
        let Some(runtime) = guard.as_mut() else {
            return false;
        };
        let ids: std::collections::HashSet<String> = match runtime.mod_fluid_blocks() {
            Ok(ids) => ids.into_iter().collect(),
            Err(e) => {
                log::warn!("1.20.1 mods: their fluids can't be listed: {e}");
                Default::default()
            }
        };
        let registry = self.pack.assets.registry;
        let states: Vec<bool> = (0..registry.len() as u32)
            .map(|id| {
                registry
                    .state(BlockStateId(id))
                    .is_some_and(|s| ids.contains(s.name.as_str()))
            })
            .collect();
        if !ids.is_empty() {
            log::info!(
                "1.20.1 mods: {} fluid(s) flow by their own code ({} states)",
                ids.len(),
                states.iter().filter(|f| **f).count()
            );
        }
        let _ = FLUID_STATES.set(states);
        true
    }

    /// One server tick of the mods' fluids in the level running now: Horizon's changes to the
    /// runtime's schedule, then the ticks due. A busy runtime misses a tick, nothing more: the
    /// changes wait for the next one.
    pub(in crate::game) fn tick_java120_fluids(&mut self) {
        if self.remote.is_some() || !self.java120_fluid_states() {
            return;
        }
        let dimension = self.dim.id().to_owned();
        let now = self.world.game_time;
        let pending = PENDING
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .and_then(|p| p.get(&dimension).copied())
            .unwrap_or(0);
        let changed_empty = CHANGED.lock().unwrap_or_else(|e| e.into_inner()).is_empty();
        if pending == 0 && changed_empty {
            return;
        }
        let ran = {
            let Ok(mut guard) = RUNTIME.try_lock() else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            let positions: Vec<i32> =
                std::mem::take(&mut *CHANGED.lock().unwrap_or_else(|e| e.into_inner()))
                    .into_iter()
                    .flatten()
                    .collect();
            if !positions.is_empty()
                && let Err(e) = runtime.fluid_changed(&Level(self), &dimension, &positions, now)
            {
                log::warn!("1.20.1 mods: their fluids' ticks weren't scheduled: {e}");
            }
            runtime.fluid_ticks(&Level(self), &dimension, now)
        };
        match ran {
            Ok(ticked) => {
                PENDING
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get_or_insert_with(HashMap::new)
                    .insert(dimension.clone(), ticked.ticked);
                self.apply_java120(
                    &dimension,
                    &ticked.set,
                    &ticked.block_entities,
                    &ticked.spawned,
                );
            }
            Err(e) => log::warn!("1.20.1 mods: their fluids' ticks failed: {e}"),
        }
    }

    /// The game: the mods' fluids drawn with their own sprites (Horizon's mesher's mod fluid
    /// looks, as the 26.3 bridge's), once the runtime can say them. Only where this process
    /// runs the runtime (single player, a LAN host), as the 26.3 bridge's fluids.
    pub(in crate::game) fn java120_fluid_looks(&mut self) {
        static DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
        if DONE.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }
        let looks = {
            let Ok(mut guard) = RUNTIME.try_lock() else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            DONE.store(true, std::sync::atomic::Ordering::Relaxed);
            runtime.fluid_looks()
        };
        let looks: HashMap<String, serde_json::Value> =
            match looks.map(|l| serde_json::from_str(&l)) {
                Ok(Ok(looks)) => looks,
                Ok(Err(e)) => return log::warn!("1.20.1 mods: their fluids' looks: {e}"),
                Err(e) => return log::warn!("1.20.1 mods: their fluids' looks: {e}"),
            };
        if looks.is_empty() {
            return;
        }
        let registry = self.pack.assets.registry;
        let mut by_state = HashMap::new();
        for id in 0..registry.len() as u32 {
            let Some(look) = registry
                .state(BlockStateId(id))
                .and_then(|s| looks.get(s.name.as_str()))
            else {
                continue;
            };
            let text = |k: &str| look[k].as_str().unwrap_or_default().to_owned();
            by_state.insert(
                id,
                horizon_client::mesher::ModFluidLook {
                    still: text("still"),
                    flow: text("flow"),
                    tint: look["tint"].as_i64().unwrap_or(-1) as u32,
                    translucent: look["translucent"].as_bool().unwrap_or(false),
                },
            );
        }
        log::info!(
            "1.20.1 mods: {} fluid(s) drawn with their own sprites ({} states)",
            looks.len(),
            by_state.len()
        );
        if horizon_client::mesher::set_mod_fluids(by_state) {
            self.all_sections_changed();
        }
    }
}
