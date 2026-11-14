// Just Enough Items for Horizon: JEI's config screen over the Options screen,
// as Java's Mod Menu, JEI's config button and the mod's page's first button
// open it. While it is open on JEI's page it hides the options
// (`draw_options`), draws itself in `screen_render_post` and takes all of the
// screen's input; Escape and E close it: back to the page, out of the options
// (to Mod Menu) when Mod Menu opened it, or back to the container screen when
// JEI's config button did. Also writes the icon and Mod Menu's metadata into
// the mod's data folder.
use super::justenoughitems as jei;
use super::justenoughitems_config as config;
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::mod_api::NativeScreenView;
use horizon_client::options_ui::Page;
use std::cell::Cell;

/// Mod Menu's Configure: `native_settings_click(id, MODMENU_CONFIGURE)` after it opened the mod's page.
pub const MODMENU_CONFIGURE: usize = 0x4D4D_4346;

/// Who opened the screen.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Opened {
    /// The page's "Configure..." button.
    Page,
    /// Mod Menu's Configure.
    ModMenu,
    /// JEI's config button beside a container screen.
    ConfigButton,
}

thread_local! {
    static OPEN: Cell<Option<Opened>> = const { Cell::new(None) };
    /// Mouse buttons whose press the screen took, so their release is taken too.
    static TAKEN: Cell<u8> = const { Cell::new(0) };
    /// The icon and the metadata were written this run.
    static WRITTEN: Cell<bool> = const { Cell::new(false) };
}

/// JEI's page is the one the Options screen shows: its index in Options > Mods.
fn page(game: &Game<'_>) -> Option<usize> {
    if game.screen != Screen::Options {
        return None;
    }
    match game.ui.options.page() {
        Page::NativeMod(i) if game.settings.mods.installed.get(i).is_some_and(|m| m.id == jei::ID) => Some(i),
        _ => None,
    }
}

/// The config screen is up over JEI's page.
pub(super) fn covers(game: &Game<'_>) -> bool {
    OPEN.with(Cell::get).is_some() && config::is_open() && page(game).is_some()
}

/// Opens the config screen over JEI's page.
pub(super) fn open(game: &mut Game<'_>, opened: Opened) {
    if page(game).is_none() {
        return;
    }
    config::open(game);
    OPEN.with(|o| o.set(Some(opened)));
    TAKEN.with(|t| t.set(0));
}

/// Once a frame: the data files, what the screen asked for, and its closing.
pub(super) fn frame(game: &mut Game<'_>) {
    if !WRITTEN.with(Cell::get) {
        WRITTEN.with(|w| w.set(true));
        write_data(game);
    }
    let Some(opened) = OPEN.with(Cell::get) else {
        return;
    };
    // Clicks and the clipboard, which need the game mutably.
    let (clicks, clip) = config::with(|c| (std::mem::take(&mut c.clicks), c.clipboard.take())).unwrap_or((0, None));
    for _ in 0..clicks {
        game.audio.play_button_click();
    }
    if let Some(op) = clip {
        let text = game.clipboard.get();
        let mut copied = None;
        config::with(|c| c.clipboard(op, || text, |s| copied = Some(s.to_owned())));
        if let Some(s) = copied {
            game.clipboard.set(&s);
        }
    }
    match page(game) {
        // The options went elsewhere some other way: the screen goes.
        None => {
            OPEN.with(|o| o.set(None));
            config::close();
        }
        Some(i) if config::with(|c| c.closed).unwrap_or(true) => {
            OPEN.with(|o| o.set(None));
            config::close();
            match opened {
                // JEI's page again, its labels following the new values.
                Opened::Page => game.native_settings_refresh(i),
                // Out of the options the way Escape goes: back to Mod Menu,
                // or to the container screen the config button was on.
                Opened::ModMenu | Opened::ConfigButton => {
                    let events = game.ui.options.back(&mut game.settings);
                    let _ = game.handle_option_events(events);
                }
            }
        }
        Some(_) => {}
    }
}

fn over(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    screen.id == "options" && covers(game)
}

