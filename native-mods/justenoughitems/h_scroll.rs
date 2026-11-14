// The wheel over a screen: the config screen's, else the overlays' pages.
fn justenoughitems_scroll(
    game: &crate::game::Game<'_>,
    screen: &horizon_client::mod_api::NativeScreenView<'_>,
    scroll: [f64; 2],
    canceled: &mut bool,
) {
    if !*canceled
        && (crate::game::justenoughitems_options::scrolled(game, screen, scroll)
            || crate::game::justenoughitems_glue::scrolled(game, screen, scroll))
    {
        *canceled = true;
    }
}
