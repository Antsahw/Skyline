//! Java mods' menus (windows the game doesn't run) and their screens' input from players.
use super::*;
use horizon_protocol::packets::play::{self};
use serde_json::Value;

/// Items Java mods dropped, by dimension, made when their level is in place.
/// The windows of Java mods' menus, by player: (connection, window).
pub(super) static JAVA_WINDOWS: std::sync::Mutex<std::collections::BTreeMap<String, (u32, i32)>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// The menu window under a player's client-only screen (JEI's recipes over a mod's
/// menu), by player: back in [`JAVA_WINDOWS`] when the client-only screen closes.
pub(super) static JAVA_UNDER: std::sync::Mutex<std::collections::BTreeMap<String, (u32, i32)>> =
    std::sync::Mutex::new(std::collections::BTreeMap::new());

/// The player whose Java mod's menu has `window` open on `conn`.
pub(super) fn java_window_of(host: &LanHost, conn: u32, window: i32) -> Option<String> {
    let uuid = host
        .hub
        .guests
        .iter()
        .find(|g| g.conn == conn)
        .map(|g| uuid_string(g.uuid))?;
    let windows = JAVA_WINDOWS.lock().unwrap_or_else(|e| e.into_inner());
    (windows.get(&uuid) == Some(&(conn, window))).then_some(uuid)
}

impl Game<'_> {
    /// Opens Horizon's own menu for one a Java mod opened (`ServerPlayer.openMenu`):
    /// a crafting grid, the player's ender chest, a stonecutter, grindstone, loom,
    /// cartography or smithing table, with no block behind it (`/workbench`). Other
    /// menus are the mod's own; Horizon can't show those yet.
    pub(super) fn java_open_menu(
        &mut self,
        host: &mut LanHost,
        uuid: &str,
        menu: &str,
        title: Option<&str>,
    ) {
        use super::super::stations::{Kind, Station};
        let Some(g) = host.hub.guests.iter().find(|g| uuid_string(g.uuid) == uuid) else {
            return;
        };
        let conn = g.conn;
        let at = g.position.map(|v| v.floor() as i32);
        let ender_items = g.ender_items.clone();
        let title_json = title
            .and_then(|j| serde_json::from_str::<serde_json::Value>(j).ok())
            .unwrap_or(serde_json::Value::Null);
        let station = match menu {
            "minecraft:stonecutter" => Some(Kind::Stonecutter),
            "minecraft:grindstone" => Some(Kind::Grindstone),
            "minecraft:loom" => Some(Kind::Loom),
            "minecraft:cartography_table" => Some(Kind::Cartography),
            "minecraft:smithing" => Some(Kind::Smithing),
            _ => None,
        };
        if menu == "minecraft:crafting" {
            self.lan_open_crafting(host, conn, at);
        } else if let Some(kind) = station {
            self.lan_open_station(host, conn, &Station::new(kind, at));
        } else if menu == "minecraft:generic_9x3"
            && title_json.to_string().contains("container.enderchest")
        {
            // PlayerEnderChestContainer: the guest's own ender chest items.
            self.guest_container_closed(host, conn);
            self.lan_station_closed(host, conn, None);
            self.lan_crafting_closed(host, conn, None);
            host.hub.open_unplaced_container(
                conn,
                menu,
                horizon_client::lan::data::component_nbt(&title_json),
                horizon_client::lan::menu::Slots::Ender,
                ender_items,
            );
        } else {
            static TOLD: std::sync::OnceLock<std::sync::Mutex<std::collections::HashSet<String>>> =
                std::sync::OnceLock::new();
            let told = TOLD.get_or_init(Default::default);
            if told
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(menu.to_string())
            {
                log::info!(
                    "Java mods: a mod opened a {menu} menu; Horizon shows only vanilla's own menus yet"
                );
            }
            return;
        }
        let window = host.hub.guest(conn).map(|g| g.next_window);
        host.play.entry(conn).or_default().mod_window = window;
    }

    /// A click in the window of a Java mod's menu (a cabinet's): the menu runs it, in Java;
    /// false for other windows.
    pub(in crate::game) fn java_menu_click(
        &mut self,
        host: &mut LanHost,
        conn: u32,
        p: &play::ContainerClick,
    ) -> bool {
        {
            let Some(uuid) = java_window_of(host, conn, p.container) else {
                return false;
            };
            let (slot, button, input) = (i32::from(p.slot), i32::from(p.button), p.input);
            let clicked = self
                .with_java_world(|| horizon_jvm::events::menu_click(&uuid, slot, button, input));
            if let Err(e) = clicked {
                log::warn!("Java mods: a click in a mod's menu failed: {e}");
            }
            self.apply_java_actions(Some(host));
            true
        }
    }

    /// Input to the mod screen of a player's Java window (game/java_screen.rs's INPUT): the
    /// screen's own code runs it.
    pub(in crate::game) fn java_screen_input(
        &mut self,
        host: &mut LanHost,
        conn: u32,
        data: &[u8],
    ) {
        {
            let Ok(input) = serde_json::from_slice::<Value>(data) else {
                return;
            };
            let window = input.get("window").and_then(Value::as_i64).unwrap_or(-1) as i32;
            // A key of a Java mod's key binding, pressed or released while playing, and the
            // HUD's size: the connection's player's, whatever window is open.
            let bind = matches!(
                input.get("kind").and_then(Value::as_str),
                Some("bind" | "modconfig")
            );
            let uuid = if bind
                || window == super::super::java_screen::HUD_WINDOW
                || window == super::super::java_screen::OVERLAY_WINDOW
            {
                host.hub
                    .guests
                    .iter()
                    .find(|g| g.conn == conn)
                    .map(|g| uuid_string(g.uuid))
            } else {
                java_window_of(host, conn, window)
            };
            let Some(uuid) = uuid else {
                return;
            };
            let input = input.to_string();
            let ran = self.with_java_world(|| horizon_jvm::events::screen_input(&uuid, &input));
            if let Err(e) = ran {
                log::warn!("Java mods: input to a mod's screen failed: {e}");
            }
            self.apply_java_actions(Some(host));
        }
    }

    /// A player's own skin from its game ([`OWN_SKIN`](super::super::java_screen::OWN_SKIN):
    /// slim flag, 64x64 RGBA): the Java mods' client builds that player's look from it.
    pub(in crate::game) fn java_own_skin(&mut self, host: &mut LanHost, conn: u32, data: &[u8]) {
        let Some((&slim, rgba)) = data.split_first() else {
            return;
        };
        if rgba.len() != 64 * 64 * 4 || !java_mods_on() {
            return;
        }
        let Some(uuid) = host
            .hub
            .guests
            .iter()
            .find(|g| g.conn == conn)
            .map(|g| uuid_string(g.uuid))
        else {
            return;
        };
        let ran = self.with_java_world(|| horizon_jvm::events::own_skin(&uuid, slim != 0, rgba));
        if let Err(e) = ran {
            log::warn!("Java mods: a player's own skin wasn't passed: {e}");
        }
    }

    /// The player closed the window of a Java mod's menu; false for other windows.
    pub(in crate::game) fn java_menu_closed(
        &mut self,
        host: &mut LanHost,
        conn: u32,
        window: i32,
    ) -> bool {
        {
            let Some(uuid) = java_window_of(host, conn, window) else {
                return false;
            };
            JAVA_WINDOWS
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&uuid);
            let closed = self.with_java_world(|| horizon_jvm::events::menu_close(&uuid));
            if let Err(e) = closed {
                log::warn!("Java mods: closing a mod's menu failed: {e}");
            }
            self.apply_java_actions(Some(host));
            true
        }
    }
}
