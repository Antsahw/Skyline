//! Horizon's hoppers and droppers at a 1.20.1 mod's block (phase 3): Fabric Transfer API, as
//! its patch of vanilla's hopper asks, in the runtime (platform120's Hoppers120). The block
//! entity is the mod's own, kept by Horizon as opaque NBT, so an item goes in or out through
//! the mods' code and the NBT comes back changed, as every call's changes do.
//!
//! Asked only where a 1.20.1 mod's block is; a block that turns out to hold no items on that
//! face is not asked again for two seconds (a hopper under a stove tries every 8 ticks).

use super::{Level, NAMESPACES, RUNTIME, stack_from_120};
use horizon_client::items::ItemStack;
use horizon_core::geometry::Direction;
use horizon_jvm_remote::mirror120::{Hopper, Moved};
use std::collections::HashMap;
use std::sync::Mutex;

/// Faces with no item storage, by dimension, position and face: the game time to ask again.
static NO_STORAGE: Mutex<Option<HashMap<(String, [i32; 3], u8), i64>>> = Mutex::new(None);

/// How long a face with no storage is left alone (vanilla's hopper cooldown is 8 ticks).
const NO_STORAGE_TICKS: i64 = 40;

fn one(stack: &ItemStack) -> (&str, i32, i32) {
    (&stack.item, stack.count as i32, stack.damage as i32)
}

impl super::super::Game<'_> {
    /// Whether the block at `pos` is a 1.20.1 mod's that a hopper may ask about `face`.
    fn java120_hopper_block(&self, pos: [i32; 3], face: Direction) -> bool {
        if self.remote.is_some() {
            return false;
        }
        let registry = self.pack.assets.registry;
        let state = super::state_name(registry, self.world.block(pos[0], pos[1], pos[2]));
        let ours = {
            let namespaces = NAMESPACES.lock().unwrap_or_else(|e| e.into_inner());
            state
                .split_once(':')
                .is_some_and(|(ns, _)| namespaces.as_ref().is_some_and(|n| n.contains(ns)))
        };
        if !ours {
            return false;
        }
        let key = (self.dim.id().to_owned(), pos, face as u8);
        let mut cache = NO_STORAGE.lock().unwrap_or_else(|e| e.into_inner());
        let cache = cache.get_or_insert_with(HashMap::new);
        match cache.get(&key) {
            Some(&until) if self.world.game_time < until => false,
            Some(_) => {
                cache.remove(&key);
                true
            }
            None => true,
        }
    }

    fn java120_no_storage(&self, pos: [i32; 3], face: Direction) {
        let mut cache = NO_STORAGE.lock().unwrap_or_else(|e| e.into_inner());
        let cache = cache.get_or_insert_with(HashMap::new);
        if cache.len() > 4096 {
            cache.clear();
        }
        cache.insert(
            (self.dim.id().to_owned(), pos, face as u8),
            self.world.game_time + NO_STORAGE_TICKS,
        );
    }

    /// What a move changed besides the item (the block entity it went into or came out of),
    /// landed here; and whether the block had storage at all.
    fn java120_moved<T>(
        &mut self,
        pos: [i32; 3],
        face: Direction,
        moved: Moved<T>,
    ) -> Option<Option<T>> {
        let dimension = self.dim.id().to_owned();
        self.apply_java120(
            &dimension,
            &moved.set,
            &moved.block_entities,
            &moved.spawned,
        );
        match moved.hopper {
            Hopper::Moved(t) => Some(Some(t)),
            Hopper::Nothing => Some(None),
            Hopper::NoStorage => {
                self.java120_no_storage(pos, face);
                None
            }
        }
    }

    /// A hopper (or dropper) pushes one item of its slots (`stacks`, in order) into the 1.20.1
    /// mod's block at `pos` through `face`. None when the block isn't a 1.20.1 mod's or holds
    /// no items there (or the runtime is busy: the hopper tries again); else the slot whose
    /// item went in, if one did. The same contract as the 26.3 bridge's `java_hopper_insert`.
    pub(in crate::game) fn java120_hopper_insert(
        &mut self,
        pos: [i32; 3],
        face: Direction,
        stacks: &[Option<&ItemStack>],
    ) -> Option<Option<usize>> {
        if !self.java120_hopper_block(pos, face) {
            return None;
        }
        let slots: Vec<Option<(&str, i32, i32)>> = stacks.iter().map(|s| s.map(one)).collect();
        let dimension = self.dim.id().to_owned();
        let moved = {
            let Ok(mut guard) = RUNTIME.try_lock() else {
                return None;
            };
            let runtime = guard.as_mut()?;
            runtime.hopper_insert(&Level(self), &dimension, pos, face as u8, &slots)
        };
        match moved {
            Ok(moved) => self.java120_moved(pos, face, moved),
            Err(e) => {
                log::warn!("1.20.1 mods: a hopper's push into {pos:?} failed: {e}");
                self.java120_no_storage(pos, face);
                None
            }
        }
    }

    /// A hopper pulls one item out of the 1.20.1 mod's block at `pos` through `face`: the
    /// first it offers that fits (`any_room`: the hopper has an empty slot; else one of
    /// `partial`, its stacks not full yet). None when the block isn't a 1.20.1 mod's or holds
    /// no items there; else the item, if one came. As the 26.3 bridge's `java_hopper_extract`.
    pub(in crate::game) fn java120_hopper_extract(
        &mut self,
        pos: [i32; 3],
        face: Direction,
        any_room: bool,
        partial: &[&ItemStack],
    ) -> Option<Option<ItemStack>> {
        if !self.java120_hopper_block(pos, face) {
            return None;
        }
        let partial: Vec<(&str, i32, i32)> = partial.iter().map(|s| one(s)).collect();
        let dimension = self.dim.id().to_owned();
        let moved = {
            let Ok(mut guard) = RUNTIME.try_lock() else {
                return None;
            };
            let runtime = guard.as_mut()?;
            runtime.hopper_extract(
                &Level(self),
                &dimension,
                pos,
                face as u8,
                any_room,
                &partial,
            )
        };
        let moved = match moved {
            Ok(moved) => moved,
            Err(e) => {
                log::warn!("1.20.1 mods: a hopper's pull out of {pos:?} failed: {e}");
                self.java120_no_storage(pos, face);
                return None;
            }
        };
        let item = self.java120_moved(pos, face, moved)?;
        Some(item.and_then(|snbt| {
            let stack = stack_from_120(&snbt);
            if stack.is_none() {
                log::warn!(
                    "1.20.1 mods: a hopper pulled {snbt} out of {pos:?}, which doesn't read"
                );
            }
            stack
        }))
    }
}
