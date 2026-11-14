// Just Enough Items for Horizon: drawing. The ingredient list right of the
// container screen, the bookmarks left of it, and the recipes screen over it,
// laid out with JEI's own numbers (`IngredientListOverlayLayout`,
// `BookmarkOverlayLayout`, `RecipesGui`, the vanilla categories) in JEI's
// sprites (`justenoughitems_sprites`, drawn as fills) and the JEI buttons the
// game's atlas carries. Every part drawn is recorded in `State::hits` for the
// mouse and keys of the next frame.
use super::justenoughitems::{self as jei, Category, Lookup, Rect, Recipe, Stack, inside};
use super::justenoughitems_defer as defer;
use super::justenoughitems_glue::{self as glue, Data, Hit, OWN_TOOLTIP, State};
use super::justenoughitems_sprites::{self as sprites, Sprite};
use super::{Game, Screen};
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use std::time::Instant;

/// An ingredient's cell (16 and a pixel round it).
const CELL: f32 = 18.0;
/// `BORDER_MARGIN`: round every panel and row.
const MARGIN: f32 = 6.0;
/// The page row and the search row.
const ROW: f32 = 20.0;
/// The ingredient list's and the bookmarks' most columns and rows.
const MAX_COLUMNS: usize = 9;
const MAX_ROWS: usize = 16;
/// The recipes screen (`RecipesGui`): least width, least and greatest
/// height, the window's padding, the small arrow buttons, the tabs.
const MIN_WIDTH: f32 = 198.0;
const MIN_HEIGHT: f32 = 175.0;
const MAX_HEIGHT: f32 = 350.0;
const PADDING: f32 = 6.0;
const SMALL: f32 = 13.0;
const TAB: f32 = 24.0;
/// A recipe's border, the space between recipes, its "+" button.
const RECIPE_BORDER: f32 = 4.0;
const RECIPE_GAP: f32 = 4.0;
const RECIPE_BUTTON: f32 = 13.0;
const MAX_RECIPE_COLUMNS: usize = 2;

const WHITE: u32 = 0xffff_ffff;
const GREY: u32 = 0xff80_8080;
const NAV_FILL: u32 = 0x3000_0000;
const DIM: u32 = 0xe810_1010;
const RED_TEXT: u32 = 0xffff_0000;
const GREEN: u32 = 0xff80_ff20;
const GREEN_ERROR: u32 = 0xffff_6060;
const GOLD: u32 = 0xffff_aa00;
const DARK_GREY: u32 = 0xff55_5555;

// ---- drawing primitives -------------------------------------------------------------

/// The game's colour from 0xAARRGGBB.
fn argb(c: u32) -> u32 {
    let [a, r, g, b] = c.to_be_bytes();
    rgba(r, g, b, a)
}

fn fill(game: &Game<'_>, batch: &mut HudBatch, r: Rect, color: u32) {
    if r[2] > 0.0 && r[3] > 0.0 {
        batch.fill(&game.gui.atlas, r[0], r[1], r[2], r[3], argb(color));
    }
}

/// One of JEI's sprites at its size, `scale` times.
fn blit(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, x: f32, y: f32, scale: f32) {
    let atlas = &game.gui.atlas;
    for &(px, py, len, c) in s.runs {
        batch.fill(
            atlas,
            x + f32::from(px) * scale,
            y + f32::from(py) * scale,
            f32::from(len) * scale,
            scale,
            c,
        );
    }
}

/// Columns `cols` and rows `rows` of a sprite, at `at`.
fn blit_part(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, at: (f32, f32), cols: (u16, u16), rows: (u16, u16)) {
    let atlas = &game.gui.atlas;
    let w = usize::from(s.width);
    for py in rows.0..rows.1 {
        let mut px = cols.0;
        while px < cols.1 {
            let c = s.pixels[usize::from(py) * w + usize::from(px)];
            let mut len = 1;
            while px + len < cols.1 && s.pixels[usize::from(py) * w + usize::from(px + len)] == c {
                len += 1;
            }
            if c >> 24 != 0 {
                batch.fill(
                    atlas,
                    at.0 + f32::from(px - cols.0),
                    at.1 + f32::from(py - rows.0),
                    f32::from(len),
                    1.0,
                    c,
                );
            }
            px += len;
        }
    }
}

/// A nine-slice sprite stretched over `r`: its corners as they are, its
/// edges and middle (plain colours in JEI's backgrounds) stretched.
fn nine(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, r: Rect) {
    let Some([l, t, rt, b]) = s.border else {
        blit(game, batch, s, r[0], r[1], 1.0);
        return;
    };
    let (w, h) = (s.width, s.height);
    let atlas = &game.gui.atlas;
    let (x, y) = (r[0], r[1]);
    let rw = r[2].max(f32::from(l + rt));
    let rh = r[3].max(f32::from(t + b));
    let px = |cx: u16, cy: u16| s.pixels[usize::from(cy) * usize::from(w) + usize::from(cx)];
    let (mid_w, mid_h) = (rw - f32::from(l + rt), rh - f32::from(t + b));
    blit_part(game, batch, s, (x, y), (0, l), (0, t));
    blit_part(game, batch, s, (x + rw - f32::from(rt), y), (w - rt, w), (0, t));
    blit_part(game, batch, s, (x, y + rh - f32::from(b)), (0, l), (h - b, h));
    blit_part(game, batch, s, (x + rw - f32::from(rt), y + rh - f32::from(b)), (w - rt, w), (h - b, h));
    for row in 0..t {
        put(batch, atlas, [x + f32::from(l), y + f32::from(row), mid_w, 1.0], px(l, row));
    }
    for row in h - b..h {
        put(batch, atlas, [x + f32::from(l), y + rh - f32::from(h - row), mid_w, 1.0], px(l, row));
    }
    for col in 0..l {
        put(batch, atlas, [x + f32::from(col), y + f32::from(t), 1.0, mid_h], px(col, t));
    }
    for col in w - rt..w {
        put(batch, atlas, [x + rw - f32::from(w - col), y + f32::from(t), 1.0, mid_h], px(col, t));
    }
    put(batch, atlas, [x + f32::from(l), y + f32::from(t), mid_w, mid_h], px(l, t));
}

fn put(batch: &mut HudBatch, atlas: &horizon_client::hud::HudAtlas, r: Rect, c: u32) {
    if c >> 24 != 0 && r[2] > 0.0 && r[3] > 0.0 {
        batch.fill(atlas, r[0], r[1], r[2], r[3], c);
    }
}

fn text(game: &Game<'_>, batch: &mut HudBatch, s: &str, x: f32, y: f32, color: u32, shadow: bool) {
    batch.text(&game.gui.atlas, s, x.floor(), y.floor(), argb(color), shadow);
}

fn width(game: &Game<'_>, s: &str) -> f32 {
    HudBatch::text_width(&game.gui.atlas, s)
}

