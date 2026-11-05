//! Java mods' mobs ridden on a server (game/java_entities.rs): their hearts, jump bar and
//! seat.
use super::*;

impl Game<'_> {
    /// This game's player sits on a Java mod's mob (the server says so each tick).
    /// The Java mod's mob it rides jumps as a horse (the bar charges with the jump key).
    pub(in crate::game) fn java_mount_jumps(&self) -> bool {
        #[cfg(feature = "java-mods")]
        {
            self.riding_java_mob() && self.java.entities.ride.is_some_and(|r| r.jump)
        }
        #[cfg(not(feature = "java-mods"))]
        {
            false
        }
    }

    /// The Java mod's mob it rides: its health and max health when it shows hearts.
    pub(in crate::game) fn java_mount_health(&self) -> Option<[f32; 2]> {
        #[cfg(feature = "java-mods")]
        {
            self.java
                .entities
                .ride
                .filter(|_| self.riding.is_none())
                .and_then(|r| r.hp)
        }
        #[cfg(not(feature = "java-mods"))]
        {
            None
        }
    }

    pub(in crate::game) fn riding_java_mob(&self) -> bool {
        #[cfg(feature = "java-mods")]
        {
            self.riding.is_none() && self.java.entities.ride.is_some()
        }
        #[cfg(not(feature = "java-mods"))]
        {
            false
        }
    }

    /// A Java mod's mob (game/java_entities.rs): the server's mirror moves it by this
    /// player's keys (PlayerInput) and says where the seat is; the player sits there, and
    /// its position packets carry the seat. False when this player rides none.
    #[cfg(feature = "java-mods")]
    pub(in crate::game) fn java_remote_ride(&mut self, jump_key: bool) -> bool {
        if self.riding.is_none()
            && let Some(ride) = self.java.entities.ride
        {
            // The jump bar (the server's mirror charges the jump itself from the keys).
            if ride.jump {
                self.mount.charge_jump(jump_key);
            }
            if let Some(p) = self.player.as_mut() {
                self.previous_position = p.position;
                p.position = Vec3::new(ride.p[0], ride.p[1], ride.p[2]);
                p.velocity = Vec3::ZERO;
                p.fall_distance = 0.0;
            }
            return true;
        }
        false
    }

    /// A Java mod's mob: its hearts, as a vanilla mount's (the server's snapshot gives them);
    /// the hearts drawn. Its jump bar is in `draw_jump_bar`.
    pub(super) fn draw_java_mount_hud(&mut self, batch: &mut HudBatch, w: i32, h: i32) -> i32 {
        let mut hearts = 0;
        if let Some([health, max]) = self.java_mount_health() {
            hearts = horizon_client::hud_layout::vehicle_max_hearts(max);
            let draws = horizon_client::hud_layout::vehicle_health(w, h, hearts, health);
            horizon_client::hud_layout::paint(batch, &self.gui.atlas, &draws);
        }
        hearts
    }

    /// A Java mod's horse-style mob: its jump bar (its hearts are the mod's).
    pub(super) fn draw_java_jump_bar(&self, batch: &mut HudBatch, w: i32, h: i32) {
        if self.java_mount_jumps() {
            let draws = horizon_client::hud_overlays::jump_bar(w, h, 0, self.mount.jump_scale);
            horizon_client::hud_layout::paint(batch, &self.gui.atlas, &draws);
        }
    }
}
