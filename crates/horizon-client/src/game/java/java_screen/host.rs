//! The server's side of Java mods' screens (feature-only): what the mods draw, to the players'
//! games; the image slots the mods' entity textures draw from.
use super::*;
use crate::game::lan::LanHost;
use horizon_protocol::packets::play::Clientbound as C;

impl Game<'_> {
    /// The Java image slot holding the image named `name`, with the image's size.
    pub(in crate::game) fn java_image_slot(&self, name: &str) -> Option<(usize, u32, u32)> {
        self.java
            .images
            .slots
            .iter()
            .find(|s| s.0 == name)
            .map(|s| (s.1, s.2, s.3))
    }

    /// What Java mods draw on a player's HUD, from Java: to their game.
    /// Sent with the 1.20.1 mods' part, if any (`send_hud_part`).
    pub(in crate::game) fn send_java_hud_frame(host: &mut LanHost, conn: u32, frame: &str) {
        send_hud_part(host, conn, HudPart::Mods263, frame);
    }

    /// What Java mods draw over a player's inventory, from Java: to their game.
    pub(in crate::game) fn send_java_overlay_frame(host: &mut LanHost, conn: u32, frame: &str) {
        host.hub.send_to(
            conn,
            &C::CustomPayload(CustomPayload {
                channel: OVERLAY_CHANNEL.into(),
                data: frame.as_bytes().to_vec(),
            }),
        );
    }

    /// A player's client-only screen closed, from Java: their game leaves it.
    pub(in crate::game) fn send_java_screen_close(host: &mut LanHost, conn: u32) {
        let data = format!("{{\"window\":{CLIENT_WINDOW},\"close\":true}}").into_bytes();
        host.hub.send_to(
            conn,
            &C::CustomPayload(CustomPayload {
                channel: CHANNEL.into(),
                data,
            }),
        );
    }

    /// A frame of a player's mod screen, from Java: to their game, for their window.
    pub(in crate::game) fn send_java_screen_frame(
        host: &mut LanHost,
        conn: u32,
        window: i32,
        frame: &str,
    ) {
        let data = format!("{{\"window\":{window},\"ops\":{frame}}}").into_bytes();
        host.hub.send_to(
            conn,
            &C::CustomPayload(CustomPayload {
                channel: CHANNEL.into(),
                data,
            }),
        );
    }
}