/// `s` cut with "..." to fit `max` pixels.
fn fit(game: &Game<'_>, s: &str, max: f32) -> String {
    if width(game, s) <= max {
        return s.to_owned();
    }
    let mut out = String::new();
    for c in s.chars() {
        if width(game, &format!("{out}{c}...")) > max {
            break;
        }
        out.push(c);
    }
    format!("{out}...")
}

/// One of JEI's buttons (the game's atlas has its nine-slices): pressed,
/// active or greyed, lit under the pointer.
fn button(game: &Game<'_>, batch: &mut HudBatch, r: Rect, active: bool, pressed: bool) {
    let hovered = inside(game.cursor, r);
    if pressed {
        let s = if hovered { &sprites::BUTTON_PRESSED_HIGHLIGHT } else { &sprites::BUTTON_PRESSED };
        if r[2] == 20.0 && r[3] == 20.0 {
            blit(game, batch, s, r[0], r[1], 1.0);
            return;
        }
    }
    let sprite = match (active, hovered) {
        (false, _) => "horizon/jei/button_disabled_v2",
        (true, false) => "horizon/jei/button_enabled_v2",
        (true, true) => "horizon/jei/button_highlight_v2",
    };
    horizon_client::ui::nine_slice(batch, &game.gui.atlas, sprite, r[0], r[1], r[2], r[3], u32::MAX);
}

/// A button with JEI's page arrow.
fn arrow_button(game: &Game<'_>, batch: &mut HudBatch, r: Rect, back: bool, active: bool) {
    button(game, batch, r, active, false);
    let icon = if back { "horizon/jei/icons/arrow_previous_v2" } else { "horizon/jei/icons/arrow_next_v2" };
    batch.sprite(&game.gui.atlas, icon, r[0] + ((r[2] - 9.0) / 2.0).floor(), r[1] + ((r[3] - 7.0) / 2.0).floor());
}

/// JEI's page row (`PageNavigation`): the two buttons, the dark strip between
/// them and "page/pages".
fn page_row(game: &Game<'_>, batch: &mut HudBatch, nav: Rect, page: usize, pages: usize) -> (Rect, Rect) {
    let b = ROW.min(nav[3]).min(nav[2] / 2.0);
    let back = [nav[0], nav[1], b, b];
    let next = [nav[0] + nav[2] - b, nav[1], b, b];
    fill(game, batch, [back[0] + b, nav[1], next[0] - back[0] - b, b], NAV_FILL);
    arrow_button(game, batch, back, true, pages > 1);
    arrow_button(game, batch, next, false, pages > 1);
    let label = format!("{}/{}", page + 1, pages);
    let w = width(game, &label);
    if w <= nav[2] - 40.0 {
        text(game, batch, &label, nav[0] + ((nav[2] - w) / 2.0).round(), nav[1] + ((b - 9.0) / 2.0).round(), WHITE, true);
    }
    (back, next)
}

/// The vanilla slot highlight behind (`back`) or over an item: 8 pixels
/// round its 16.
fn highlight(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32, size: f32, back: bool) {
    let name = if back {
        "gui/sprites/container/slot_highlight_back"
    } else {
        "gui/sprites/container/slot_highlight_front"
    };
    batch.sprite_scaled(&game.gui.atlas, name, x - 4.0, y - 4.0, size + 8.0, size + 8.0, u32::MAX);
}

fn item(game: &Game<'_>, batch: &mut HudBatch, stack: &ItemStack, x: f32, y: f32) {
    game.draw_item(batch, stack, x, y);
}

fn intersects(a: Rect, b: Rect) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}

// ---- where things go ----------------------------------------------------------------

/// The container screen's window (`AbstractContainerScreenHandler`).
fn container_rect(game: &Game<'_>, batch: &HudBatch) -> Rect {
    let image = game.container_image(batch);
    [image.left as f32, image.top as f32, image.width as f32, image.height as f32]
}

/// What the overlays keep out of: the toasts (top right), the recipe book
/// (left) and the inventory's effect bars (right, `InventoryEffectRenderer`).
fn exclusions(game: &Game<'_>, batch: &HudBatch, gui: Rect) -> Vec<Rect> {
    let mut out = toasts(game, batch);
    if game.recipe_book_shift(batch.width) > 0.0 {
        let x = ((batch.width - 147.0) / 2.0 - 86.0).floor();
        out.push([x, gui[1], 147.0, 166.0]);
    }
    let shows = match game.screen {
        Screen::Inventory => game.container.is_none() && game.craft_size == 2 && game.station.is_none(),
        Screen::Creative => true,
        _ => false,
    };
    let n = game.potions.effects.list.len();
    let x = gui[0] + gui[2] + 2.0;
    if shows && n > 0 && batch.width - x >= 32.0 {
        let step = if n > 5 { 132.0 / (n - 1) as f32 } else { 33.0 };
        let w = if batch.width - x >= 120.0 { 120.0 } else { 32.0 };
        for i in 0..n {
            out.push([x, gui[1] + i as f32 * step, w, 32.0]);
        }
    }
    out
}

/// The toasts' corner (`ScreenHelper.getToastsArea`): 32 pixels a slot.
fn toasts(game: &Game<'_>, batch: &HudBatch) -> Vec<Rect> {
    let used = game.toast_slots_used();
    if used == 0 {
        return Vec::new();
    }
    let slots = (32 - used.leading_zeros()) as f32;
    vec![[batch.width - 160.0, 0.0, 160.0, slots * 32.0]]
}

/// A grid of cells laid out as JEI's `IngredientGridWithNavigationLayout`:
/// the free cells (those clear of `exclusions`) and the page row above.
struct Grid {
    cells: Vec<(f32, f32)>,
    rect: Rect,
    nav: Rect,
}

fn grid(avail: Rect, right: bool, items: Option<usize>, exclusions: &[Rect]) -> Option<Grid> {
    // insetBy(6), the page row reserved above.
    let a = [avail[0] + MARGIN, avail[1] + MARGIN + ROW + 2.0, avail[2] - 2.0 * MARGIN, avail[3] - 2.0 * MARGIN - ROW - 2.0];
    let columns = ((a[2] / CELL).floor().max(0.0) as usize).min(MAX_COLUMNS);
    let mut rows = ((a[3] / CELL).floor().max(0.0) as usize).min(MAX_ROWS);
    if columns < 2 || rows < 1 {
        return None;
    }
    // shrinkToFit: as few rows as hold every item.
    if let Some(n) = items
        && n <= columns * rows
    {
        rows = n.max(1).div_ceil(columns);
    }
    let w = columns as f32 * CELL;
    let x = if right { a[0] + a[2] - w } else { a[0] };
    let rect = [x, a[1], w, rows as f32 * CELL];
    let mut cells = Vec::new();
    for r in 0..rows {
        for c in 0..columns {
            let cx = x + c as f32 * CELL;
            let cy = a[1] + r as f32 * CELL;
            let grown = [cx - 2.0, cy - 2.0, CELL + 4.0, CELL + 4.0];
            if !exclusions.iter().any(|e| intersects(grown, *e)) {
                cells.push((cx, cy));
            }
        }
    }
    if cells.is_empty() {
        return None;
    }
    let nav = [x, a[1] - ROW - 2.0, w, ROW];
    // A page row under something (the toasts): the grid goes below it.
    if let Some(bottom) = exclusions
        .iter()
        .filter(|e| intersects(nav, **e))
        .map(|e| e[1] + e[3])
        .reduce(f32::max)
        && bottom > avail[1]
    {
        let moved = [avail[0], bottom, avail[2], avail[3] - (bottom - avail[1])];
        return grid(moved, right, items, exclusions);
    }
    Some(Grid { cells, rect, nav })
}

