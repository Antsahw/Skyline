// A mouse button let go: taken when the config screen or the overlays took its press.
fn justenoughitems_release(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    canceled: &mut bool,
) {
    let config = crate::game::justenoughitems_options::released(game, screen, button);
    if config || crate::game::justenoughitems_glue::released(game, screen, button) {
        *canceled = true;
    }
}
