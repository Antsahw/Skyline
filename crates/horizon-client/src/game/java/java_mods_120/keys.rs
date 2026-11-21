//! The 1.20.1 mods' key bindings (phase 5): which keys they bind goes to each player's game
//! once ([`KEYS_120`]); the game then sends those keys while playing as the 26.3 bridge's are
//! (`{"kind": "bind", "key": scancode, "down": bool}` on the screen input channel), and each
//! press goes to the runtime, where the player's next client tick (in `server_tick`) clicks
//! the mod's mapping as 1.20.1's keyboard handler would.

use super::super::lan::LanHost;
use super::screens::glfw_key;
use super::{Level, uuid_string, wait_for_runtime};
use crate::game::java_screen::KEYS_120;
use horizon_protocol::packets::configuration::CustomPayload;
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::Mutex;

/// The keys told (as the game's scancodes), once read; and the players' games told them.
struct Told {
    keys: Option<Vec<i32>>,
    conns: BTreeSet<u32>,
}
static TOLD: Mutex<Told> = Mutex::new(Told {
    keys: None,
    conns: BTreeSet::new(),
});

/// The game's scancode for GLFW's key code, the one [`glfw_key`] turns into that code.
fn scancode(glfw: i32) -> Option<i32> {
    (0..128).find(|&sdl| glfw_key(i64::from(sdl)) == glfw)
}

impl super::super::Game<'_> {
    /// Tells each player's game, once, which keys the 1.20.1 mods bind (mods register theirs
    /// as they load, so the list is read once).
    pub(in crate::game) fn tell_java120_keys(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let mut told = TOLD.lock().unwrap_or_else(|e| e.into_inner());
        let waiting: Vec<u32> = host
            .hub
            .guests
            .iter()
            .filter(|g| g.placed && !told.conns.contains(&g.conn))
            .map(|g| g.conn)
            .collect();
        if waiting.is_empty() {
            return;
        }
        if told.keys.is_none() {
            let Ok(mut guard) = super::RUNTIME.try_lock() else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            let keys = match runtime.mod_keys(&Level(self)) {
                Ok(keys) => keys,
                Err(e) => {
                    log::warn!("1.20.1 mods: their key bindings can't be read: {e}");
                    Vec::new()
                }
            };
            told.keys = Some(keys.into_iter().filter_map(scancode).collect());
        }
        let keys = told.keys.clone().unwrap_or_default();
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        for conn in waiting {
            told.conns.insert(conn);
            if !keys.is_empty() {
                send_keys(host, conn, &keys);
            }
        }
    }

    /// A key the 1.20.1 mods bind, pressed or let go by `conn`'s player while playing. The
    /// 26.3 bridge takes the same input too, for its mods' keys.
    pub(in crate::game) fn java120_key(&mut self, host: &mut LanHost, conn: u32, input: &Value) {
        let (Some(key), Some(down)) = (
            input.get("key").and_then(Value::as_i64),
            input.get("down").and_then(Value::as_bool),
        ) else {
            return;
        };
        let glfw = glfw_key(key);
        if glfw < 0 {
            return;
        }
        let bound = TOLD
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .keys
            .as_ref()
            .is_some_and(|keys| keys.contains(&(key as i32)));
        if !bound {
            return;
        }
        let Some(guest) = host.hub.guests.iter().find(|g| g.conn == conn) else {
            return;
        };
        let (uuid, name) = (uuid_string(guest.uuid), guest.name.clone());
        self.as_guest(host, conn, |g| {
            let pressed = {
                let mut guard = wait_for_runtime()?;
                let runtime = guard.as_mut()?;
                runtime.key_press(&Level(g), &uuid, &name, glfw, down)
            };
            if let Err(e) = pressed {
                log::warn!("1.20.1 mods: a key for {uuid}: {e}");
            }
            Some(())
        });
    }
}

/// A player left: told again if they come back.
pub(super) fn forget(conn: u32) {
    TOLD.lock()
        .unwrap_or_else(|e| e.into_inner())
        .conns
        .remove(&conn);
}

fn send_keys(host: &mut LanHost, conn: u32, keys: &[i32]) {
    host.hub.send_to(
        conn,
        &horizon_protocol::packets::play::Clientbound::CustomPayload(CustomPayload {
            channel: KEYS_120.into(),
            data: serde_json::to_vec(keys).unwrap_or_default(),
        }),
    );
}
