// The pointer dragged with a button held: the config screen's.
fn justenoughitems_drag(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    canceled: &mut bool,
) {
    if crate::game::justenoughitems_options::dragged(game, screen) {
        *canceled = true;
    }
}
