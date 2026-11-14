// Options > Mods: the mod's row and keys.
fn justenoughitems_list(
    game: &mut crate::game::Game<'_>,
    mods: &mut Vec<crate::game::native_hooks::NativeMod>,
) {
    crate::game::justenoughitems_glue::mods(game, mods);
}
