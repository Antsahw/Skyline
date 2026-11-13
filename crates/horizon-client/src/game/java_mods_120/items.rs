//! A right-click with a 1.20.1 mod's item at nothing, and a player leaving (phase 5).
//!
//! The click runs 1.20.1's own `ServerPlayerGameMode.useItem` in the runtime: the item's own
//! use (a wand, a backpack, a guide book opening its screen) and Fabric's `UseItemCallback`. A
//! use held over many ticks (eating, a bow, a shield) is passed back: Horizon runs those itself
//! for the content items it knows.

use super::breaking::stack_to_120;
use super::{Level, NAMESPACES, stack_from_120, tracing, uuid_string, wait_for_runtime};
use horizon_jvm_remote::mirror120::UseItem;

impl super::super::Game<'_> {
    /// Whether the stack in that hand is one of the 1.20.1 mods' items.
    fn java120_item_in(&self, off_hand: bool) -> bool {
        let held = if off_hand {
            self.offhand.as_ref()
        } else {
            self.inventory.get(self.selected).and_then(Option::as_ref)
        };
        let Some(held) = held else {
            return false;
        };
        let namespaces = NAMESPACES.lock().unwrap_or_else(|e| e.into_inner());
        held.item
            .split_once(':')
            .is_some_and(|(ns, _)| namespaces.as_ref().is_some_and(|n| n.contains(ns)))
    }

    /// The player acting now right-clicks with what's in that hand at nothing (the head of
    /// `ServerPlayerGameMode.useItem`, where the 26.3 bridge asks too). `Some(swing)` when the
    /// 1.20.1 mods took it; `None` for Horizon to go on.
    pub(in crate::game) fn java120_use_item(&mut self, off_hand: bool) -> Option<bool> {
        if self.remote.is_some() || !self.java120_item_in(off_hand) {
            return None;
        }
        let held = if off_hand {
            self.offhand.as_ref()
        } else {
            self.inventory.get(self.selected).and_then(Option::as_ref)
        };
        let request = UseItem {
            uuid: self.java120_actor(),
            name: self.acting_player_name().to_owned(),
            dimension: self.dim.id().to_owned(),
            off_hand,
            held: held.map(stack_to_120).unwrap_or_default(),
            creative: self.mode == horizon_client::command::args::GameMode::Creative,
            stance: self.java120_stance(),
        };
        let used = {
            let mut guard = wait_for_runtime()?;
            let runtime = guard.as_mut()?;
            runtime.use_item(&Level(self), &request)
        };
        trace120!("using {}: {used:?}", request.held);
        let used = match used {
            Ok(used) => used,
            Err(e) => {
                log::warn!("1.20.1 mods: using {} failed: {e}", request.held);
                return None;
            }
        };
        if used.result == 0 {
            return None;
        }
        let dimension = self.dim.id().to_owned();
        self.apply_java120(&dimension, &used.set, &used.block_entities, &used.spawned);
        if let Some(opened) = &used.opened {
            self.java120_opened(&request.uuid, &opened.kind);
        }
        self.java120_held_after(off_hand, &request.held, &used.held);
        Some(used.result == 2)
    }

    /// The stack in that hand as the mods' code left it (one used up, one worn, a bucket
    /// filled); one that doesn't read back stays as it was rather than vanish.
    pub(super) fn java120_held_after(&mut self, off_hand: bool, before: &str, after: &str) {
        if after == before {
            return;
        }
        let after = if after.is_empty() {
            Some(None)
        } else {
            stack_from_120(after).map(Some)
        };
        if let Some(after) = after {
            if off_hand {
                self.offhand = after;
            } else if let Some(slot) = self.inventory.get_mut(self.selected) {
                *slot = after;
            }
        }
    }

    /// Player `uuid` left the game: the 1.20.1 mods hear it, their screen there closes.
    pub(in crate::game) fn java120_player_left(&mut self, conn: u32, uuid: u128) {
        super::screens::forget(conn);
        if self.remote.is_some() {
            return;
        }
        let uuid = uuid_string(uuid);
        let left = {
            let Some(mut guard) = wait_for_runtime() else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            runtime.player_left(&Level(self), &uuid)
        };
        if let Err(e) = left {
            log::warn!("1.20.1 mods: {uuid} leaving: {e}");
        }
    }
}
