// Over a container screen: JEI's overlays in place of the game's item panel.
fn justenoughitems_draw(
    game: &mut crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
) -> core::ops::ControlFlow<()> {
    if crate::game::justenoughitems_view::draw(game, batch) {
        return core::ops::ControlFlow::Break(());
    }
    core::ops::ControlFlow::Continue(())
}
