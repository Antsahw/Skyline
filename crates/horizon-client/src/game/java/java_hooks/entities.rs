//! Hooks about entities without the `java-mods` feature (bodies: `java_entities.rs`,
//! `java_server/entities.rs`, `creatures/java.rs`, `mounts/java.rs`): mods' entities drawn,
//! aimed at, hit, ridden, among mobs and projectiles' targets. There are none.
use super::super::{Game, lan::LanHost};
use horizon_client::hud::HudBatch;
use horizon_client::mesher::Vertex;
use horizon_client::projectiles::{Hit, Projectile};

impl Game<'_> {
    pub(in crate::game) fn java_entity_geometry(
        &self,
        _camera: glam::DVec3,
        _alpha: f64,
        _out: &mut Vec<Vertex>,
    ) {
    }
    pub(in crate::game) fn java_hand_geometry(
        &self,
        _to_world: glam::Mat4,
        _packed: u32,
        _out: &mut Vec<Vertex>,
    ) -> [bool; 2] {
        [false; 2]
    }
    pub(in crate::game) fn java_entity_shadows(&self, _alpha: f64) -> Vec<(glam::DVec3, f32, f32)> {
        Vec::new()
    }
    pub(in crate::game) fn java_entity_names(
        &self,
        _alpha: f64,
    ) -> Vec<(i32, glam::DVec3, f64, horizon_client::text::Text, bool)> {
        Vec::new()
    }
    /// The 1.20.1 mods' mobs are aimed at the same way, when that bridge is built.
    pub(in crate::game) fn java_entity_hit(&self) -> Option<(i32, f64)> {
        #[cfg(feature = "java-mods-120")]
        return self.java120_mob_hit();
        #[cfg(not(feature = "java-mods-120"))]
        None
    }
    pub(in crate::game) fn java_entity_location(&self, _id: i32, _distance: f64) -> [f64; 3] {
        #[cfg(feature = "java-mods-120")]
        return self.java120_mob_location(_id, _distance);
        #[cfg(not(feature = "java-mods-120"))]
        [0.0; 3]
    }
    pub(in crate::game) fn java_use_item(
        &mut self,
        _off_hand: bool,
        _yaw: f32,
        _pitch: f32,
    ) -> Option<bool> {
        None
    }
    #[allow(dead_code)]
    pub(in crate::game) fn java_attack_mob(&mut self, _id: u32) -> bool {
        false
    }
    pub(in crate::game) fn java_mob_attack(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _gi: usize,
        _id: i32,
    ) -> bool {
        false
    }
    pub(in crate::game) fn java_ride(&mut self, _vehicle: u32) -> bool {
        false
    }
    pub(in crate::game) fn java_dismount(&mut self, _vehicle: u32, _after: bool) -> bool {
        false
    }
    pub(in crate::game) fn java_entity_attack(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _id: i32,
    ) -> bool {
        false
    }
    pub(in crate::game) fn java_entity_interact(
        &mut self,
        _host: &mut LanHost,
        _conn: u32,
        _p: &horizon_protocol::packets::play::Interact,
    ) -> bool {
        false
    }
    /// The mods' creatures of a fresh chunk: none.
    pub(in crate::game) fn java_fresh_chunk(&mut self, _key: (i32, i32)) {}
    /// The mods' living entities, as the vanilla types mobs take them for: none.
    pub(in crate::game) fn java_neighbours_seen(&self) -> Vec<horizon_client::mobs::Seen> {
        Vec::new()
    }
    /// A mob's hit on one of the mods' entities: none is one.
    pub(in crate::game) fn java_mob_hits_entity(
        &mut self,
        _target: u32,
        _amount: f32,
        _source: horizon_client::mobs::Damage,
    ) -> Option<bool> {
        None
    }
    /// The mods' living entities among projectiles' targets: none.
    pub(in crate::game) fn java_projectile_targets(
        &self,
    ) -> Vec<(horizon_client::mobs::Who, horizon_core::geometry::Aabb)> {
        Vec::new()
    }
    /// A projectile's hit on one of the mods' entities: none is one.
    pub(in crate::game) fn java_projectile_hit(&mut self, _p: &Projectile, _hit: &Hit) -> bool {
        false
    }
    /// The mods' entities for the agent's entity list: none.
    pub(in crate::game) fn java_entity_list(&self) -> Vec<(i32, &str, [f64; 3])> {
        Vec::new()
    }
    pub(in crate::game) fn java_mount_jumps(&self) -> bool {
        false
    }
    pub(in crate::game) fn java_mount_health(&self) -> Option<[f32; 2]> {
        None
    }
    pub(in crate::game) fn riding_java_mob(&self) -> bool {
        false
    }
    /// This game's player rides a mod's mob on a server: it doesn't.
    pub(in crate::game) fn java_remote_ride(&mut self, _jump_key: bool) -> bool {
        false
    }
    pub(in crate::game) fn draw_java_mount_hud(
        &mut self,
        _batch: &mut HudBatch,
        _w: i32,
        _h: i32,
    ) -> i32 {
        0
    }
    pub(in crate::game) fn draw_java_jump_bar(&self, _batch: &mut HudBatch, _w: i32, _h: i32) {}
}