/// What to show over everything at the end of the frame.
struct Tip {
    stack: Option<ItemStack>,
    /// Lines under the stack's name, or the whole tooltip without a stack.
    lines: Vec<(String, u32)>,
}

impl Tip {
    fn text(lines: Vec<(String, u32)>) -> Tip {
        Tip { stack: None, lines }
    }
    fn stack(stack: ItemStack) -> Tip {
        Tip { stack: Some(stack), lines: Vec::new() }
    }
}

// ---- the whole frame --------------------------------------------------------------------

/// Draws the overlays over the container screen in place of the game's
/// item panel (`draw_mod_browser`): true when the mod is on.
pub(super) fn draw(game: &mut Game<'_>, batch: &mut HudBatch) -> bool {
    if !glue::on(game) {
        defer::forget();
        return false;
    }
    // The screen's tooltips (its slots', tabs', effects', buttons') go over
    // the overlays, as Java draws them after `ScreenEvent.Render.Post`.
    let tooltips = defer::take(batch);
    let tip = glue::with(|s| frame(game, batch, s)).flatten();
    let cursor = game.cursor;
    let covered = glue::COVERED.with(std::cell::Cell::get);
    // JEI's own tooltip, when it shows one, is the only one.
    let mut own = false;
    if game.carried.is_none() {
        match tip {
            Some(Tip { stack: Some(stack), lines }) => {
                let extra: Vec<(Text, u32)> = lines.into_iter().map(|(l, c)| (Text::literal(l), argb(c))).collect();
                OWN_TOOLTIP.with(|c| c.set(true));
                game.draw_stack_tooltip_with(batch, &stack, false, cursor.0, cursor.1, extra);
                OWN_TOOLTIP.with(|c| c.set(false));
                own = true;
            }
            Some(Tip { stack: None, lines }) if !lines.is_empty() => {
                let lines: Vec<(String, u32)> = lines.into_iter().map(|(l, c)| (l, argb(c))).collect();
                game.draw_tooltip_lines(batch, &lines, cursor.0, cursor.1);
                own = true;
            }
            _ => {}
        }
    } else {
        game.draw_carried(batch);
    }
    // Under the recipes screen the game's tooltips stay hidden.
    if !own && !covered {
        defer::put_back(batch, tooltips);
    }
    true
}

fn frame(game: &mut Game<'_>, batch: &mut HudBatch, s: &mut State) -> Option<Tip> {
    let settings = glue::settings(game, s);
    s.hits.clear();
    let millis = s.started.get_or_insert_with(Instant::now).elapsed().as_millis() as u64;
    s.data.as_ref()?;
    let game: &Game<'_> = game;
    let mut tip = None;
    let container = container_rect(game, batch);
    let mut exclusion = exclusions(game, batch, container);
    // The area the overlays keep beside: the recipes screen's when it is up.
    let mut gui = container;
    if s.recipes.is_open() {
        fill(game, batch, [0.0, 0.0, batch.width, batch.height], DIM);
        exclusion.clear();
        if let Some((area, t)) = recipes_screen(game, batch, s, millis) {
            gui = area;
            tip = t;
        }
    }
    if let Some(t) = ingredient_list(game, batch, s, gui, &exclusion, settings) {
        tip = Some(t);
    }
    if let Some(t) = bookmark_list(game, batch, s, gui, &exclusion, settings) {
        tip = Some(t);
    }
    if let Some((note, at)) = &s.note
        && at.elapsed().as_secs() < 3
    {
        let w = width(game, note);
        let x = ((batch.width - w) / 2.0).floor();
        fill(game, batch, [x - 3.0, 3.0, w + 6.0, 12.0], 0xc000_0000);
        text(game, batch, note, x, 5.0, 0xffff_5555, true);
    }
    tip
}

// ---- the ingredient list (`IngredientListOverlay`) --------------------------------------

