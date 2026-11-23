//! Horizon's neighbour updates beside the 1.20.1 mods' blocks. When Horizon updates the
//! neighbours of a position (a lever flipped, a block placed or broken) and one of them is a
//! mod's block, the position is kept (`LevelAccess::observe_neighbours_updated`); each tick the
//! mods' blocks beside the kept positions hear it in the runtime (their shape for that side,
//! then `neighborChanged`, as vanilla's neighbour updater tells them: a Wired Redstone wire
//! takes the power of a lever flipped beside it), and what they set and changed lands here.

use super::{Level, NAMESPACES, runtime, tick_failed_once};
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
        self.retry_java120_shapes();
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
        // By the runtimes whose blocks are beside each position (not the block at it, which may
        // be gone: the lower half of a waystone just broken, its upper half to hear it).
        for (loader, group) in self.java120_by_neighbours_owner(&positions) {
            let _route = super::route(loader);
            self.java120_neighbours_of(group);
        }
    }

    /// `positions` by the runtimes owning a block beside each: a position beside blocks of both
    /// goes to both.
    fn java120_by_neighbours_owner(
        &self,
        positions: &[[i32; 3]],
    ) -> Vec<(super::Loader, Vec<[i32; 3]>)> {
        let mut groups: Vec<(super::Loader, Vec<[i32; 3]>)> = Vec::new();
        for &[x, y, z] in positions {
            let mut owners: Vec<super::Loader> = Vec::new();
            for (dx, dy, dz) in [
                (1, 0, 0),
                (-1, 0, 0),
                (0, 1, 0),
                (0, -1, 0),
                (0, 0, 1),
                (0, 0, -1),
            ] {
                if let Some(loader) = self.java120_owner_at([x + dx, y + dy, z + dz])
                    && !owners.contains(&loader)
                {
                    owners.push(loader);
                }
            }
            for loader in owners {
                match groups.iter_mut().find(|(l, _)| *l == loader) {
                    Some((_, group)) => group.push([x, y, z]),
                    None => groups.push((loader, vec![[x, y, z]])),
                }
            }
        }
        groups
    }

    /// The neighbours' changes at `positions`, told to the runtime calls go to now.
    fn java120_neighbours_of(&mut self, positions: Vec<[i32; 3]>) {
        let dimension = self.dim.id().to_owned();
        let flat: Vec<i32> = positions.iter().flatten().copied().collect();
        let heard = {
            let Ok(mut guard) = runtime().try_lock() else {
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

    /// The players the mods' code teleported (a waystone's teleport): Horizon's player moves
    /// there, into the other dimension when it is one.
    pub(in crate::game) fn tick_java120_teleports(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let me = self.java120_actor();
        for teleport in horizon_jvm_remote::mirror120::take_teleports() {
            if teleport.uuid != me {
                continue;
            }
            let rotation = Some([teleport.yaw, teleport.pitch]);
            let target = super::super::commands::Target::Player;
            if teleport.dimension == self.dim.id() {
                self.teleport_to_in_level(target, teleport.pos, rotation, [false; 3], false);
            } else {
                let level = super::super::Dim::from_id(&teleport.dimension);
                if let Err(e) = self.teleport_across(
                    target,
                    level,
                    teleport.pos,
                    rotation,
                    false,
                    crate::game::vehicle_teleport::TeleportMotion::Stop,
                ) {
                    log::warn!(
                        "1.20.1 mods: a teleport into {} failed: {e:?}",
                        teleport.dimension
                    );
                }
            }
        }
    }
}
