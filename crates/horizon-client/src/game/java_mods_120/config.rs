//! The 1.20.1 mods' settings screens (phase 5h), on the Mods page beside the 26.3 mods'. The
//! runtime lists the mods with one (Mod Menu's entrypoints, `Runtime120::config_mods`); the
//! server tells each player's game the list once ([`CHANNEL`]), which adds them to the page;
//! the page's "modconfig" input for one of them opens it in the runtime for that player, and
//! from there it is one of their screens like any mod's own (drawn, clicked, typed into).

use super::super::Game;
use super::super::lan::LanHost;
use super::{Level, RUNTIME, wait_for_runtime};
use horizon_protocol::packets::configuration::CustomPayload;
use serde_json::Value;
use std::collections::BTreeSet;
use std::sync::Mutex;

/// Server to game, once: `[[id, name]...]` of the 1.20.1 mods with a settings screen.
pub(in crate::game) const CHANNEL: &str = "horizon:java120_config_mods";

struct Told {
    /// The list, read once from the runtime.
    mods: Option<Vec<(String, String)>>,
    conns: BTreeSet<u32>,
}

static TOLD: Mutex<Told> = Mutex::new(Told {
    mods: None,
    conns: BTreeSet::new(),
});

impl Game<'_> {
    /// Tells each player's game, once, which 1.20.1 mods have a settings screen.
    pub(in crate::game) fn tell_java120_config(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let Some(host) = self.lan.as_ref() else {
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
        if told.mods.is_none() {
            let Ok(mut guard) = RUNTIME.try_lock() else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            told.mods = Some(runtime.config_mods().unwrap_or_else(|e| {
                log::warn!("1.20.1 mods: their settings screens can't be listed: {e}");
                Vec::new()
            }));
        }
        let mods = told.mods.clone().unwrap_or_default();
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        for conn in waiting {
            told.conns.insert(conn);
            if mods.is_empty() {
                continue;
            }
            host.hub.send_to(
                conn,
                &horizon_protocol::packets::play::Clientbound::CustomPayload(CustomPayload {
                    channel: CHANNEL.into(),
                    data: serde_json::to_vec(&mods).unwrap_or_default(),
                }),
            );
        }
    }

    /// The game: the 1.20.1 mods with a settings screen, added to the Mods page's list.
    pub(in crate::game) fn remote_java120_config_mods(&mut self, data: &[u8]) {
        let Ok(mods) = serde_json::from_slice::<Vec<(String, String)>>(data) else {
            return;
        };
        let list = &mut self.settings.mods.java;
        for m in mods {
            if !list.iter().any(|(id, _)| *id == m.0) {
                list.push(m);
            }
        }
        list.sort_by_key(|m| m.1.to_lowercase());
    }

    /// The server: `conn`'s player asked for a mod's settings from the Mods page. False when
    /// it isn't a 1.20.1 mod's (the 26.3 bridge's, then).
    pub(in crate::game) fn java120_open_config(
        &mut self,
        host: &mut LanHost,
        conn: u32,
        input: &Value,
    ) -> bool {
        let Some(id) = input.get("mod").and_then(Value::as_str) else {
            return false;
        };
        let theirs = TOLD
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .mods
            .as_ref()
            .is_some_and(|mods| mods.iter().any(|(m, _)| m == id));
        if !theirs {
            return false;
        }
        let Some(guest) = host.hub.guests.iter().find(|g| g.conn == conn) else {
            return true;
        };
        let (uuid, name) = (super::uuid_string(guest.uuid), guest.name.clone());
        let id = id.to_owned();
        self.as_guest(host, conn, |g| {
            let opened = {
                let mut guard = wait_for_runtime()?;
                let runtime = guard.as_mut()?;
                runtime.open_config(&Level(g), &uuid, &name, &id)
            };
            match opened {
                Ok(Some(kind)) => g.java120_opened(&uuid, &kind),
                Ok(None) => log::warn!("1.20.1 mods: {id}'s settings didn't open"),
                Err(e) => log::warn!("1.20.1 mods: {id}'s settings didn't open: {e}"),
            }
            Some(())
        });
        true
    }
}
