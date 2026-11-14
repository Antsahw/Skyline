// A key over a screen: the config screen's, else the search box, R, U, A and the page keys.
fn justenoughitems_key(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    key: &str,
    modifiers: u32,
    canceled: &mut bool,
) {
    if !*canceled
        && (crate::game::justenoughitems_options::key_pressed(game, screen, key, modifiers)
            || crate::game::justenoughitems_glue::key_pressed(game, screen, key, modifiers))
    {
        *canceled = true;
    }
}
