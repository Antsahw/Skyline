// Over the Options screen: JEI's config screen, while it is open on JEI's page.
fn justenoughitems_render(
    game: &crate::game::Game<'_>,
    batch: &mut horizon_client::hud::HudBatch,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
) {
    crate::game::justenoughitems_options::render(game, batch, screen);
}
