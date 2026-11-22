//! What server mods (Java mods) change about the guests: their commands, abilities,
//! tab-list names and inventory slots.
use super::*;

impl Hub {
    /// The mods' commands as last set (a root node, or null).
    pub fn mod_commands(&self) -> &serde_json::Value {
        &self.mod_commands.0
    }
    /// Sends guest `i`'s inventory-menu `slots` (`Guest::menu_slot` ids) as they are now,
    /// for a change made outside a click's answer (Java mods' crafted hooks during a click).
    pub fn send_inventory_slots(&mut self, i: usize, slots: &[usize]) {
        for &slot in slots {
            let now = self.guests[i].menu_slot(slot);
            let net = self.stack(&now);
            self.guests.update(i, |g| {
                g.menu_state = (g.menu_state + 1) & 0x7FFF;
                g.send(&C::ContainerSetSlot(play::ContainerSetSlot {
                    container: 0,
                    state: g.menu_state,
                    slot: slot as i16,
                    item: net,
                }));
                g.menu_sent[slot] = now;
            });
        }
    }

    /// A server mod (a Java mod) changed a guest's abilities: `/fly`, `/invuln`, speeds.
    /// ServerPlayer.onUpdateAbilities: the guest is told at once.
    pub fn set_mod_abilities(&mut self, conn: u32, abilities: super::super::guest::ModAbilities) {
        let _ = self.guests.update_connection(conn, |g| {
            if g.game_mode != abilities.game_mode {
                return;
            }
            let mut now = g.abilities;
            now.invulnerable = abilities.flags & 1 != 0;
            now.may_fly = abilities.flags & 4 != 0;
            now.instabuild = abilities.flags & 8 != 0;
            now.flying = now.may_fly && (now.flying || abilities.flags & 2 != 0);
            now.flying_speed = abilities.flying_speed;
            now.walking_speed = abilities.walking_speed;
            if now == g.abilities {
                return;
            }
            g.abilities = now;
            g.send(&g.abilities_packet());
        });
    }

    /// A server mod (a Java mod's nickname) changed the name a guest shows in the tab list:
    /// everyone is told (PlayerInfo UPDATE_DISPLAY_NAME).
    pub fn set_tab_name(&mut self, uuid: u128, name: Option<Tag>) {
        let Some(i) = self.guests.iter().position(|g| g.uuid == uuid) else {
            return;
        };
        if self.guests[i].tab_name == name {
            return;
        }
        let stored = name.clone();
        self.guests.update(i, |g| g.tab_name = stored);
        self.broadcast(&C::PlayerInfoUpdate(play::PlayerInfoUpdate {
            actions: 1 << 5,
            entries: vec![play::PlayerInfo {
                uuid,
                display_name: Some(name.or_else(|| self.nicknames.get(&uuid).map(|n| literal(n)))),
                ..Default::default()
            }],
        }));
    }
}
