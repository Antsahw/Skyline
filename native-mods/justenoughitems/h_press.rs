// A mouse button over a screen: the config screen's, else the overlays' clicks.
fn justenoughitems_press(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    button: i32,
    modifiers: u32,
    canceled: &mut bool,
) {
    if !*canceled
        && (crate::game::justenoughitems_options::pressed(game, screen, button)
            || crate::game::justenoughitems_glue::pressed(game, screen, button, modifiers))
    {
        *canceled = true;
    }
}
