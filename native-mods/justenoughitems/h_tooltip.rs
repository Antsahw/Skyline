// An item tooltip: the mod's name at its foot.
fn justenoughitems_tooltip(
    stack: &horizon_client::items::ItemStack,
    lines: &mut Vec<(horizon_client::text::Text, u32)>,
) {
    crate::game::justenoughitems_glue::tooltip(stack, lines);
}
