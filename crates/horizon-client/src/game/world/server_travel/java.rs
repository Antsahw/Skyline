//! Java mods' heads of `ServerPlayer.teleport` on a server player's trip to another level: a
//! cancel keeps the player where it is (java_server.rs).
use super::*;

impl Game<'_> {
    /// ServerPlayer.teleport's head (Entity.teleport from handlePortal) for guest `i`: a Java
    /// mod that cancels it keeps the player here (None); the portal asks again next tick, as
    /// vanilla's does. Else the player's UUID and look.
    pub(super) fn java_portal_teleport(
        &mut self,
        host: &LanHost,
        i: usize,
        target: Dim,
    ) -> Option<(u128, [f32; 2])> {
        let (uuid, look, here) = {
            let g = &host.hub.guests[i];
            (g.uuid, [g.yaw, g.pitch], g.position)
        };
        if self.java_player_teleport(uuid, target.id(), here, look, false) {
            return None;
        }
        Some((uuid, look))
    }

    /// ServerPlayer.teleport's head for guest `i`'s teleport `t` into another level: a Java
    /// mod that cancels it keeps the player in its level (true).
    pub(super) fn java_level_teleport_refused(
        &mut self,
        host: &LanHost,
        i: usize,
        t: &LevelTeleport,
    ) -> bool {
        let (uuid, look) = {
            let g = &host.hub.guests[i];
            (g.uuid, t.rotation.unwrap_or([g.yaw, g.pitch]))
        };
        self.java_player_teleport(uuid, t.dim.id(), t.at, look, false)
    }
}