fn argb(c: u32) -> u32 {
    let [a, r, g, b] = c.to_be_bytes();
    rgba(r, g, b, a)
}

/// Draws the screen over the Options screen (`screen_render_post`): the
/// screen's background first (`Screen.extractTransparentBackground`), then
/// the screen, then its tooltip.
pub(super) fn render(game: &Game<'_>, batch: &mut HudBatch, screen: &NativeScreenView<'_>) {
    if !over(game, screen) {
        return;
    }
    let in_world = game.title.is_none() && game.player.is_some();
    if in_world {
        // (no screen blur hook in this Horizon)
    }
    let (w, h) = (batch.width, batch.height);
    horizon_client::ui::menu_background(batch, &game.gui.atlas, 0.0, 0.0, w, h, in_world);
    if let Some(lines) = config::draw(game, batch)
        && !lines.is_empty()
    {
        let lines: Vec<(String, u32)> = lines.into_iter().map(|(l, c)| (l, argb(c))).collect();
        game.draw_tooltip_lines(batch, &lines, game.cursor.0, game.cursor.1);
    }
}

fn bit(button: i32) -> u8 {
    1 << button.clamp(0, 7)
}

/// A mouse button pressed over the screen: true when it took it.
pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    if !over(game, screen) {
        return false;
    }
    TAKEN.with(|t| t.set(t.get() | bit(button)));
    config::press(game, (screen.mouse[0], screen.mouse[1]), button);
    true
}

/// A mouse button let go: taken when its press was.
pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    let taken = TAKEN.with(|t| {
        let was = t.get() & bit(button) != 0;
        t.set(t.get() & !bit(button));
        was
    });
    if !over(game, screen) {
        return taken;
    }
    config::release(game, (screen.mouse[0], screen.mouse[1]), button);
    true
}

/// The pointer dragged with a button held.
pub(super) fn dragged(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    if !over(game, screen) {
        return false;
    }
    config::drag(game, (screen.mouse[0], screen.mouse[1]));
    true
}

/// The wheel.
pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !over(game, screen) {
        return false;
    }
    config::scroll(game, (screen.mouse[0], screen.mouse[1]), scroll[1]);
    true
}

/// A key: the screen takes every one.
pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !over(game, screen) {
        return false;
    }
    config::key(game, key, modifiers);
    true
}

/// A key let go: nothing under the screen hears it.
pub(super) fn key_released(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    over(game, screen)
}

/// A character: the search field's when it has focus.
pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    if !over(game, screen) {
        return false;
    }
    config::char_typed(c);
    true
}

// ---- the icon and Mod Menu's metadata ---------------------------------------------------

/// JEI's icon.
const ICON: &[u8] = horizon_client::native_data!("justenoughitems", "icon.png");

/// What Mod Menu shows of the mod, from JEI's `fabric.mod.json` (its
/// environment is "*", so no Client badge).
fn modmenu_json() -> String {
    format!(
        r#"{{
  "schema": 1,
  "name": "{name}",
  "version": "{version}",
  "description": "JEI is an item and recipe viewing mod for Minecraft, built from the ground up for stability and performance.",
  "authors": ["mezz"],
  "contributors": ["DeportedToHell"],
  "license": "MIT",
  "links": {{
    "homepage": "https://www.curseforge.com/minecraft/mc-mods/jei",
    "sources": "https://github.com/DeportedToHell/MCRE-Mods/tree/main/justenoughitems",
    "issues": "https://github.com/DeportedToHell/MCRE-Mods/issues"
  }},
  "badges": [],
  "config_screen": true
}}
"#,
        name = jei::NAME,
        version = jei::VERSION,
    )
}

/// `icon.png` and `modmenu.json` in the mod's data folder, each written only
/// when it differs from what is there.
fn write_data(game: &Game<'_>) {
    let dir = game.native_data_dir(jei::ID);
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let meta = modmenu_json();
    for (name, bytes) in [("icon.png", ICON), ("modmenu.json", meta.as_bytes())] {
        let path = dir.join(name);
        if std::fs::read(&path).ok().as_deref() != Some(bytes) {
            let _ = std::fs::write(&path, bytes);
        }
    }
}
