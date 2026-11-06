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
    /// Mods' client half runs here for this game's player (the mods the server doesn't run,
    /// or every mod): their HUD, client-only screens and key bindings are this JVM's.
    own: bool,
    /// The own client runs every mod (the server runs no Java mods, or
    /// `HORIZON_JAVA_OWN_CLIENT`): the server's HUD, overlay, keys and settings list are unused.
    /// Else it runs only the mods the server lacks, beside the server's.
    all: bool,
    /// The Java mods the server said it runs ([`horizon_jvm::device::SERVER_MODS_CHANNEL`]).
    server_mods: Option<String>,
    /// The own client's key bindings (beside the server's, when it runs only some mods).
    keys: Vec<i32>,
    /// The own client's mods with a settings screen (beside the server's).
    config: Vec<String>,
    /// The player's UUID as Java writes it (the own client's input is theirs).
    uuid: String,
    /// The own client's client-only screen is the one shown.
    screen: bool,
    /// The inventory's hash as the own client last had it.
    inventory: u64,
}

static DEVICE: Mutex<Device> = Mutex::new(Device {
    joined: None,
    mods: Vec::new(),
    own: false,
    all: false,
    server_mods: None,
    keys: Vec::new(),
    config: Vec::new(),
    uuid: String::new(),
    screen: false,
    inventory: 0,
});

/// Whether this game's own JVM runs mods' client half for its player (connected to another's
/// server): every mod's, or those the server doesn't run.
pub(super) fn own_client() -> bool {
    let device = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
    device.own && device.joined.is_some()
}

/// Whether this game's own JVM runs every mod's client half: the server's HUD, overlay, keys
/// and settings list are then not used.
pub(super) fn own_all() -> bool {
    let device = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
    device.own && device.all && device.joined.is_some()
}

/// Whether this game's own JVM runs only the mods the server lacks, beside the server's.
fn own_some() -> bool {
    let device = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
    device.own && !device.all && device.joined.is_some()
}

/// The server's Java mods ([`horizon_jvm::device::SERVER_MODS_CHANNEL`]), as it greets this game.
pub(super) fn server_mods(data: &[u8]) {
    let mods = String::from_utf8_lossy(data).into_owned();
    log::info!(
        "Java mods: the server runs {} Java mods",
        mods.split(',').filter(|m| !m.is_empty()).count()
    );
    DEVICE.lock().unwrap_or_else(|e| e.into_inner()).server_mods = Some(mods);
}

/// The own client's key bindings when it runs beside the server's mods (to add to theirs).
pub(super) fn own_keys() -> Vec<i32> {
    let device = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
    if device.own && !device.all && device.joined.is_some() {
        device.keys.clone()
    } else {
        Vec::new()
    }
}

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

/// The device client's HUD (its mods' drawing), drawn over the game's HUD.
static HUD: Mutex<Vec<Value>> = Mutex::new(Vec::new());
/// The game's GUI size, which the device client lays its HUD and screens out in.
static GUI_SIZE: Mutex<(i32, i32)> = Mutex::new((0, 0));
/// The device client's screen is the one shown (a client-only screen of this game's own).
static SCREEN_OPEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether the open client-only screen is the device client's (its input goes there).
pub(super) fn screen_open() -> bool {
    SCREEN_OPEN.load(std::sync::atomic::Ordering::Relaxed)
}

/// The GUI's size as the game draws its HUD (`draw_java_hud`).
pub(super) fn gui_size(width: i32, height: i32) {
    *GUI_SIZE.lock().unwrap_or_else(|e| e.into_inner()) = (width, height);
}