fn ingredient_list(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &mut State,
    gui: Rect,
    exclusions: &[Rect],
    settings: jei::Settings,
) -> Option<Tip> {
    let ru = s.ru;
    let right = gui[0] + gui[2];
    let display: Rect = [right, 0.0, (batch.width - right).max(0.0), batch.height];
    let centered = settings.center_search && gui[1] + gui[3] + ROW < batch.height;
    let avail = if centered { display } else { [display[0], 0.0, display[2], batch.height - ROW - 2.0] };
    let list = if settings.overlay { glue::filtered(game, s) } else { Vec::new() };
    let laid = grid(avail, true, None, exclusions);
    // The search row: under the list, or centred under the screen.
    let inset = [display[0] + MARGIN, MARGIN, display[2] - 2.0 * MARGIN, batch.height - 2.0 * MARGIN];
    let row_y = inset[1] + inset[3] - ROW;
    let row: Rect = if centered {
        [gui[0], row_y, gui[2], ROW]
    } else if let Some(g) = &laid {
        [g.rect[0], row_y, g.rect[2], ROW]
    } else {
        [inset[0], row_y, inset[2], ROW]
    };
    if row[2] < ROW {
        return None;
    }
    let search_rect: Rect = [row[0], row[1], row[2] - ROW, ROW];
    let config_rect: Rect = [row[0] + row[2] - ROW, row[1], ROW, ROW];
    let mut hits: Vec<(Rect, Hit)> = Vec::new();
    let mut tip = None;

    if settings.overlay {
        if s.filtered_text_changed() {
            s.list_page = 0;
        }
        let empty = list.is_empty();
        let search = glue::search_box(s);
        [search.x, search.y, search.width, search.height] = search_rect;
        search.suggestion = None;
        search.text_color = argb(if empty && !search.value.is_empty() { RED_TEXT } else { WHITE });
        search.draw(batch, &game.gui.atlas, None);
        hits.push((search_rect, Hit::Search));
    }

    // The config button: JEI's gear, red in cheat mode.
    button(game, batch, config_rect, true, false);
    let icon = if settings.cheat { &sprites::ICON_CONFIG_CHEAT } else { &sprites::ICON_CONFIG };
    blit(game, batch, icon, config_rect[0] + 2.0, config_rect[1] + 2.0, 1.0);
    hits.push((config_rect, Hit::Config));
    if inside(game.cursor, config_rect) {
        let mut lines = vec![(jei::tr(ru, "config").to_owned(), WHITE)];
        if !settings.overlay {
            lines.push((jei::tr(ru, "overlay_off").to_owned(), GOLD));
            lines.push((jei::tr(ru, "overlay_off_fix").to_owned(), 0xffaa_aaaa));
        }
        if settings.cheat {
            lines.push((jei::tr(ru, "cheat_on").to_owned(), 0xffff_5555));
        }
        lines.push((jei::tr(ru, "cheat_hint").to_owned(), 0xffaa_aaaa));
        tip = Some(Tip::text(lines));
    }

    if let (true, Some(g)) = (settings.overlay, laid) {
        let per = g.cells.len();
        s.list_per = per;
        let pages = jei::pages(list.len(), per);
        s.list_page = s.list_page.min(pages - 1);
        let bg: Rect = [g.nav[0], g.nav[1], g.rect[2], g.rect[1] + g.rect[3] - g.nav[1]];
        hits.insert(0, (bg, Hit::Area));
        let (back, next) = page_row(game, batch, g.nav, s.list_page, pages);
        hits.push((back, Hit::ListPage(-1)));
        hits.push((next, Hit::ListPage(1)));
        let data = s.data.as_ref()?;
        for (&(x, y), &id) in g.cells.iter().zip(list.iter().skip(s.list_page * per)) {
            let Some(ingredient) = data.ingredient(id) else {
                continue;
            };
            let cell = [x, y, CELL, CELL];
            let hovered = inside(game.cursor, cell);
            if hovered {
                highlight(game, batch, x + 1.0, y + 1.0, 16.0, true);
            }
            item(game, batch, &ingredient.stack, x + 1.0, y + 1.0);
            if hovered {
                highlight(game, batch, x + 1.0, y + 1.0, 16.0, false);
                tip = Some(Tip::stack(ingredient.stack.clone()));
            }
            hits.push((cell, Hit::List(id)));
        }
    }
    s.hits.extend(hits);
    tip
}

// ---- the bookmarks (`BookmarkOverlay`) ------------------------------------------------

fn bookmark_list(
    game: &Game<'_>,
    batch: &mut HudBatch,
    s: &mut State,
    gui: Rect,
    exclusions: &[Rect],
    settings: jei::Settings,
) -> Option<Tip> {
    let ru = s.ru;
    let mut right = gui[0].max(0.0);
    for e in exclusions {
        if e[1] < gui[1] + gui[3] && e[1] + e[3] > gui[1] && e[0] > 0.0 && e[0] < gui[0] && e[0] + e[2] >= gui[0] {
            right = right.min(e[0]);
        }
    }
    let display: Rect = [0.0, 0.0, right, batch.height];
    let avail: Rect = [0.0, 0.0, right, batch.height - ROW - 2.0];
    let keys = glue::bookmarks(s).keys.clone();
    let data = s.data.as_ref()?;
    let ids: Vec<u32> = keys.iter().filter_map(|k| data.ingredient_by_key(k)).collect();
    let laid = grid(avail, false, Some(ids.len()), &[]);
    let inset = [MARGIN, MARGIN, display[2] - 2.0 * MARGIN, display[3] - 2.0 * MARGIN];
    let mut hits: Vec<(Rect, Hit)> = Vec::new();
    let mut tip = None;
    let shown = settings.overlay && settings.bookmarks && !ids.is_empty();

    // The bookmark button, bottom left: pressed while the bookmarks show.
    if inset[2] >= 42.0 && inset[3] >= ROW {
        let x = match &laid {
            Some(g) if g.rect[2] >= 42.0 => g.rect[0],
            _ => inset[0],
        };
        let r: Rect = [x, inset[1] + inset[3] - ROW, ROW, ROW];
        let on = settings.bookmarks && !ids.is_empty();
        button(game, batch, r, true, on);
        let icon = if on { &sprites::ICON_BOOKMARK_ON } else { &sprites::ICON_BOOKMARK_OFF };
        let nudge = if on { 0.5 } else { 0.0 };
        blit(game, batch, icon, r[0] + 2.0 + nudge, r[1] + 2.0 + nudge, 1.0);
        hits.push((r, Hit::BookmarkButton));
        if inside(game.cursor, r) {
            let first = if on { "bookmarks_disable" } else { "bookmarks_enable" };
            tip = Some(Tip::text(vec![
                (jei::tr(ru, first).to_owned(), WHITE),
                (jei::tr(ru, "bookmarks_usage").to_owned(), 0xffaa_aaaa),
            ]));
        }
    }

    if let (true, Some(g)) = (shown, laid) {
        let per = g.cells.len();
        s.bookmark_per = per;
        let pages = jei::pages(ids.len(), per);
        s.bookmark_page = s.bookmark_page.min(pages - 1);
        let bg: Rect = [g.nav[0], g.nav[1], g.rect[2], g.rect[1] + g.rect[3] - g.nav[1]];
        hits.insert(0, (bg, Hit::Area));
        let (back, next) = page_row(game, batch, g.nav, s.bookmark_page, pages);
        hits.push((back, Hit::BookmarkPage(-1)));
        hits.push((next, Hit::BookmarkPage(1)));
        for (&(x, y), &id) in g.cells.iter().zip(ids.iter().skip(s.bookmark_page * per)) {
            let Some(ingredient) = data.ingredient(id) else {
                continue;
            };
            let cell = [x, y, CELL, CELL];
            let hovered = inside(game.cursor, cell);
            if hovered {
                highlight(game, batch, x + 1.0, y + 1.0, 16.0, true);
            }
            item(game, batch, &ingredient.stack, x + 1.0, y + 1.0);
            if hovered {
                highlight(game, batch, x + 1.0, y + 1.0, 16.0, false);
                tip = Some(Tip::stack(ingredient.stack.clone()));
            }
            hits.push((cell, Hit::Bookmark(id)));
        }
    }
    s.hits.extend(hits);
    tip
}

// ---- the recipes screen (`RecipesGui`) ----------------------------------------------

/// A category's size (`getWidth`, `getHeight`).
fn category_size(game: &Game<'_>, ru: bool, c: Category) -> (f32, f32) {
    match c {
        Category::Crafting | Category::Smelting => (116.0, 54.0),
        Category::Smoking | Category::Blasting => (82.0, 54.0),
        Category::Campfire => (82.0, 44.0),
        Category::Stonecutting => (82.0, 34.0),
        Category::SmeltingFuel | Category::SmokingFuel | Category::BlastingFuel => {
            let widest = if ru { "Плавит 10 000 000 предметов" } else { "Smelts 10,000,000 items" };
            ((18.0 + 20.0 + width(game, widest)).ceil(), 34.0)
        }
        Category::Brewing => (114.0, 61.0),
        Category::Anvil => (125.0, 38.0),
        Category::Grindstone => (73.0, 52.0),
        Category::Smithing => (108.0, 28.0),
        Category::Compostable => (120.0, 18.0),
    }
}

