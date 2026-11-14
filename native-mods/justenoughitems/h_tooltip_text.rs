// A tooltip text is drawn, the last of a box: where its vertices end.
fn justenoughitems_tooltip_text(value: (), batch: &horizon_client::hud::HudBatch) {
    crate::game::justenoughitems_defer::tooltip_ends(batch.vertices.len());
    value
}
