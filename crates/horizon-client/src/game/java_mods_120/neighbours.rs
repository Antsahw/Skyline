//! Horizon's neighbour updates beside the 1.20.1 mods' blocks. When Horizon updates the
//! neighbours of a position (a lever flipped, a block placed or broken) and one of them is a
//! mod's block, the position is kept (`LevelAccess::observe_neighbours_updated`); each tick the
//! mods' blocks beside the kept positions hear it in the runtime (their shape for that side,
//! then `neighborChanged`, as vanilla's neighbour updater tells them: a Wired Redstone wire
//! takes the power of a lever flipped beside it), and what they set and changed lands here.

use super::{Level, NAMESPACES, RUNTIME, tick_failed_once};
use horizon_core::position::BlockPos;
use horizon_world::registry::BlockStateId;
use std::sync::{Mutex, OnceLock};

/// Whether each state (by id) is a 1.20.1 mod's: made once the mods' namespaces are known.
static OURS: OnceLock<Vec<bool>> = OnceLock::new();
/// The positions whose neighbours Horizon updated with a mod's block among them.
static UPDATED: Mutex<Vec<[i32; 3]>> = Mutex::new(Vec::new());
/// Kept at most: a tick's worth of a huge redstone machine.
const MAX: usize = 1 << 16;

impl super::super::Game<'_> {
    /// `LevelAccess::observe_neighbours_updated`: kept when a neighbour of `source` is a mod's
    /// block (`state_at` reads the level).
    pub(in crate::game) fn java120_neighbours_updated(
        source: BlockPos,
        state_at: impl Fn(BlockPos) -> BlockStateId,
    ) {
        let Some(ours) = OURS.get() else {
            return;
        };
        let near = [
            (1, 0, 0),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ]
        .iter()
        .any(|&(x, y, z)| {
            let id = state_at(BlockPos::new(source.x + x, source.y + y, source.z + z)).0;
            ours.get(id as usize).copied().unwrap_or(false)
        });
        if near {
            let mut updated = UPDATED.lock().unwrap_or_else(|e| e.into_inner());
            let p = [source.x, source.y, source.z];
            if updated.len() < MAX && updated.last() != Some(&p) {
                updated.push(p);
            }
        }
    }

    /// The mods' blocks hear the neighbour updates kept since the last tick. A busy runtime is
    /// skipped and the updates kept for the next tick.
    pub(in crate::game) fn tick_java120_neighbours(&mut self) {
        if self.remote.is_some() {
            return;
        }
        if OURS.get().is_none() {
            let namespaces = NAMESPACES.lock().unwrap_or_else(|e| e.into_inner());
            let Some(namespaces) = namespaces.as_ref() else {
                return;
            };
            let registry = self.pack.assets.registry;
            let ours = (0..registry.len() as u32)
                .map(|id| {
                    registry
                        .state(BlockStateId(id))
                        .and_then(|s| s.name.split_once(':'))
                        .is_some_and(|(ns, _)| namespaces.contains(ns))
                })
                .collect();
            let _ = OURS.set(ours);
        }
        let positions: Vec<[i32; 3]> = {
            let mut updated = UPDATED.lock().unwrap_or_else(|e| e.into_inner());
            if updated.is_empty() {
                return;
            }
            let mut positions = std::mem::take(&mut *updated);
            positions.sort_unstable();
            positions.dedup();
            positions
        };
        let dimension = self.dim.id().to_owned();
        let flat: Vec<i32> = positions.iter().flatten().copied().collect();
        let heard = {
            let Ok(mut guard) = RUNTIME.try_lock() else {
                UPDATED
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .extend(positions);
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            runtime.neighbours(&Level(self), &dimension, &flat)
        };
        match heard {
            Ok(heard) => self.apply_java120(
                &dimension,
                &heard.set,
                &heard.block_entities,
                &heard.spawned,
            ),
            Err(e) => tick_failed_once(&e),
        }
    }
}
