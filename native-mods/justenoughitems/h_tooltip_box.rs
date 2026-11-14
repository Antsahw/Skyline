// A tooltip box starts: where its vertices begin.
fn justenoughitems_tooltip_box(batch: &horizon_client::hud::HudBatch) {
    crate::game::justenoughitems_defer::tooltip_begins(batch.vertices.len());
}
