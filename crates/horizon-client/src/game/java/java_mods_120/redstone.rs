//! The redstone of the 1.20.1 mods' blocks, for Horizon's own redstone. A mod's block gives
//! power by its code (a Wired Redstone wire carries what it was given), which Horizon's
//! registry can't say. So whenever such a block changes, the server asks the mods what it
//! gives toward each side (`Runtime120::block_signals`) and keeps it here; Horizon's redstone
//! reads it through `LevelAccess::adjust_signal` without calling the mods, and a change
//! updates the block's neighbours, as vanilla's `updateNeighborsAt` would (a lamp beside the
//! wire looks again).

use horizon_core::geometry::Direction;
use horizon_core::position::BlockPos;
use horizon_world::registry::BlockStateId;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

/// Per position: the state it was asked for, and its signals (six `getSignal`, six
/// `getDirectSignal`, in vanilla's side order).
static SIGNALS: Mutex<Option<HashMap<[i32; 3], (BlockStateId, [u8; 12])>>> = Mutex::new(None);
/// The mods' states that are redstone sources.
static SOURCES: OnceLock<HashSet<u32>> = OnceLock::new();

/// Vanilla's 3D data value of a side (down 0, up 1, north 2, south 3, west 4, east 5).
fn side(direction: Direction) -> usize {
    match direction {
        Direction::Down => 0,
        Direction::Up => 1,
        Direction::North => 2,
        Direction::South => 3,
        Direction::West => 4,
        Direction::East => 5,
    }
}

/// The mods' signal sources, once known (`Game::java120_signal_sources`).
pub(super) fn set_sources(states: HashSet<u32>) {
    let _ = SOURCES.set(states);
}

/// Whether the sources are known yet.
pub(super) fn sources_known() -> bool {
    SOURCES.get().is_some()
}

/// Keeps what a block gives (or, with `None`, forgets it); whether that changed.
pub(super) fn keep(pos: [i32; 3], value: Option<(BlockStateId, [u8; 12])>) -> bool {
    let mut all = SIGNALS.lock().unwrap_or_else(|e| e.into_inner());
    let all = all.get_or_insert_with(HashMap::new);
    let before = all.get(&pos).map(|(_, s)| *s);
    let after = value.map(|(_, s)| s);
    match value {
        Some(v) => {
            all.insert(pos, v);
        }
        None => {
            all.remove(&pos);
        }
    }
    before.unwrap_or_default() != after.unwrap_or_default()
}

impl super::super::Game<'_> {
    /// `LevelAccess::adjust_signal_source`: a mod's state that its mod says is a source is one.
    pub(in crate::game) fn java120_signal_source(state: BlockStateId, vanilla: bool) -> bool {
        vanilla || SOURCES.get().is_some_and(|s| s.contains(&state.0))
    }

    /// `LevelAccess::adjust_signal`: what a mod's block at `pos` gives toward `direction`
    /// (`direct`: its direct signal), when the mods were asked for it and it is still `state`.
    pub(in crate::game) fn java120_signal(
        pos: BlockPos,
        state: BlockStateId,
        direction: Direction,
        direct: bool,
        vanilla: i32,
    ) -> i32 {
        let all = SIGNALS.lock().unwrap_or_else(|e| e.into_inner());
        match all.as_ref().and_then(|a| a.get(&[pos.x, pos.y, pos.z])) {
            Some((at, signals)) if *at == state => {
                i32::from(signals[side(direction) + if direct { 6 } else { 0 }])
            }
            _ => vanilla,
        }
    }

    /// The blocks around `pos` hear that what it gives changed: its neighbours, and theirs
    /// (a strong signal reaches through a block), as `RedStoneWireBlock` updates them.
    pub(super) fn java120_signal_changed(&mut self, [x, y, z]: [i32; 3]) {
        let at = BlockPos::new(x, y, z);
        let state = self.world.block(x, y, z);
        self.with_level(|b, level, ticks| {
            let block = b.block(state);
            b.update_neighbors_at(level, ticks, at, block);
            for [dx, dy, dz] in [
                [0, -1, 0],
                [0, 1, 0],
                [0, 0, -1],
                [0, 0, 1],
                [-1, 0, 0],
                [1, 0, 0],
            ] {
                b.update_neighbors_at(level, ticks, at.offset(dx, dy, dz), block);
            }
        });
    }
}
