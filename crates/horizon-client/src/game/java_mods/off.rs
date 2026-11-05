//! Without the `java-mods` feature: what Horizon's files call on `Game` for the Java mods'
//! entities (game/java_entities.rs) and clicks (game/java_server.rs) answers "none", so the
//! plain build keeps its hooks without a `cfg` at each one.

use super::super::Game;
use horizon_client::mesher::Vertex;

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
    pub(in crate::game) fn java_entity_hit(&self) -> Option<(i32, f64)> {
        None
    }
    pub(in crate::game) fn java_entity_location(&self, _id: i32, _distance: f64) -> [f64; 3] {
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
}