/// A category's title: JEI's, or the game's own block name.
fn title(game: &Game<'_>, ru: bool, c: Category) -> String {
    match c {
        Category::Anvil => game.translate("block.minecraft.anvil", jei::category_title(ru, c)),
        Category::Smithing => game.translate("block.minecraft.smithing_table", jei::category_title(ru, c)),
        _ => jei::category_title(ru, c).to_owned(),
    }
}

/// Draws the recipes screen; returns the area the overlays keep beside (the
/// window and the catalyst tab left of it) and the tooltip.
fn recipes_screen(game: &Game<'_>, batch: &mut HudBatch, s: &mut State, millis: u64) -> Option<(Rect, Option<Tip>)> {
    let category = s.recipes.category()?;
    let ru = s.ru;
    let mod_name = s.settings.is_some_and(|x| x.mod_name);
    let badges = s.settings.is_none_or(|x| x.badges);
    let data = s.data.as_ref()?;
    let recipes: Vec<usize> = s.recipes.recipes().to_vec();
    let (cat_w, cat_h) = category_size(game, ru, category);
    let transfer = category == Category::Crafting && game.mod_recipe_screen();
    let total_w = cat_w + if transfer { 2.0 * RECIPE_BORDER + 2.0 + RECIPE_BUTTON } else { 2.0 * RECIPE_BORDER };
    let border_w = cat_w + 2.0 * RECIPE_BORDER;
    let border_h = cat_h + 2.0 * RECIPE_BORDER;

    // The window (`RecipeGuiSizing`), stretched for wide recipes.
    let catalysts: Vec<u32> = data.index.catalysts(category).to_vec();
    let room = batch.height - 58.0;
    let tall = room.max(MIN_HEIGHT);
    let h = tall.min(MAX_HEIGHT);
    let extra = tall - h;
    let x_size = MIN_WIDTH;
    let gui_left = ((batch.width - x_size) / 2.0).floor();
    let gui_top = 45.0 + (extra / 2.0).floor();
    let area_w = (total_w.max(x_size - 12.0) + 12.0).floor();
    let area: Rect = [((batch.width - area_w) / 2.0).floor(), gui_top, area_w, h];
    let cursor = game.cursor;
    let mut hits: Vec<(Rect, Hit)> = vec![(area, Hit::Area)];
    let mut tip: Option<Tip> = None;

    nine(game, batch, &sprites::GUI_BACKGROUND, area);

    // The category row and the page row.
    let cat_y = gui_top + 4.0;
    let page_y = cat_y + SMALL + 2.0;
    let left_x = gui_left + PADDING;
    let right_x = gui_left + x_size - PADDING - SMALL;
    let tabs: Vec<Category> = s.recipes.tabs.iter().map(|(c, _)| *c).collect();
    fill(game, batch, [left_x + SMALL, cat_y, right_x - left_x - SMALL, SMALL], NAV_FILL);
    arrow_button(game, batch, [left_x, cat_y, SMALL, SMALL], true, tabs.len() > 1);
    arrow_button(game, batch, [right_x, cat_y, SMALL, SMALL], false, tabs.len() > 1);
    hits.push(([left_x, cat_y, SMALL, SMALL], Hit::CategoryTurn(-1)));
    hits.push(([right_x, cat_y, SMALL, SMALL], Hit::CategoryTurn(1)));
    let title_area = [gui_left + 33.0, cat_y, x_size - 66.0, SMALL];
    let full = title(game, ru, category);
    let shown_title = fit(game, &full, title_area[2]);
    let tw = width(game, &shown_title);
    let title_rect = [title_area[0] + ((title_area[2] - tw) / 2.0).round(), cat_y + 2.0, tw, 9.0];
    text(game, batch, &shown_title, title_rect[0], title_rect[1], WHITE, true);
    hits.push((title_rect, Hit::Title));
    if inside(cursor, title_rect) {
        let mut lines = Vec::new();
        if shown_title != full {
            lines.push((full.clone(), WHITE));
        }
        if !matches!(s.recipes.lookup, Some(Lookup::Category(_))) {
            lines.push((jei::tr(ru, "show_all_hotkey").to_owned(), 0xffaa_aaaa));
        }
        if !lines.is_empty() {
            tip = Some(Tip::text(lines));
        }
    }

    // The recipes' grid (`RecipeGuiGrid`).
    let layouts: Rect = [area[0] + PADDING, area[1] + 34.0, area[2] - 2.0 * PADDING, area[3] - 40.0];
    let ideal_w = x_size - 2.0 * PADDING;
    let rows = (((layouts[3] + RECIPE_GAP) / (border_h + RECIPE_GAP)).floor() as usize).max(1);
    let cols = (((ideal_w + RECIPE_GAP) / (total_w + RECIPE_GAP)).floor() as usize).clamp(1, MAX_RECIPE_COLUMNS);
    let per_page = rows * cols;
    s.per_page = per_page;
    let pages = if recipes.len() <= 1 { 1 } else { recipes.len().div_ceil(per_page) };
    s.recipes.page = s.recipes.page.min(pages - 1);
    fill(game, batch, [left_x + SMALL, page_y, right_x - left_x - SMALL, SMALL], NAV_FILL);
    arrow_button(game, batch, [left_x, page_y, SMALL, SMALL], true, recipes.len() > per_page);
    arrow_button(game, batch, [right_x, page_y, SMALL, SMALL], false, recipes.len() > per_page);
    hits.push(([left_x, page_y, SMALL, SMALL], Hit::RecipePage(-1)));
    hits.push(([right_x, page_y, SMALL, SMALL], Hit::RecipePage(1)));
    let label = format!("{}/{}", s.recipes.page + 1, pages);
    let lw = width(game, &label);
    text(game, batch, &label, left_x + ((x_size - 2.0 * PADDING - lw) / 2.0).round(), page_y + 2.0, WHITE, true);

    let col_w = ((layouts[2] - (cols - 1) as f32 * RECIPE_GAP) / cols as f32).floor();
    let spacing = (((layouts[3].max(border_h)) - rows as f32 * border_h) / (rows + 1) as f32).floor();
    let focus = s.recipes.lookup.and_then(Lookup::focus);
    let ctx = Context { game, data, ru, millis, focus, cursor, badges };
    for (i, &ri) in recipes.iter().skip(s.recipes.page * per_page).take(per_page).enumerate() {
        let Some(recipe) = data.index.recipes.get(ri) else {
            continue;
        };
        let col_x = layouts[0] + (i % cols) as f32 * (col_w + RECIPE_GAP);
        let buttons = total_w - border_w;
        let rx = if col_w > border_w + 2.0 * buttons {
            col_x + ((col_w - border_w) / 2.0).floor()
        } else {
            col_x + ((col_w - total_w) / 2.0).floor()
        };
        let ry = layouts[1] + spacing + (i / cols) as f32 * (border_h + spacing);
        nine(game, batch, &sprites::RECIPE_BACKGROUND, [rx, ry, border_w, border_h]);
        let (ox, oy) = (rx + RECIPE_BORDER, ry + RECIPE_BORDER);
        let (slot_hits, slot_tip) = draw_recipe(&ctx, batch, category, recipe, ox, oy, cat_w, cat_h);
        hits.extend(slot_hits);
        if slot_tip.is_some() {
            tip = slot_tip;
        }
        if transfer {
            let br = [ox + cat_w + RECIPE_BORDER + 2.0, oy + cat_h + RECIPE_BORDER - RECIPE_BUTTON, RECIPE_BUTTON, RECIPE_BUTTON];
            let fits = glue::can_transfer(game, recipe);
            button(game, batch, br, fits, false);
            blit(game, batch, &sprites::ICON_TRANSFER, br[0] + 3.0, br[1] + 3.0, 1.0);
            hits.push((br, if fits { Hit::Transfer(ri) } else { Hit::Area }));
            if inside(cursor, br) {
                let mut lines = vec![(jei::tr(ru, "transfer").to_owned(), WHITE)];
                if !fits {
                    lines.push((jei::tr(ru, "too_large").to_owned(), 0xffff_5555));
                }
                tip = Some(Tip::text(lines));
            }
        }
    }

    // The catalysts (`CraftingStations`), left of the window.
    let mut left_extra = 0.0;
    if !catalysts.is_empty() {
        let avail_h = area[3] - 44.0 - 8.0;
        let mut per_col = (((avail_h - 12.0) / 16.0).floor() as usize).max(1);
        let cols = catalysts.len().div_ceil(per_col);
        per_col = catalysts.len().div_ceil(cols);
        let w = 12.0 + 16.0 * cols as f32;
        let h = 12.0 + 16.0 * per_col as f32;
        let left = area[0] - w + 6.0;
        let top = area[1];
        let tab = [left, top, w, h];
        left_extra = w - 6.0;
        nine(game, batch, &sprites::CATALYST_TAB, tab);
        nine(game, batch, &sprites::CATALYST_SLOT, [left + 5.0, top + 5.0, w - 10.0, h - 10.0]);
        hits.push((tab, Hit::Area));
        for (i, &id) in catalysts.iter().enumerate() {
            let (col, row) = (i / per_col, i % per_col);
            let x = left + 5.0 + 16.0 * col as f32 + 1.0;
            let y = top + 5.0 + 16.0 * row as f32 + 1.0;
            let Some(ingredient) = data.ingredient(id) else {
                continue;
            };
            let cell = [x, y, 16.0, 16.0];
            let hovered = inside(cursor, cell);
            if hovered {
                highlight(game, batch, x, y, 16.0, true);
            }
            item(game, batch, &ingredient.stack, x, y);
            if hovered {
                highlight(game, batch, x, y, 16.0, false);
                tip = Some(Tip::stack(ingredient.stack.clone()));
            }
            hits.push((cell, Hit::Catalyst(id)));
        }
    }

    // The category tabs (`RecipeGuiTabs`), over the window's top edge.
    let per_tab_page = (((x_size - 4.0) / TAB).floor() as usize).clamp(1, tabs.len().max(1));
    let tab_page = s.recipes.tab / per_tab_page;
    let tab_pages = tabs.len().div_ceil(per_tab_page);
    for (n, &c) in tabs.iter().enumerate().skip(tab_page * per_tab_page).take(per_tab_page) {
        let tx = gui_left + 2.0 + (n - tab_page * per_tab_page) as f32 * TAB;
        let ty = gui_top - 21.0;
        let sprite = if n == s.recipes.tab { &sprites::TAB_SELECTED } else { &sprites::TAB_UNSELECTED };
        blit(game, batch, sprite, tx, ty, 1.0);
        if let Some(i) = data.index.catalysts(c).first().and_then(|&id| data.ingredient(id)) {
            item(game, batch, &i.stack, tx + 4.0, ty + 4.0);
            // Fuel tabs: the station with a small flame (`IconWithFlameOverlay`).
            if matches!(c, Category::SmeltingFuel | Category::SmokingFuel | Category::BlastingFuel) {
                blit(game, batch, &sprites::ICON_FLAME, tx + 12.0, ty + 12.0, 0.5);
            }
        }
        let r = [tx, ty, TAB, TAB];
        if inside(cursor, r) {
            let mut lines = vec![(title(game, ru, c), WHITE)];
            if mod_name {
                lines.push(("Minecraft".to_owned(), 0xff55_55ff));
            }
            tip = Some(Tip::text(lines));
        }
        hits.push((r, Hit::Tab(n)));
    }
    if tab_pages > 1 {
        let nav = [gui_left + 2.0, gui_top - 43.0, x_size - 4.0, ROW];
        let (back, next) = page_row(game, batch, nav, tab_page, tab_pages);
        hits.push((back, Hit::TabPage(-1)));
        hits.push((next, Hit::TabPage(1)));
        s.tab_scroll = per_tab_page;
    }

    s.hits.extend(hits);
    let gui = [area[0] - left_extra, area[1], area[2] + left_extra, area[3]];
    Some((gui, tip))
}

