//! The window's own player's connection (a memory one): what it is sent is also kept for
//! the Java mods' client (`packet_tap`), and its frames have no 21-bit limit.
use super::*;

impl PlayerSender {
    /// Keeps `body` for the Java mods' client, if this is the window's own player.
    pub(super) fn tap(&self, body: &[u8]) {
        if self.tapped {
            super::super::packet_tap::keep(body);
        }
    }
}

/// Sends the login's end, `finished`: the window's own player's is where what it is sent
/// starts being kept for the Java mods' client (`packet_tap`).
pub(super) fn send_login_finished(
    c: &mut Connection,
    uuid: u128,
    finished: &[u8],
) -> horizon_protocol::Result<()> {
    if c.is_memory() {
        super::super::packet_tap::begin(uuid);
        super::super::packet_tap::keep(finished);
    }
    c.send(finished)
}

/// Sends `body`, kept for the Java mods' client if `memory` (the window's own player).
pub(super) fn send_tapped(
    c: &mut Connection,
    memory: bool,
    body: &[u8],
) -> horizon_protocol::Result<()> {
    if memory {
        super::super::packet_tap::keep(body);
    }
    c.send(body)
}

/// The window's own player over an in-memory pipe: no 21-bit frame (a big pack's join).
pub(super) fn wide_frame(
    writer: &mut ConnWriter,
    body: &[u8],
    out: &mut Vec<u8>,
) -> horizon_protocol::Result<()> {
    let contents = horizon_protocol::frame::unbounded_contents(body, writer.threshold)?;
    writer.frame_into(&contents, out);
    Ok(())
}
