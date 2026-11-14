// A character typed: the config screen's search field, else the search box's when it has focus.
fn justenoughitems_char(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    c: char,
    canceled: &mut bool,
) {
    if !*canceled
        && (crate::game::justenoughitems_options::char_typed(game, screen, c)
            || crate::game::justenoughitems_glue::char_typed(game, screen, c))
    {
        *canceled = true;
    }
}
