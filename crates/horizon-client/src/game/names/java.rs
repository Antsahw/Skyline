//! Java mods' entities' name tags (game/java_entities.rs).
use super::*;

impl Game<'_> {
    /// Java mods' entities: the name tag their renderer gives, shown always or when under
    /// the crosshair (its shouldShowName, asked both ways), within 64 blocks.
    pub(super) fn java_name_tags(&self, camera: glam::DVec3, alpha: f64, tags: &mut Vec<NameTag>) {
        let java_target = self.java_entity_hit().map(|(id, _)| id);
        for (id, at, height, name, visible) in self.java_entity_names(alpha) {
            if at.distance_squared(camera) >= 64.0 * 64.0
                || self.ui.hide_gui
                || !(visible || Some(id) == java_target)
            {
                continue;
            }
            tags.push(NameTag {
                at: at + glam::DVec3::new(0.0, height + 0.5, 0.0),
                lines: vec![(name.resolve_legacy(), 0.0)],
                light: self.world.light(
                    at.x.floor() as i32,
                    (at.y + height).floor() as i32,
                    at.z.floor() as i32,
                ),
                discrete: false,
                style: None,
            });
        }
    }
}
