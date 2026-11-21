//! This game's own Java mods' client (`java_device.rs`: their client half in the player's
//! own JVM) as the screens' code asks it (`java_screen.rs`), without the `java-mods`
//! feature: there is none, so the server's mods draw and hear everything.
use super::super::Game;
use horizon_client::hud::HudBatch;
use serde_json::Value;

impl Game<'_> {
    /// Whether this game's own JVM runs every mod's client half: no.
    pub(in crate::game) fn java_own_all() -> bool {
        false
    }
    /// The keys this game's own mods' client binds: none.
    pub(in crate::game) fn java_own_keys() -> Vec<i32> {
        Vec::new()
    }
    pub(in crate::game) fn java_own_key(_key: i32, _keycode: i32, _mods: i32, _down: bool) {}
    /// Whether this game's own client takes `input` instead of the server: no.
    pub(in crate::game) fn own_client_takes(_input: &Value) -> bool {
        false
    }
    /// Whether this game's own client hears `input` beside the server: no.
    pub(in crate::game) fn own_client_also(_input: &Value) -> bool {
        false
    }
    pub(in crate::game) fn own_client_input(&mut self, _input: &Value) {}
    /// Whether this game's own client shows a screen: no.
    pub(in crate::game) fn java_device_screen_open() -> bool {
        false
    }
    pub(in crate::game) fn device_screen_input(&mut self, _input: &Value) {}
    /// What this game's own mods' client draws on the HUD: nothing.
    pub(in crate::game) fn draw_own_client_hud(&mut self, _batch: &mut HudBatch) {}
}