struct Context<'a, 'g> {
    game: &'a Game<'g>,
    data: &'a Data,
    ru: bool,
    millis: u64,
    focus: Option<u32>,
    cursor: (f32, f32),
    badges: bool,
}

impl Context<'_, '_> {
    /// What a slot shows now: the focus when it holds it, else one of its
    /// stacks a second (`CycleTicker`).
    fn shown(&self, stacks: &[Stack], offset: usize) -> Option<Stack> {
        if let Some(f) = self.focus
            && let Some(s) = stacks.iter().find(|s| s.id == f)
        {
            return Some(*s);
        }
        stacks.get(jei::cycle(stacks.len(), self.millis, offset)).copied()
    }
    /// Its candidates are different items that share an item tag (JEI's tag
    /// badge; one item's variants get the list badge).
    fn tagged(&self, stacks: &[Stack]) -> bool {
        let first = stacks.first().and_then(|s| self.data.ingredient(s.id)).map(|i| i.stack.item.clone());
        if stacks.iter().all(|s| self.data.ingredient(s.id).map(|i| i.stack.item.clone()) == first) {
            return false;
        }
        let mut tags: Option<Vec<&String>> = None;
        for s in stacks {
            let Some(i) = self.data.ingredient(s.id) else {
                return false;
            };
            let mine: Vec<&String> = i.tags().iter().collect();
            tags = Some(match tags {
                None => mine,
                Some(t) => t.into_iter().filter(|x| mine.contains(x)).collect(),
            });
        }
        tags.is_some_and(|t| !t.is_empty())
    }
}

