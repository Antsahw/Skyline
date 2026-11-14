// Once a frame: the tables, what the screen hooks asked for, and the config
// screen over the Options screen.
fn justenoughitems_frame(game: &mut crate::game::Game<'_>) {
    crate::game::justenoughitems_glue::frame(game);
    crate::game::justenoughitems_options::frame(game);
}
