//! Hooks in what a server's players send without the `java-mods` feature (bodies:
//! `java_server/menus.rs`, `java_server/tick.rs`, `lan_actions/java.rs`): mods' menus and
//! screens' input, a player's own skin for the mods, devices' payloads, level events' source.
use super::super::{Game, lan::LanHost};

impl Game<'_> {
    /// The connection a Java mod's level event being played now leaves out: none.
    pub(in crate::game) fn java_event_except(&self) -> Option<u32> {
        self.mods.java.event_except()
    }
    pub(in crate::game) fn java_menu_click(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _p: &horizon_protocol::packets::play::ContainerClick,
    ) -> bool {
        false
    }
    pub(in crate::game) fn java_menu_closed(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _window: i32,
    ) -> bool {
        false
    }
    pub(in crate::game) fn java_screen_input(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _data: &[u8],
    ) {
    }
    pub(in crate::game) fn java_own_skin(&mut self, _host: &mut LanHost, _conn: u32, _data: &[u8]) {
    }
    /// A guest's payload on a channel nothing before took: ignored, as any unknown one.
    pub(in crate::game) fn java_device_payload(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _channel: &str,
        _data: &[u8],
    ) {
    }
}