/// The device client's HUD operations (empty: none).
pub(super) fn hud_ops() -> Vec<Value> {
    HUD.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// A key while playing, to the device client too (its mods' key bindings and handlers).
pub(super) fn key(key: i32, keycode: i32, mods: i32, down: bool) {
    if DEVICE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .mods
        .is_empty()
    {
        return;
    }
    if let Err(e) = horizon_jvm::device::key(key, keycode, mods, down) {
        log::warn!("Java mods: a key to the device client failed: {e}");
    }
}

/// Whether a server's custom payload is for the device client here (a device mod's channel).
pub(super) fn takes(channel: &str) -> bool {
    let device = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
    if device.own {
        // Every mod's channel (Fabric API's too): its client half is here.
        return device.joined.is_some()
            && !channel.starts_with("minecraft:")
            && !channel.starts_with("horizon:");
    }
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
    SCREEN_OPEN.store(false, std::sync::atomic::Ordering::Relaxed);
    HUD.lock().unwrap_or_else(|e| e.into_inner()).clear();
    let mut device = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
    device.own = false;
    device.all = false;
    device.server_mods = None;
    device.keys.clear();
    device.config.clear();
    device.screen = false;
    device.inventory = 0;
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
            // What the server said it runs: none by now (a server with no Java mods, a vanilla
            // one) and every mod's client half runs here.
            let server_mods = DEVICE
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .server_mods
                .clone();
            match horizon_jvm::device::join(&host, port, &uuid, &name, server_mods.as_deref()) {
                Ok(None) => return, // Java is still starting.
                Ok(Some(mut mods)) => {
                    let mut device = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
                    device.joined = Some(key);
                    let marker = mods.first().cloned();
                    device.own = matches!(marker.as_deref(), Some("*" | "+"));
                    device.all = marker.as_deref() == Some("*");
                    // The server is told the marker too (every mod's, or these).
                    let data = mods.join(",").into_bytes();
                    if device.own {
                        mods.remove(0);
                    }
                    device.uuid = uuid.clone();
                    device.mods = mods;
                    if device.mods.is_empty() {
                        return;
                    }
                    let own = device.own;
                    drop(device);
                    self.send(S::CustomPayload(CustomPayload {
                        channel: horizon_jvm::device::RUNS_OWN_CHANNEL.into(),
                        data,
                    }));
                    if own {
                        self.own_client_greet();
                    }
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
        if own_client() {
            self.tick_own_client();
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
        let (w, h) = *GUI_SIZE.lock().unwrap_or_else(|e| e.into_inner());
        let state = json!({
            "x": p.position.x, "y": p.position.y, "z": p.position.z,
            "yaw": p.yaw, "pitch": p.pitch, "players": players, "w": w, "h": h,
        });
        match horizon_jvm::device::tick(&state.to_string()) {
            Ok(shown) => self.show_device(&shown),
            Err(e) => log::warn!("Java mods: the device client's tick failed: {e}"),
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

    /// What the server's games are told as they join, from this game's own client: the keys
    /// its mods bind, the mods with a settings screen, and that the inventory overlay is here.
    fn own_client_greet(&mut self) {
        if own_some() {
            // Beside the server's mods: their keys and settings screens are added to the
            // server's; the inventory overlay stays the server's.
            self.own_client_keys();
            if let Ok(mods) = horizon_jvm::events::config_mods() {
                DEVICE.lock().unwrap_or_else(|e| e.into_inner()).config =
                    mods.iter().map(|(id, _)| id.clone()).collect();
                for m in mods {
                    if !self.settings.mods.java.iter().any(|(id, _)| *id == m.0) {
                        self.settings.mods.java.push(m);
                    }
                }
            }
            return;
        }
        match horizon_jvm::events::bound_keys() {
            Ok(keys) => self.remote_java_keys(&serde_json::to_vec(&keys).unwrap_or_default()),
            Err(e) => log::warn!("Java mods: this game's own client's key bindings: {e}"),
        }
        if let Ok(mappings) = horizon_jvm::events::key_mappings() {
            self.remote_java_key_mappings(mappings.as_bytes());
        }
        match horizon_jvm::events::config_mods() {
            Ok(mods) => {
                self.remote_java_config_mods(&serde_json::to_vec(&mods).unwrap_or_default())
            }
            Err(e) => log::warn!("Java mods: this game's own client's settings screens: {e}"),
        }
        self.remote_java_overlay(b"[]");
    }

    /// The player's inventory as the mirror takes it (vanilla's slots, SNBT each), when it
    /// changed since the own client last had it.
    fn own_client_inventory(&self) -> Option<Vec<String>> {
        use std::hash::{Hash, Hasher};
        let slots: Vec<String> = (0..horizon_jvm::events::INVENTORY_SLOTS)
            .map(|v| {
                let stack = match v {
                    0..=35 => self.inventory.get(v).cloned().flatten(),
                    36..=39 => self.armor[39 - v].clone(),
                    _ => self.offhand.clone(),
                };
                stack.map_or_else(String::new, |stack| {
                    horizon_client::snbt::to_snbt(&horizon_items::item_nbt::item_to_nbt(&stack))
                })
            })
            .collect();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        slots.hash(&mut hasher);
        let hash = hasher.finish();
        let mut device = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
        (std::mem::replace(&mut device.inventory, hash) != hash).then_some(slots)
    }

    /// Each tick of a game whose own JVM runs every mod's client half: its player where this
    /// game sees it, over this game's world (the chunks the server sent), the client's tick
    /// and frame there; what it draws shows here, what it sends goes to the server.
    fn tick_own_client(&mut self) {
        // Its key bindings, again now and then (mods register some as their client starts).
        if self.ticks.is_multiple_of(100) && own_some() {
            self.own_client_keys();
        } else if self.ticks.is_multiple_of(100)
            && let Ok(keys) = horizon_jvm::events::bound_keys()
        {
            self.remote_java_keys(&serde_json::to_vec(&keys).unwrap_or_default());
            if let Ok(mappings) = horizon_jvm::events::key_mappings() {
                self.remote_java_key_mappings(mappings.as_bytes());
            }
        }
        let Some(p) = self.player.as_ref() else {
            return;
        };
        let mode = match self.mode {
            horizon_client::command::args::GameMode::Survival => 0,
            horizon_client::command::args::GameMode::Creative => 1,
            horizon_client::command::args::GameMode::Adventure => 2,
            horizon_client::command::args::GameMode::Spectator => 3,
        };
        let sneaking = self.keys.contains(&winit::keyboard::KeyCode::ShiftLeft);
        let (w, h) = *GUI_SIZE.lock().unwrap_or_else(|e| e.into_inner());
        let mut state = json!({
            "tick": self.ticks, "dim": self.dim.id(),
            "x": p.position.x, "y": p.position.y, "z": p.position.z,
            "yaw": p.yaw, "pitch": p.pitch, "mode": mode, "health": p.health.health(),
            "food": p.health.food.food, "sat": p.health.food.saturation,
            "perm": i32::from(sneaking) << 8, "w": w, "h": h,
        });
        if let Some(spawn) = self.spawn {
            state["spawn"] = json!([
                spawn.x.floor() as i32,
                spawn.y.floor() as i32,
                spawn.z.floor() as i32
            ]);
        }
        if let Some(inventory) = self.own_client_inventory() {
            state["inv"] = json!(inventory);
        }
        let state = state.to_string();
        if let Err(e) = self.with_java_world(|| horizon_jvm::device::tick(&state)) {
            log::warn!("Java mods: this game's own client's tick failed: {e}");
        }
        self.own_client_shows();
    }

    /// The own client's key bindings beside the server's mods': added to the server's.
    fn own_client_keys(&mut self) {
        let Ok(keys) = horizon_jvm::events::bound_keys() else {
            return;
        };
        self.java.binds.keys.extend(keys.iter().copied());
        DEVICE.lock().unwrap_or_else(|e| e.into_inner()).keys = keys;
    }

    /// Input for the own client (its HUD's size, the inventory overlay's, its client-only
    /// screen's, a key binding, a settings screen asked for): run here, for this player.
    pub(super) fn own_client_input(&mut self, input: &Value) {
        let uuid = DEVICE
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .uuid
            .clone();
        let input = input.to_string();
        if let Err(e) = self.with_java_world(|| horizon_jvm::events::screen_input(&uuid, &input)) {
            log::warn!("Java mods: this game's own client's input failed: {e}");
        }
        self.own_client_shows();
    }

    /// Whether this input goes to the own client (only): all but a server menu's window (its
    /// screen is the server's, made where the menu is). Beside the server's mods: its own
    /// client-only screen and its mods' settings screens; keys and the HUD's size go to both
    /// ([`Self::own_client_also`]), the inventory overlay to the server.
    pub(super) fn own_client_takes(input: &Value) -> bool {
        let device = DEVICE.lock().unwrap_or_else(|e| e.into_inner());
        if !(device.own && device.joined.is_some()) {
            return false;
        }
        let kind = input.get("kind").and_then(Value::as_str);
        let window = input.get("window").and_then(Value::as_i64).unwrap_or(-1) as i32;
        if window == super::java_screen::CLIENT_WINDOW && device.screen {
            return true;
        }
        if !device.all {
            let config = input.get("mod").and_then(Value::as_str);
            return kind == Some("modconfig")
                && config.is_some_and(|m| device.config.iter().any(|c| c == m));
        }
        matches!(kind, Some("bind" | "modconfig"))
            || window == super::java_screen::HUD_WINDOW
            || window == super::java_screen::OVERLAY_WINDOW
    }

    /// Whether this input goes to the own client as well as to the server: its mods run beside
    /// the server's, and both hear the keys and the HUD's size.
    pub(super) fn own_client_also(input: &Value) -> bool {
        if !own_some() {
            return false;
        }
        let window = input.get("window").and_then(Value::as_i64).unwrap_or(-1) as i32;
        input.get("kind").and_then(Value::as_str) == Some("bind")
            || window == super::java_screen::HUD_WINDOW
    }

    /// What the own client made since: its HUD, overlay, client-only screen and chat lines
    /// shown here; its messages to the server sent.
    fn own_client_shows(&mut self) {
        let window = super::java_screen::CLIENT_WINDOW;
        let all = own_all();
        for action in horizon_jvm::events::take_actions() {
            match action {
                horizon_jvm::events::Action::Tell { text, .. } => {
                    self.chat_line(chat_text(&text), horizon_client::chat::ChatSource::Server);
                }
                horizon_jvm::events::Action::Overlay { text, .. } => {
                    self.titles.set_overlay(chat_text(&text), false);
                }
                horizon_jvm::events::Action::HudFrame { frame, .. } if all => {
                    self.remote_java_hud(frame.as_bytes());
                }
                // Beside the server's mods: its HUD over the server's (as a device client's).
                horizon_jvm::events::Action::HudFrame { frame, .. } => {
                    if let Ok(Value::Array(ops)) = serde_json::from_str::<Value>(&frame) {
                        let ops = self.take_java_images(&ops);
                        *HUD.lock().unwrap_or_else(|e| e.into_inner()) = ops;
                    }
                }
                horizon_jvm::events::Action::View { scale, .. }
                    if all || (scale - 1.0).abs() > 1e-4 =>
                {
                    self.remote_java_view(&serde_json::to_vec(&scale).unwrap_or_default());
                }
                horizon_jvm::events::Action::OverlayFrame { frame, .. } if all => {
                    self.remote_java_overlay(frame.as_bytes());
                }
                horizon_jvm::events::Action::ClientScreen { open, .. } => {
                    DEVICE.lock().unwrap_or_else(|e| e.into_inner()).screen = open;
                    if !open {
                        let close = json!({"window": window, "close": true}).to_string();
                        self.remote_java_screen(close.as_bytes());
                    }
                }
                horizon_jvm::events::Action::ScreenFrame { frame, .. }
                    if DEVICE.lock().unwrap_or_else(|e| e.into_inner()).screen =>
                {
                    let frame = format!("{{\"window\":{window},\"ops\":{frame}}}");
                    self.remote_java_screen(frame.as_bytes());
                }
                _ => {}
            }
        }
        for (channel, data) in horizon_jvm::device::take_out() {
            self.send(S::CustomPayload(CustomPayload { channel, data }));
        }
    }

    /// What the device client shows (its tick's or its input's answer): its HUD, its screen
    /// (a client-only screen of this game: `Screen::JavaMod`), its screen closing.
    fn show_device(&mut self, shown: &str) {
        let Ok(shown) = serde_json::from_str::<Value>(shown) else {
            return;
        };
        let window = super::java_screen::CLIENT_WINDOW;
        if shown.get("close").and_then(Value::as_bool) == Some(true)
            && SCREEN_OPEN.swap(false, std::sync::atomic::Ordering::Relaxed)
        {
            let close = json!({"window": window, "close": true}).to_string();
            self.remote_java_screen(close.as_bytes());
        }
        if let Some(Value::Array(ops)) = shown.get("hud") {
            let ops = self.take_java_images(ops);
            *HUD.lock().unwrap_or_else(|e| e.into_inner()) = ops;
        }
        if let Some(ops) = shown.get("screen") {
            SCREEN_OPEN.store(true, std::sync::atomic::Ordering::Relaxed);
            let frame = json!({"window": window, "ops": ops}).to_string();
            self.remote_java_screen(frame.as_bytes());
        }
    }

    /// Input to the device client's screen (`send_java_screen_input` while it is the one shown).
    pub(super) fn device_screen_input(&mut self, input: &Value) {
        match horizon_jvm::device::input(&input.to_string()) {
            Ok(shown) => self.show_device(&shown),
            Err(e) => log::warn!("Java mods: the device client's screen input failed: {e}"),
        }
        if input.get("kind").and_then(Value::as_str) == Some("close") {
            SCREEN_OPEN.store(false, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// Server: a player's game runs some mods' client half itself (`data`: which, as it said).
    pub(super) fn java_device_runs_own(&mut self, host: &mut LanHost, conn: u32, data: &[u8]) {
        let Some(uuid) = host
            .hub
            .guests
            .iter()
            .find(|g| g.conn == conn)
            .map(|g| g.uuid)
        else {
            return;
        };
        let mods = String::from_utf8_lossy(data);
        if let Err(e) = horizon_jvm::device::runs_their_own(&uuid_string(uuid), &mods) {
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