/// How a slot looks.
#[derive(Clone, Copy, PartialEq)]
enum Frame {
    /// No background (drawn by the category's own picture).
    None,
    /// `slot`, 18x18, hover 18x18.
    Standard,
    /// `output_slot`, 26x26, hover 16x16.
    Output,
}

struct Slot<'a> {
    stacks: &'a [Stack],
    x: f32,
    y: f32,
    frame: Frame,
    offset: usize,
    /// Lines under the item's name in its tooltip.
    lines: Vec<(String, u32)>,
}

fn slot(ctx: &Context<'_, '_>, batch: &mut HudBatch, hits: &mut Vec<(Rect, Hit)>, slot: Slot<'_>) -> Option<Tip> {
    let game = ctx.game;
    match slot.frame {
        Frame::Standard => blit(game, batch, &sprites::SLOT, slot.x - 1.0, slot.y - 1.0, 1.0),
        Frame::Output => blit(game, batch, &sprites::OUTPUT_SLOT, slot.x - 5.0, slot.y - 5.0, 1.0),
        Frame::None => {}
    }
    let shown = ctx.shown(slot.stacks, slot.offset)?;
    let stack = ctx.data.stack(shown)?;
    let hover = if slot.frame == Frame::Standard {
        [slot.x - 1.0, slot.y - 1.0, CELL, CELL]
    } else {
        [slot.x, slot.y, 16.0, 16.0]
    };
    let hovered = inside(ctx.cursor, hover);
    if hovered {
        highlight(game, batch, slot.x, slot.y, 16.0, true);
    }
    item(game, batch, &stack, slot.x, slot.y);
    if ctx.badges && slot.stacks.len() > 1 {
        let badge = if ctx.tagged(slot.stacks) { &sprites::ICON_TAG } else { &sprites::ICON_LIST };
        blit(game, batch, badge, slot.x + 8.0, slot.y - 1.0, 1.0);
    }
    hits.push((hover, Hit::Slot(shown.id)));
    if hovered {
        highlight(game, batch, slot.x, slot.y, 16.0, false);
        return Some(Tip { stack: Some(stack), lines: slot.lines });
    }
    None
}

/// The recipe ID line JEI adds to an output's tooltip with Shift or F3+H.
fn recipe_id_lines(ctx: &Context<'_, '_>, r: &Recipe) -> Vec<(String, u32)> {
    let advanced = ctx.game.settings.file.bool("advancedItemTooltips");
    if r.id.is_empty() || !(advanced || ctx.game.has_shift_down()) {
        return Vec::new();
    }
    vec![(format!("{}{}", jei::tr(ctx.ru, "recipe_id"), r.id), DARK_GREY)]
}

/// The animated arrow, filled over `ticks` (`addAnimatedRecipeArrowWidget`).
fn arrow(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32, ticks: u32, millis: u64) {
    blit(game, batch, &sprites::RECIPE_ARROW, x, y, 1.0);
    let cycle = u64::from(ticks.max(1)) * 50;
    let w = ((millis % cycle) * 23 / cycle).min(22) as u16;
    if w > 0 {
        blit_part(game, batch, &sprites::RECIPE_ARROW_FILLED, (x, y), (0, w), (0, 16));
    }
}

/// The flame, burning down over `ticks` (`addAnimatedRecipeFlameWidget`).
fn flame(game: &Game<'_>, batch: &mut HudBatch, x: f32, y: f32, ticks: u32, millis: u64) {
    blit(game, batch, &sprites::ICON_FLAME_EMPTY, x, y, 1.0);
    let cycle = u64::from(ticks.max(1)) * 50;
    let top = ((millis % cycle) * 15 / cycle).min(14) as u16;
    blit_part(game, batch, &sprites::ICON_FLAME, (x, y + f32::from(top)), (0, 14), (top, 14));
}

