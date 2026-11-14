// The settings page's buttons.
fn justenoughitems_settings(game: &mut crate::game::Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    crate::game::justenoughitems_glue::settings_buttons(game, mod_id, buttons);
}
