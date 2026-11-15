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

    /// Player `uuid` died, `message` the death message's key (`death.attack.fall`): what the
    /// 1.20.1 mods inject into a player's `die` (the library mods' death events).
    pub(in crate::game) fn java120_player_died(
        &mut self,
        uuid: u128,
        name: &str,
        dimension: &str,
        message: Option<&str>,
    ) {
        if self.remote.is_some() {
            return;
        }
        let damage_type = message
            .and_then(|k| k.strip_prefix("death.attack."))
            .map_or("generic", |id| id.split('.').next().unwrap_or(id))
            .to_owned();
        let uuid = uuid_string(uuid);
        let died = {
            let Some(mut guard) = super::runtime_within(std::time::Duration::from_millis(25))
            else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            runtime.player_died(&Level(self), &uuid, name, dimension, &damage_type)
        };
        if let Err(e) = died {
            log::warn!("1.20.1 mods: {uuid} dying: {e}");
        }
    }
}

impl super::super::Game<'_> {
    /// One of Horizon's vanilla mobs died (`LivingEntity.die`, past the native mods' cancel): the
    /// 1.20.1 mods hear it, and what they drop lands. `killer`: the guest connection that hit it
    /// last, when a player did.
    pub(in crate::game) fn java120_mob_died(
        &mut self,
        kind: &str,
        pos: [f64; 3],
        damage_type: &str,
        killer: Option<u32>,
    ) {
        if self.remote.is_some() || !kind.starts_with("minecraft:") {
            return;
        }
        let killer = killer.and_then(|conn| {
            let guests = self.lan.as_ref()?.hub.guests.as_slice();
            let g = guests.iter().find(|g| g.conn == conn)?;
            Some((uuid_string(g.uuid), g.name.clone()))
        });
        let dimension = self.dim.id().to_owned();
        let died = {
            let Some(mut guard) = super::runtime_within(std::time::Duration::from_millis(25))
            else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            runtime.mob_died(
                &Level(self),
                &dimension,
                kind,
                pos,
                damage_type,
                killer.as_ref().map(|(u, n)| (u.as_str(), n.as_str())),
            )
        };
        match died {
            Ok(spawned) if !spawned.is_empty() => {
                self.apply_java120(&dimension, &[], &[], &spawned);
            }
            Ok(_) => {}
            Err(e) => log::warn!("1.20.1 mods: a {kind} dying: {e}"),
        }
    }
}

/// The players of the 1.20.1 mods' levels by entity id: their UUID, name and dimension, for
/// what Horizon's damage code asks about a player with no game at hand (`java120_hurt_player`).
static PLAYERS_120: std::sync::Mutex<
    Option<std::collections::HashMap<i32, (String, String, String)>>,
> = std::sync::Mutex::new(None);

/// Remembers who player entity `id` is (each tick, from the players the mods' mobs see).
pub(in crate::game) fn remember_player(id: i32, uuid: String, name: String, dimension: String) {
    PLAYERS_120
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(Default::default)
        .insert(id, (uuid, name, dimension));
}

impl super::super::Game<'_> {
    /// A player is about to be hurt (`LivingEntity.hurtServer` past its guards, where native
    /// mods hear it: native_hooks.rs `hurt_server`): what the 1.20.1 mods inject into its
    /// `hurt`. True cancels the hit. Never waits for a busy runtime.
    pub(in crate::game) fn java120_hurt_player(
        hurting: &horizon_client::health::Hurting<'_>,
        amount: f32,
    ) -> bool {
        let horizon_client::health::Hurting::Player(id, source) = hurting else {
            return false;
        };
        let Some((uuid, name, dimension)) = PLAYERS_120
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .and_then(|p| p.get(id).cloned())
        else {
            log::debug!("1.20.1 mods: a hit on player {id}, whom the mods haven't met");
            return false;
        };
        let Some(mut guard) = super::runtime_within(std::time::Duration::from_millis(25)) else {
            return false;
        };
        let Some(runtime) = guard.as_mut() else {
            return false;
        };
        match runtime.player_hurt(&uuid, &name, &dimension, source.damage_type, amount) {
            Ok(canceled) => canceled,
            Err(e) => {
                log::warn!("1.20.1 mods: a hit on {name}: {e}");
                false
            }
        }
    }
}
