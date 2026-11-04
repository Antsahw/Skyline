//! Device mods (Java mods whose client half uses the player's own microphone or speakers:
//! Simple Voice Chat) for a game connected to another's server: their client half runs in this
//! game's own JVM (`horizon_jvm::device`), with the player and the others where this game sees
//! them, and their messages over this connection. On a server, a player's game saying so
//! ([`horizon_jvm::device::RUNS_OWN_CHANNEL`]) gets those mods' messages, and what it sends on
//! their channels goes to the mods' server code.

use super::{Game, lan::LanHost};
use horizon_protocol::packets::{configuration::CustomPayload, play::Serverbound as S};
use serde_json::{Value, json};
use std::sync::Mutex;

/// The device client of this game: the server it joined and the mods running for it.
struct Device {
    /// `address|uuid` of the connection the device client joined (None: not joined).
    joined: Option<String>,
    /// The device mods running here (their ids: their channels' namespaces).
    mods: Vec<String>,
}

static DEVICE: Mutex<Device> = Mutex::new(Device {
    joined: None,
    mods: Vec::new(),
});

/// Ticks the player has been in the world before the device client joins: the server has
/// mirrored them by then (a message before that would find no player there).
static WAITED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
const JOIN_AFTER: u32 = 40;

fn uuid_string(u: u128) -> String {
    let h = format!("{u:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// The host and port of an address as typed (`host`, `host:port`, `[v6]:port`).
fn host_port(address: &str) -> (String, u16) {
    if let Some(rest) = address.strip_prefix('[')
        && let Some((host, tail)) = rest.split_once(']')
    {
        let port = tail.strip_prefix(':').and_then(|p| p.parse().ok());
        return (host.to_string(), port.unwrap_or(25565));
    }
    match address.rsplit_once(':') {
        Some((host, port)) if !host.contains(':') => {
            (host.to_string(), port.parse().unwrap_or(25565))
        }
        _ => (address.to_string(), 25565),
    }
}

/// Whether a server's custom payload is for the device client here (a device mod's channel).
pub(super) fn takes(channel: &str) -> bool {
    let device = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
    device.joined.is_some() && horizon_jvm::device::is_device_channel(channel, &device.mods)
}

/// A device mod's message from the server, to its client half here.
pub(super) fn payload(channel: &str, data: &[u8]) {
    if let Err(e) = horizon_jvm::device::payload(channel, data) {
        log::warn!("Java mods: a device mod's message {channel} failed: {e}");
    }
}

/// The game left the server: the device client disconnects.
pub(super) fn left() {
    WAITED.store(0, std::sync::atomic::Ordering::Relaxed);
    let mut device = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
    if device.joined.take().is_some()
        && let Err(e) = horizon_jvm::device::leave()
    {
        log::warn!("Java mods: the device client's leave failed: {e}");
    }
}

/// A Java mod's chat line (a JSON component), as the game's chat shows it.
fn chat_text(text: &str) -> horizon_client::text::Text {
    match serde_json::from_str::<Value>(text) {
        Ok(v) if v.is_object() || v.is_array() || v.is_string() => {
            horizon_client::packs::component(&v).with_component(v)
        }
        _ => horizon_client::text::Text::literal(text),
    }
}

impl Game<'_> {
    /// Each tick of a game connected to another's server (not its own server thread): the
    /// device client joins once Java is up, then ticks with where the players are; what it
    /// sends goes to the server, what it tells its player shows here.
    pub(super) fn tick_device_client(&mut self) {
        if self.integrated.is_some() || !super::java_mods::has_java_mods() {
            return;
        }
        let Some(r) = self.remote.as_ref() else {
            return;
        };
        if !r.joined || self.player.is_none() {
            return;
        }
        let key = format!("{}|{:x}", r.session.address, r.profile());
        let joined = DEVICE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .joined
            .clone();
        if joined.as_deref() != Some(key.as_str()) {
            if WAITED.fetch_add(1, std::sync::atomic::Ordering::Relaxed) < JOIN_AFTER {
                return;
            }
            let (host, port) = host_port(&r.session.address);
            let uuid = uuid_string(r.profile());
            let name = r.profile_name.clone();
            match horizon_jvm::device::join(&host, port, &uuid, &name) {
                Ok(None) => return, // Java is still starting.
                Ok(Some(mods)) => {
                    let mut device = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
                    device.joined = Some(key);
                    device.mods = mods;
                    if device.mods.is_empty() {
                        return;
                    }
                    let data = device.mods.join(",").into_bytes();
                    drop(device);
                    self.send(S::CustomPayload(CustomPayload {
                        channel: horizon_jvm::device::RUNS_OWN_CHANNEL.into(),
                        data,
                    }));
                }
                Err(e) => {
                    log::warn!("Java mods: the device client couldn't join: {e}");
                    DEVICE.lock().unwrap_or_else(|e| e.into_inner()).joined = Some(key);
                    return;
                }
            }
        }
        if DEVICE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .mods
            .is_empty()
        {
            return;
        }
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let players: Vec<Value> = self
            .remote_player_names()
            .into_iter()
            .filter_map(|(id, name)| {
                let uuid = self.remote_player_uuid(id)?;
                let m = self.mobs.iter().find(|m| m.id == id)?;
                Some(json!({
                    "u": uuid_string(uuid), "n": name,
                    "x": m.position.x, "y": m.position.y, "z": m.position.z, "yaw": m.yaw,
                }))
            })
            .collect();
        let state = json!({
            "x": p.position.x, "y": p.position.y, "z": p.position.z,
            "yaw": p.yaw, "pitch": p.pitch, "players": players,
        });
        if let Err(e) = horizon_jvm::device::tick(&state.to_string()) {
            log::warn!("Java mods: the device client's tick failed: {e}");
        }
        for (channel, data) in horizon_jvm::device::take_out() {
            self.send(S::CustomPayload(CustomPayload { channel, data }));
        }
        // What the device mods tell their player (this process has no server to apply the
        // rest: the bridge's other actions are a server's).
        for action in horizon_jvm::events::take_actions() {
            match action {
                horizon_jvm::events::Action::Tell { text, .. } => {
                    self.chat_line(chat_text(&text), horizon_client::chat::ChatSource::Server);
                }
                horizon_jvm::events::Action::Overlay { text, .. } => {
                    self.titles.set_overlay(chat_text(&text), false);
                }
                _ => {}
            }
        }
    }

    /// Server: a player's game runs the device mods' client half itself.
    pub(super) fn java_device_runs_own(&mut self, host: &mut LanHost, conn: u32) {
        let Some(uuid) = host
            .hub
            .guests
            .iter()
            .find(|g| g.conn == conn)
            .map(|g| g.uuid)
        else {
            return;
        };
        if let Err(e) = horizon_jvm::device::runs_their_own(&uuid_string(uuid)) {
            log::warn!("Java mods: {e}");
        }
    }

    /// Server: a player's game sent a custom payload Horizon has no use for: a device mod's
    /// (Java takes it if so).
    pub(super) fn java_device_from_game(
        &mut self,
        host: &mut LanHost,
        conn: u32,
        channel: &str,
        data: &[u8],
    ) {
        if !super::java_mods::has_java_mods() || channel.starts_with("minecraft:") {
            return;
        }
        let Some(uuid) = host
            .hub
            .guests
            .iter()
            .find(|g| g.conn == conn)
            .map(|g| g.uuid)
        else {
            return;
        };
        if let Err(e) = horizon_jvm::device::from_game(&uuid_string(uuid), channel, data) {
            log::warn!("Java mods: a player's message on {channel} failed: {e}");
        }
    }

    /// Server: the player whose game this process is (the single-player world's owner), told
    /// to Java when it changes.
    pub(super) fn java_device_own_player(&mut self, owner: Option<u128>) {
        static TOLD: Mutex<Option<Option<u128>>> = Mutex::new(None);
        let mut told = TOLD.lock().unwrap_or_else(|e| e.into_inner());
        if *told == Some(owner) || !horizon_jvm::events::ready() {
            return;
        }
        let uuid = owner.map(uuid_string);
        if horizon_jvm::device::own_player(uuid.as_deref()).is_ok() {
            *told = Some(owner);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_split_into_host_and_port() {
        assert_eq!(host_port("localhost:25599"), ("localhost".into(), 25599));
        assert_eq!(host_port("example.org"), ("example.org".into(), 25565));
        assert_eq!(host_port("[::1]:25570"), ("::1".into(), 25570));
        assert_eq!(host_port("::1"), ("::1".into(), 25565));
    }
}