#[allow(clippy::too_many_arguments)]
fn draw_recipe(
    ctx: &Context<'_, '_>,
    batch: &mut HudBatch,
    category: Category,
    r: &Recipe,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
) -> (Vec<(Rect, Hit)>, Option<Tip>) {
    let game = ctx.game;
    let ru = ctx.ru;
    let millis = ctx.millis;
    let mut hits = Vec::new();
    let mut tip = None;
    let empty: Vec<Stack> = Vec::new();
    let input = |i: usize| r.inputs.get(i).unwrap_or(&empty);
    let output = r.outputs.first().unwrap_or(&empty);
    let mut add = |batch: &mut HudBatch, hits: &mut Vec<(Rect, Hit)>, stacks: &[Stack], sx: f32, sy: f32, frame: Frame, offset: usize, lines: Vec<(String, u32)>| {
        let t = slot(ctx, batch, hits, Slot { stacks, x: x + sx, y: y + sy, frame, offset, lines });
        if t.is_some() {
            tip = t;
        }
    };
    let id_lines = recipe_id_lines(ctx, r);
    let link = r.focus_link(ctx.focus);
    match category {
        Category::Crafting => {
            add(batch, &mut hits, output, 95.0, 19.0, Frame::Output, 0, id_lines);
            for i in 0..9 {
                let (c, row) = ((i % 3) as f32, (i / 3) as f32);
                add(batch, &mut hits, input(i), c * 18.0 + 1.0, row * 18.0 + 1.0, Frame::Standard, 0, Vec::new());
            }
        }
        Category::Smelting | Category::Smoking | Category::Blasting | Category::Campfire => {
            let fuel_slot = category != Category::Campfire;
            add(batch, &mut hits, input(0), 1.0, 1.0, Frame::Standard, 0, Vec::new());
            if fuel_slot && !r.fuel.is_empty() {
                let lines = vec![(jei::tr(ru, "any_fuel").to_owned(), GOLD)];
                add(batch, &mut hits, &r.fuel, 1.0, 37.0, Frame::Standard, 0, lines);
            }
            let out_x = w - 21.0;
            let out_y = if fuel_slot { 19.0 } else { 9.0 };
            add(batch, &mut hits, output, out_x, out_y, Frame::Output, 0, id_lines);
        }
        Category::Stonecutting => {
            add(batch, &mut hits, input(0), 1.0, 9.0, Frame::Standard, 0, Vec::new());
            add(batch, &mut hits, output, 61.0, 9.0, Frame::Output, 0, id_lines);
        }
        Category::SmeltingFuel | Category::SmokingFuel | Category::BlastingFuel => {
            add(batch, &mut hits, input(0), 1.0, 17.0, Frame::Standard, 0, Vec::new());
        }
        Category::Brewing => {
            blit(game, batch, &sprites::BREWING_BACKGROUND, x, y + 1.0, 1.0);
            blit(game, batch, &sprites::BREWING_HEAT, x + 5.0, y + 30.0, 1.0);
            add(batch, &mut hits, input(0), 1.0, 37.0, Frame::None, 0, Vec::new());
            add(batch, &mut hits, input(0), 24.0, 44.0, Frame::None, 0, Vec::new());
            add(batch, &mut hits, input(0), 47.0, 37.0, Frame::None, 0, Vec::new());
            add(batch, &mut hits, input(1), 24.0, 3.0, Frame::None, 0, Vec::new());
            add(batch, &mut hits, output, 81.0, 3.0, Frame::Standard, 0, id_lines);
        }
        // The book and the item it makes, or the enchanted item and the one
        // the grindstone gives back, show the same alternative (JEI's focus link).
        Category::Anvil => {
            add(batch, &mut hits, &link.input(r, 0), 1.0, 1.0, Frame::Standard, 0, Vec::new());
            add(batch, &mut hits, &link.input(r, 1), 50.0, 1.0, Frame::Standard, 0, Vec::new());
            add(batch, &mut hits, &link.output(r), 108.0, 1.0, Frame::Standard, 0, Vec::new());
        }
        Category::Grindstone => {
            add(batch, &mut hits, &link.input(r, 0), 1.0, 1.0, Frame::Standard, 0, Vec::new());
            add(batch, &mut hits, &link.input(r, 1), 1.0, 24.0, Frame::Standard, 0, Vec::new());
            add(batch, &mut hits, &link.output(r), 52.0, 13.0, Frame::Output, 0, Vec::new());
        }
        Category::Smithing => {
            for i in 0..3 {
                add(batch, &mut hits, input(i), 1.0 + 18.0 * i as f32, 6.0, Frame::Standard, 0, Vec::new());
            }
            add(batch, &mut hits, output, 91.0, 6.0, Frame::Standard, 0, id_lines);
        }
        Category::Compostable => {
            add(batch, &mut hits, input(0), 1.0, 1.0, Frame::Standard, 0, Vec::new());
        }
    }
    // The widgets and texts, over the slots.
    match category {
        Category::Crafting => {
            blit(game, batch, &sprites::RECIPE_ARROW, x + 61.0, y + 19.0, 1.0);
            if r.shapeless {
                let icon = [x + w - 8.0, y, 8.0, 8.0];
                blit(game, batch, &sprites::ICON_SHAPELESS, icon[0], icon[1], 0.25);
                if inside(ctx.cursor, icon) && tip.is_none() {
                    tip = Some(Tip::text(vec![(jei::tr(ru, "shapeless").to_owned(), WHITE)]));
                }
            }
        }
        Category::Smelting | Category::Smoking | Category::Blasting | Category::Campfire => {
            let fuel_slot = category != Category::Campfire;
            flame(game, batch, x + 1.0, y + 20.0, 300, millis);
            let arrow_x = if w == 82.0 { 26.0 } else { ((w - 24.0) / 2.0).floor() };
            let arrow_y = if fuel_slot { 17.0 } else { 7.0 };
            arrow(game, batch, x + arrow_x, y + arrow_y, r.ticks, millis);
            if fuel_slot && r.experience > 0.0 {
                let t = jei::experience(ru, r.experience);
                let tw = width(game, &t);
                text(game, batch, &t, x + w - tw, y, GREY, false);
            }
            let t = jei::cooking_time(ru, r.ticks);
            let tw = width(game, &t);
            text(game, batch, &t, x + 21.0 + ((w - 42.0 - tw) / 2.0).round(), y + h - 8.0, GREY, false);
        }
        Category::Stonecutting => {
            blit(game, batch, &sprites::RECIPE_ARROW, x + 26.0, y + 9.0, 1.0);
        }
        Category::SmeltingFuel | Category::SmokingFuel | Category::BlastingFuel => {
            flame(game, batch, x + 1.0, y, r.ticks, millis);
            let t = jei::smelt_count(ru, r.ticks, r.item_ticks);
            let tw = width(game, &t);
            text(game, batch, &t, x + 20.0 + ((w - 20.0 - tw) / 2.0).round(), y + 13.0, GREY, false);
        }
        Category::Brewing => {
            // Bubbles rise in 7 frames of 100 ms; the arrow fills over 400 ticks.
            const LEVELS: [u16; 7] = [29, 23, 18, 13, 9, 5, 0];
            let frame = ((millis % 700) / 100) as usize;
            let top = LEVELS[frame.min(6)];
            if top < 28 {
                blit_part(game, batch, &sprites::BREWING_BUBBLES, (x + 9.0, y + 1.0 + f32::from(top)), (0, 11), (top, 28));
            }
            let rows = ((millis % 20_000) * 28 / 20_000).min(27) as u16;
            if rows > 0 {
                blit_part(game, batch, &sprites::BREWING_ARROW, (x + 43.0, y + 3.0), (0, 7), (0, rows));
            }
            let t = jei::brewing_steps_text(ru, r.steps);
            text(game, batch, &t, x + 70.0, y + 28.0, GREY, false);
        }
        Category::Anvil => {
            blit(game, batch, &sprites::RECIPE_PLUS, x + 27.0, y + 3.0, 1.0);
            blit(game, batch, &sprites::RECIPE_ARROW, x + 76.0, y + 1.0, 1.0);
            // The cost of the book shown now (the shown output's).
            let cost = r.costs.get(link.index(r.costs.len(), millis)).copied().unwrap_or(r.cost);
            if cost > 0 {
                let template = game.translate("container.repair.cost", "Enchantment Cost: %1$s");
                let t = template.replace("%1$s", &cost.to_string()).replace("%s", &cost.to_string());
                let tw = width(game, &t);
                let color = if game.creative || cost < 40 { GREEN } else { GREEN_ERROR };
                text(game, batch, &t, x + 123.0 - tw, y + 27.0, color, true);
            }
        }
        Category::Grindstone => {
            blit(game, batch, &sprites::RECIPE_ARROW, x + 20.0, y + 12.0, 1.0);
            if r.cost > 0 {
                let t = jei::grindstone_xp(ru, r.cost);
                let tw = width(game, &t);
                text(game, batch, &t, x + 73.0 - tw, y + 43.0, GREEN, true);
            }
        }
        Category::Smithing => {
            blit(game, batch, &sprites::RECIPE_ARROW, x + 61.0, y + 6.0, 1.0);
        }
        Category::Compostable => {
            let t = jei::compost_chance(ru, r.chance);
            let tw = width(game, &t);
            text(game, batch, &t, x + 24.0 + ((96.0 - tw) / 2.0).round(), y + 5.0, GREY, false);
        }
    }
    (hits, tip)
}
