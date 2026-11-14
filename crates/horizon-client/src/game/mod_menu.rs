// Mod Menu for Horizon by tariksabanuz (MIT, github.com/tariksabanuz/ModMenuHorizon), built into
// Skyline, which adds the Java mods of each runtime (Fabric 26.3, Fabric 1.20.1) to its list.
// A Mods button on the title screen (beside Accessibility) opens a Mods screen laid out like the
// Java Mod Menu's: a search box, a filter, a list of every installed mod and a detail pane for
// the one you pick. That button is the only way in.
//
// This file needs nothing from the game: layout, search, filtering, wrapping and the like are
// plain functions. The drawing and the input are in `mod_menu_ui.rs`.
use horizon_client::lang;

pub const ID: &str = "mod_menu";
pub const NAME: &str = "Mod Menu";
pub const VERSION: &str = "0.3.5";
pub const AUTHOR: &str = "tariksabanuz";
/// Its description, a lang key.
pub const DESCRIPTION: &str = "horizon.mod_menu.description";

/// One installed mod, as the Mods page knows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String,
    /// `Name 1.0`.
    pub title: String,
    pub description: String,
    pub on: bool,
    /// A native mod (has code); otherwise a content mod.
    pub native: bool,
    /// Loaded with `--dev-mod`.
    pub dev: bool,
    /// Why it is not running.
    pub problem: Option<String>,
    /// Options > Mods has a Settings button for it (a native mod).
    pub has_settings: bool,
    /// A Java mod: its runtime's name, a lang key (`horizon.mods.runtime.fabric_1_20_1`).
    pub runtime: Option<String>,
    /// A Java mod whose settings screen the world's server can open: its index in
    /// `Mods::java` (game/java_screen.rs).
    pub java_settings: Option<usize>,
    /// A Java mod's own icon (`mod_list::RUNTIME_ICON` square, RGBA).
    pub icon: Option<std::sync::Arc<Vec<u8>>>,
}

/// Mod Menu itself: it can't be turned off from here and has no settings, so its detail pane has
/// no buttons.
pub fn is_self(e: &Entry) -> bool {
    e.id == ID
}

// ---------------------------------------------------------------------------------------------
// The Mods screen: everything about it that needs no game.

/// A rectangle in GUI pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

impl Rect {
    pub const fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self { x, y, w, h }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x < self.x + self.w && y >= self.y && y < self.y + self.h
    }
}

/// Where the list starts (below the search box) and how tall the footer is, in GUI pixels.
pub const HEADER: f32 = 48.0;
pub const FOOTER: f32 = 44.0;
/// One list row: a 32 pixel icon with a pixel of air above and below.
pub const ROW: f32 = 36.0;
pub const ICON: f32 = 32.0;
pub const BUTTON_H: f32 = 20.0;
pub const LINE: f32 = 10.0;
/// What one wheel step scrolls.
pub const SCROLL_STEP: f32 = 18.0;
/// The search box takes this many characters.
pub const MAX_SEARCH: usize = 48;

/// Where everything on the screen is, for a screen `w` by `h` GUI pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub search: Rect,
    pub filter: Rect,
    /// Where "Showing 6 mods" starts.
    pub showing: [f32; 2],
    pub list: Rect,
    pub detail: Rect,
    pub folder: Rect,
    pub done: Rect,
    /// The top of the line of small print under the footer buttons.
    pub hint_y: f32,
}

pub fn layout(w: f32, h: f32) -> Layout {
    let list_w = (w * 0.45).clamp(150.0, 320.0).min((w - 100.0).max(100.0));
    let body_h = (h - HEADER - FOOTER).max(40.0);
    let button_w = 150.0_f32.min(((w - 16.0) / 2.0).max(60.0));
    let mid = (w / 2.0).floor();
    Layout {
        search: Rect::new(6.0, 12.0, list_w - 12.0 - 24.0, BUTTON_H),
        filter: Rect::new(list_w - 6.0 - BUTTON_H, 12.0, BUTTON_H, BUTTON_H),
        showing: [6.0, 36.0],
        list: Rect::new(0.0, HEADER, list_w, body_h),
        detail: Rect::new(list_w + 6.0, HEADER, (w - list_w - 12.0).max(40.0), body_h),
        folder: Rect::new(mid - button_w - 4.0, h - FOOTER + 8.0, button_w, BUTTON_H),
        done: Rect::new(mid + 4.0, h - FOOTER + 8.0, button_w, BUTTON_H),
        hint_y: h - 12.0,
    }
}

/// The filter button cycles through these.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Filter {
    All,
    On,
    Off,
    Native,
    Content,
    Java,
}

impl Filter {
    pub fn next(self) -> Filter {
        match self {
            Filter::All => Filter::On,
            Filter::On => Filter::Off,
            Filter::Off => Filter::Native,
            Filter::Native => Filter::Content,
            Filter::Content => Filter::Java,
            Filter::Java => Filter::All,
        }
    }

    pub fn label(self) -> String {
        lang::get(match self {
            Filter::All => "horizon.mod_menu.filter.all",
            Filter::On => "horizon.mod_menu.filter.on",
            Filter::Off => "horizon.mod_menu.filter.off",
            Filter::Native => "horizon.mod_menu.filter.native",
            Filter::Content => "horizon.mod_menu.filter.content",
            Filter::Java => "horizon.mod_menu.filter.java",
        })
    }

    pub fn accepts(self, e: &Entry) -> bool {
        match self {
            Filter::All => true,
            Filter::On => e.on && e.problem.is_none(),
            Filter::Off => !e.on,
            Filter::Native => e.native,
            Filter::Content => !e.native && e.runtime.is_none(),
            Filter::Java => e.runtime.is_some(),
        }
    }
}

/// `Name 1.0` as the Mods page titles a mod: the name and, when the last word starts like a
/// version, the version.
pub fn name_and_version(title: &str) -> (&str, Option<&str>) {
    let t = title.trim();
    if let Some((name, last)) = t.rsplit_once(' ') {
        let digits = last.strip_prefix('v').unwrap_or(last);
        if !name.trim().is_empty() && digits.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            return (name.trim(), Some(last));
        }
    }
    (t, None)
}

fn matches_screen(e: &Entry, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    q.is_empty()
        || e.title.to_lowercase().contains(&q)
        || e.id.to_lowercase().contains(&q)
        || e.description.to_lowercase().contains(&q)
}

/// The mods the list shows, as indexes into `entries`, by name.
pub fn visible(entries: &[Entry], query: &str, filter: Filter) -> Vec<usize> {
    let mut found: Vec<usize> = (0..entries.len())
        .filter(|&i| filter.accepts(&entries[i]) && matches_screen(&entries[i], query))
        .collect();
    found.sort_by_key(|&i| (name_and_version(&entries[i].title).0.to_lowercase(), i));
    found
}

/// "Showing 6 mods".
pub fn showing(count: usize) -> String {
    if count == 1 {
        lang::get("horizon.mod_menu.showing_one")
    } else {
        lang::get("horizon.mod_menu.showing").replace("%s", &count.to_string())
    }
}

/// How far the list of `rows` rows scrolls in a view `view_h` tall.
pub fn max_scroll(rows: usize, view_h: f32) -> f32 {
    (rows as f32 * ROW - view_h).max(0.0)
}

/// The row under `(x, y)` of a list scrolled by `scroll` (the scroll bar's strip is not a row).
pub fn row_at(l: &Layout, scroll: f32, count: usize, x: f32, y: f32) -> Option<usize> {
    if x < l.list.x || x >= l.list.x + l.list.w - 6.0 || y < l.list.y || y >= l.list.y + l.list.h {
        return None;
    }
    let row = ((y - l.list.y + scroll) / ROW).floor();
    if row < 0.0 {
        return None;
    }
    let row = row as usize;
    (row < count).then_some(row)
}

/// The row `delta` rows from `current` (none picks the first or the last), kept in the list.
pub fn step(count: usize, current: Option<usize>, delta: i32) -> Option<usize> {
    if count == 0 {
        return None;
    }
    let at = current.map_or(if delta < 0 { count as i32 } else { -1 }, |c| c as i32);
    Some((at + delta).clamp(0, count as i32 - 1) as usize)
}

/// The scroll that shows row `row` of a view `view_h` tall.
pub fn scroll_into_view(scroll: f32, row: usize, view_h: f32) -> f32 {
    let top = row as f32 * ROW;
    if top < scroll {
        top
    } else if top + ROW > scroll + view_h {
        top + ROW - view_h
    } else {
        scroll
    }
}

/// Words broken into lines no wider than `max` (`measure` is the text's width); `\n` starts a
/// line, a word wider than a line is cut where it stops fitting.
pub fn wrap(text: &str, max: f32, measure: &dyn Fn(&str) -> f32) -> Vec<String> {
    let mut lines: Vec<String> = Vec::new();
    for paragraph in text.split('\n') {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            let mut word = word.to_owned();
            while measure(&word) > max && word.chars().count() > 1 {
                let mut cut = String::new();
                for c in word.chars() {
                    let mut next = cut.clone();
                    next.push(c);
                    if measure(&next) > max && !cut.is_empty() {
                        break;
                    }
                    cut = next;
                }
                if !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                }
                let rest: String = word.chars().skip(cut.chars().count()).collect();
                lines.push(cut);
                word = rest;
            }
            if word.is_empty() {
                continue;
            }
            let joined = if line.is_empty() {
                word.clone()
            } else {
                format!("{line} {word}")
            };
            if line.is_empty() || measure(&joined) <= max {
                line = joined;
            } else {
                lines.push(std::mem::replace(&mut line, word));
            }
        }
        lines.push(line);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

/// The end of `text` that fits `max` wide: a search box shows what you are typing.
pub fn tail_fit(text: &str, max: f32, measure: &dyn Fn(&str) -> f32) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut start = 0;
    while start < chars.len() && measure(&chars[start..].iter().collect::<String>()) > max {
        start += 1;
    }
    chars[start..].iter().collect()
}

/// `text`, or its start and "..." when it is wider than `max`.
pub fn ellipsize(text: &str, max: f32, measure: &dyn Fn(&str) -> f32) -> String {
    if measure(text) <= max {
        return text.to_owned();
    }
    let mut out = String::new();
    for c in text.chars() {
        let mut next = out.clone();
        next.push(c);
        if measure(&format!("{next}...")) > max {
            break;
        }
        out = next;
    }
    format!("{}...", out.trim_end())
}

/// Typing into the search box: printable characters only, up to [`MAX_SEARCH`].
pub fn type_text(search: &mut String, text: &str) {
    for c in text.chars().filter(|c| !c.is_control()) {
        if search.chars().count() >= MAX_SEARCH {
            break;
        }
        search.push(c);
    }
}

/// Backspace, or Ctrl+Backspace for the last word.
pub fn backspace(search: &mut String, word: bool) {
    if !word {
        search.pop();
        return;
    }
    while search.ends_with(' ') {
        search.pop();
    }
    while search.chars().last().is_some_and(|c| c != ' ') {
        search.pop();
    }
}

/// A coloured tag beside a name, as the Java Mod Menu's "Client" and "Library".
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Badge {
    pub text: String,
    /// 0xRRGGBB.
    pub rgb: u32,
}

pub fn screen_badges(e: &Entry) -> Vec<Badge> {
    let badge = |key: &str, rgb| Badge {
        text: lang::get(key),
        rgb,
    };
    let mut v = vec![match &e.runtime {
        // Fabric's runtimes in grey; other loaders get colours of their own.
        Some(runtime) if runtime.contains("fabric") => badge(runtime, 0x7F7F7F),
        Some(runtime) => badge(runtime, 0xB5652E),
        None if e.native => badge("horizon.mod_menu.badge.native", 0x3B7DC4),
        None => badge("horizon.mod_menu.badge.content", 0x3F9A55),
    }];
    if e.dev {
        v.push(badge("horizon.mod_menu.badge.dev", 0xB8862B));
    }
    match &e.problem {
        Some(p) if p.starts_with("Installed, not built") => {
            v.push(badge("horizon.mod_menu.badge.not_built", 0xB07A2A))
        }
        Some(_) => v.push(badge("horizon.mod_menu.badge.stopped", 0xB03A3A)),
        None if !e.on => v.push(badge("horizon.mod_menu.badge.off", 0x6E6E6E)),
        None => {}
    }
    v
}

/// A mod's icon colour: the same id is always the same hue.
pub fn icon_rgb(id: &str) -> [u8; 3] {
    let mut hash: u32 = 2_166_136_261;
    for b in id.bytes() {
        hash ^= u32::from(b);
        hash = hash.wrapping_mul(16_777_619);
    }
    hsv((hash % 360) as f32, 0.5, 0.9)
}

fn hsv(h: f32, s: f32, v: f32) -> [u8; 3] {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    [
        ((r + m) * 255.0).round() as u8,
        ((g + m) * 255.0).round() as u8,
        ((b + m) * 255.0).round() as u8,
    ]
}

/// `rgb` made `k` times as bright (above 1 lightens, below 1 darkens).
pub fn shade(rgb: [u8; 3], k: f32) -> [u8; 3] {
    rgb.map(|c| (f32::from(c) * k).round().clamp(0.0, 255.0) as u8)
}

/// How a line of the detail pane is coloured.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Text,
    Dim,
    Bad,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    pub text: String,
    pub tone: Tone,
}

/// The two small lines under a mod's name in the detail pane.
pub fn info_pair(e: &Entry) -> [String; 2] {
    let unknown = lang::get("horizon.mod_menu.unknown");
    let version = name_and_version(&e.title).1.unwrap_or(&unknown);
    let version = lang::get("horizon.mod_menu.version").replace("%s", version);
    if is_self(e) {
        return [
            version,
            lang::get("horizon.mod_menu.by").replace("%s", AUTHOR),
        ];
    }
    let kind = match &e.runtime {
        Some(runtime) => lang::get(runtime),
        None if e.native => lang::get("horizon.mod_menu.kind.native"),
        None => lang::get("horizon.mod_menu.kind.content"),
    };
    let no_id = lang::get("horizon.mod_menu.no_id");
    let id = if e.id.is_empty() {
        no_id.as_str()
    } else {
        e.id.as_str()
    };
    [version, format!("{kind} - {id}")]
}

fn push_wrapped(
    out: &mut Vec<Line>,
    text: &str,
    tone: Tone,
    width: f32,
    measure: &dyn Fn(&str) -> f32,
) {
    for text in wrap(text, width, measure) {
        out.push(Line { text, tone });
    }
}

/// The detail pane's text under the buttons: what it does, and why it is not running if it is not.
pub fn body_lines(e: &Entry, width: f32, measure: &dyn Fn(&str) -> f32) -> Vec<Line> {
    let mut out = Vec::new();
    if e.description.trim().is_empty() {
        let none = lang::get("horizon.mod_menu.no_description");
        push_wrapped(&mut out, &none, Tone::Dim, width, measure);
    } else {
        push_wrapped(&mut out, e.description.trim(), Tone::Text, width, measure);
    }
    if let Some(problem) = &e.problem {
        out.push(Line {
            text: String::new(),
            tone: Tone::Text,
        });
        let problem = lang::get("horizon.mod_menu.problem").replace("%s", problem);
        push_wrapped(&mut out, &problem, Tone::Bad, width, measure);
    }
    out
}

/// Where the detail pane puts things, for a pane scrolled by `scroll`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Detail {
    pub icon: Rect,
    pub name: [f32; 2],
    pub info: [[f32; 2]; 2],
    /// Turn the mod on or off.
    pub toggle: Rect,
    /// The mod's own settings page.
    pub settings: Rect,
    /// The first line of [`body_lines`].
    pub body: [f32; 2],
    /// How wide those lines may be.
    pub width: f32,
}

/// `buttons` is false for Mod Menu itself, whose pane has no Turn off or Settings: the text then
/// starts right under the name.
pub fn detail(l: &Layout, scroll: f32, buttons: bool) -> Detail {
    let px = l.detail.x + 4.0;
    let top = l.detail.y + 4.0 - scroll;
    let text_x = px + ICON + 6.0;
    let button_w = 76.0;
    Detail {
        icon: Rect::new(px, top, ICON, ICON),
        name: [text_x, top + 2.0],
        info: [[text_x, top + 14.0], [text_x, top + 24.0]],
        toggle: Rect::new(px, top + 40.0, button_w, BUTTON_H),
        settings: Rect::new(px + button_w + 4.0, top + 40.0, button_w, BUTTON_H),
        body: [px, top + if buttons { 68.0 } else { 44.0 }],
        width: (l.detail.w - 8.0 - 8.0).max(40.0),
    }
}

/// The detail pane's whole height, to scroll it.
pub fn detail_height(lines: usize, buttons: bool) -> f32 {
    4.0 + if buttons { 68.0 } else { 44.0 } + lines as f32 * LINE + 4.0
}

/// Mod Menu's own icon, a 16 by 16 picture: `#` is white, the rest is the blue square.
pub const MENU_ICON: [&str; 16] = [
    "................",
    "................",
    "..##########.#..",
    "................",
    "..##.#########..",
    "..##.#########..",
    "................",
    "..##.#########..",
    "..##.#########..",
    "................",
    "..##.#########..",
    "..##.#########..",
    "................",
    "..#####..#####..",
    "................",
    "................",
];
pub const MENU_ICON_BLUE: [u8; 3] = [19, 75, 255];

/// The white runs of [`MENU_ICON`] as `(column, row, length)`.
pub fn menu_icon_runs() -> Vec<(u8, u8, u8)> {
    let mut runs = Vec::new();
    for (row, line) in MENU_ICON.iter().enumerate() {
        let bytes = line.as_bytes();
        let mut col = 0;
        while col < bytes.len() {
            if bytes[col] != b'#' {
                col += 1;
                continue;
            }
            let mut end = col;
            while end < bytes.len() && bytes[end] == b'#' {
                end += 1;
            }
            runs.push((col as u8, row as u8, (end - col) as u8));
            col = end;
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: &str, title: &str, on: bool, native: bool) -> Entry {
        Entry {
            id: id.into(),
            title: title.into(),
            description: format!("{title} does things."),
            on,
            native,
            dev: false,
            problem: None,
            has_settings: native,
            runtime: None,
            java_settings: None,
            icon: None,
        }
    }

    fn six(s: &str) -> f32 {
        s.chars().count() as f32 * 6.0
    }

    #[test]
    fn the_layout_fits_every_screen_size() {
        for (w, h) in [
            (320.0, 240.0),
            (427.0, 240.0),
            (480.0, 270.0),
            (640.0, 360.0),
            (960.0, 540.0),
            (1280.0, 720.0),
        ] {
            let l = layout(w, h);
            assert!(
                l.search.x + l.search.w < l.filter.x,
                "{w}x{h}: search and filter overlap"
            );
            assert!(
                l.filter.x + l.filter.w <= l.list.w,
                "{w}x{h}: filter leaves the list"
            );
            assert!(
                l.list.x + l.list.w <= l.detail.x,
                "{w}x{h}: list and detail overlap"
            );
            assert!(
                l.detail.x + l.detail.w <= w,
                "{w}x{h}: detail leaves the screen"
            );
            assert!(
                l.list.y + l.list.h <= l.folder.y,
                "{w}x{h}: list reaches the buttons"
            );
            assert!(
                l.folder.x >= 0.0 && l.folder.x + l.folder.w < l.done.x,
                "{w}x{h}: buttons overlap"
            );
            assert!(l.done.x + l.done.w <= w, "{w}x{h}: Done leaves the screen");
            assert!(
                l.folder.y + l.folder.h <= l.hint_y && l.hint_y + 8.0 <= h,
                "{w}x{h}: small print is cut"
            );
        }
    }

    fn named(id: &str, title: &str, on: bool, native: bool) -> Entry {
        entry(id, title, on, native)
    }

    #[test]
    fn the_list_is_sorted_filtered_and_searched() {
        let all = vec![
            named("zoom", "Zoom 1.0", true, true),
            named("alpha", "alpha 0.1", false, false),
            named("beta", "Beta 2.0", true, false),
        ];
        // By name, ignoring case.
        assert_eq!(visible(&all, "", Filter::All), vec![1, 2, 0]);
        assert_eq!(visible(&all, "", Filter::On), vec![2, 0]);
        assert_eq!(visible(&all, "", Filter::Off), vec![1]);
        assert_eq!(visible(&all, "", Filter::Native), vec![0]);
        assert_eq!(visible(&all, "", Filter::Content), vec![1, 2]);
        assert_eq!(visible(&all, "", Filter::Java), Vec::<usize>::new());
        // The search reads the name, the id and the description.
        assert_eq!(visible(&all, "BETA", Filter::All), vec![2]);
        assert_eq!(visible(&all, "does things", Filter::All).len(), 3);
        assert_eq!(visible(&all, "zoom", Filter::Content), Vec::<usize>::new());
        // No language is loaded in tests: lang::get answers with the key.
        assert_eq!(showing(1), "horizon.mod_menu.showing_one");
        assert_eq!(showing(6), "horizon.mod_menu.showing");
        let mut f = Filter::All;
        for _ in 0..6 {
            f = f.next();
        }
        assert_eq!(f, Filter::All);
        assert_eq!(Filter::Off.label(), "horizon.mod_menu.filter.off");
    }

    #[test]
    fn titles_split_into_name_and_version() {
        assert_eq!(
            name_and_version("Mod Menu 0.2.0"),
            ("Mod Menu", Some("0.2.0"))
        );
        assert_eq!(
            name_and_version("horizon_plus v0.5.0"),
            ("horizon_plus", Some("v0.5.0"))
        );
        assert_eq!(name_and_version("Right Harvest"), ("Right Harvest", None));
        assert_eq!(
            name_and_version("rightharvest.hnmod"),
            ("rightharvest.hnmod", None)
        );
        assert_eq!(name_and_version("2 3"), ("2", Some("3")));
        assert_eq!(name_and_version(" 1.0"), ("1.0", None));
    }

    #[test]
    fn rows_scroll_and_get_picked() {
        let l = layout(427.0, 240.0);
        assert_eq!(max_scroll(2, l.list.h), 0.0);
        assert_eq!(max_scroll(10, 100.0), 260.0);
        // First row, second row, the scroll bar strip, above the list, below the last row.
        assert_eq!(row_at(&l, 0.0, 3, 10.0, l.list.y + 1.0), Some(0));
        assert_eq!(row_at(&l, 0.0, 3, 10.0, l.list.y + ROW + 1.0), Some(1));
        assert_eq!(row_at(&l, 0.0, 3, l.list.w - 3.0, l.list.y + 1.0), None);
        assert_eq!(row_at(&l, 0.0, 3, 10.0, l.list.y - 1.0), None);
        assert_eq!(row_at(&l, 0.0, 3, 10.0, l.list.y + ROW * 3.0 + 1.0), None);
        assert_eq!(row_at(&l, ROW, 3, 10.0, l.list.y + 1.0), Some(1));
        // The arrow keys stay inside the list.
        assert_eq!(step(0, None, 1), None);
        assert_eq!(step(3, None, 1), Some(0));
        assert_eq!(step(3, None, -1), Some(2));
        assert_eq!(step(3, Some(2), 1), Some(2));
        assert_eq!(step(3, Some(0), -1), Some(0));
        assert_eq!(step(30, Some(5), 5), Some(10));
        // And the list follows them.
        assert_eq!(scroll_into_view(0.0, 1, 100.0), 0.0);
        assert_eq!(scroll_into_view(0.0, 4, 100.0), 4.0 * ROW + ROW - 100.0);
        assert_eq!(scroll_into_view(200.0, 2, 100.0), 2.0 * ROW);
    }

    #[test]
    fn text_wraps_to_the_width() {
        assert_eq!(wrap("one two three", 60.0, &six), vec!["one two", "three"]);
        assert_eq!(wrap("one two three", 1000.0, &six), vec!["one two three"]);
        assert_eq!(wrap("a\n\nb", 60.0, &six), vec!["a", "", "b"]);
        assert_eq!(wrap("a\n", 60.0, &six), vec!["a"]);
        assert!(wrap("", 60.0, &six).is_empty());
        // A word wider than a line is cut, and no line is wider than the width.
        let cut = wrap("abcdefghijklmnop", 30.0, &six);
        assert_eq!(cut, vec!["abcde", "fghij", "klmno", "p"]);
        assert!(
            wrap("x abcdefghijklmnop y", 30.0, &six)
                .iter()
                .all(|l| six(l) <= 30.0)
        );
        // Nothing fits: still ends, a character a line.
        assert_eq!(wrap("abc", 1.0, &six), vec!["a", "b", "c"]);
    }

    #[test]
    fn long_text_is_cut_to_fit() {
        assert_eq!(tail_fit("hello", 100.0, &six), "hello");
        assert_eq!(tail_fit("hello world", 36.0, &six), " world");
        assert_eq!(tail_fit("hello world", 35.0, &six), "world");
        assert_eq!(tail_fit("hello", 0.0, &six), "");
        assert_eq!(ellipsize("short", 100.0, &six), "short");
        assert_eq!(ellipsize("a very long name", 60.0, &six), "a very...");
        assert!(six(&ellipsize("a very long name", 60.0, &six)) <= 60.0);
    }

    #[test]
    fn the_search_box_takes_what_you_type() {
        let mut s = String::new();
        type_text(&mut s, "zo");
        type_text(&mut s, "\u{8}\u{16}om x");
        assert_eq!(s, "zoom x");
        backspace(&mut s, false);
        assert_eq!(s, "zoom ");
        type_text(&mut s, "more");
        backspace(&mut s, true);
        assert_eq!(s, "zoom ");
        backspace(&mut s, true);
        assert_eq!(s, "");
        backspace(&mut s, true);
        backspace(&mut s, false);
        assert_eq!(s, "");
        type_text(&mut s, &"x".repeat(100));
        assert_eq!(s.chars().count(), MAX_SEARCH);
    }

    #[test]
    fn badges_say_what_a_mod_is_and_how_it_is_doing() {
        let texts = |e: &Entry| {
            screen_badges(e)
                .into_iter()
                .map(|b| {
                    b.text
                        .trim_start_matches("horizon.mod_menu.badge.")
                        .to_owned()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(texts(&named("a", "A 1", true, true)), ["native"]);
        assert_eq!(texts(&named("a", "A 1", false, false)), ["content", "off"]);
        let mut e = named("a", "A 1", true, true);
        e.dev = true;
        assert_eq!(texts(&e), ["native", "dev"]);
        e.problem = Some("Doesn't run any more".into());
        assert_eq!(texts(&e), ["native", "dev", "stopped"]);
        e.problem = Some("Installed, not built into this game: build it".into());
        assert_eq!(texts(&e), ["native", "dev", "not_built"]);
        // A Java mod is named after its runtime.
        let mut j = named("sodium", "Sodium 0.5", true, false);
        j.runtime = Some("horizon.mods.runtime.fabric_1_20_1".into());
        assert_eq!(texts(&j), ["horizon.mods.runtime.fabric_1_20_1"]);
    }

    #[test]
    fn icons_keep_their_colour() {
        assert_eq!(icon_rgb("zoom"), icon_rgb("zoom"));
        assert_ne!(icon_rgb("zoom"), icon_rgb("towns"));
        assert_eq!(shade([100, 200, 250], 0.5), [50, 100, 125]);
        assert_eq!(shade([100, 200, 250], 2.0), [200, 255, 255]);
        for id in ["", "a", "mod_menu", "horizon_plus", "rightharvest"] {
            let [r, g, b] = icon_rgb(id);
            assert!(
                u32::from(r) + u32::from(g) + u32::from(b) > 150,
                "{id} is too dark"
            );
        }
    }

    #[test]
    fn the_detail_pane_tells_the_story() {
        let e = named("zoom", "Zoom 1.0", true, true);
        assert_eq!(info_pair(&e)[1], "horizon.mod_menu.kind.native - zoom");
        let lines = body_lines(&e, 300.0, &six);
        let texts: Vec<&str> = lines.iter().map(|l| l.text.as_str()).collect();
        assert_eq!(texts, ["Zoom 1.0 does things."], "no status, no keys");
        let mut e = e;
        e.on = false;
        e.problem = Some("Doesn't run any more: needs a native port".into());
        e.description.clear();
        let lines = body_lines(&e, 600.0, &six);
        assert_eq!(lines[0].text, "horizon.mod_menu.no_description");
        assert_eq!(lines[0].tone, Tone::Dim);
        assert!(
            lines
                .iter()
                .any(|l| l.tone == Tone::Bad && l.text.starts_with("horizon.mod_menu.problem"))
        );
        assert_eq!(
            info_pair(&named("", "Broken.hmod", true, false))[1],
            "horizon.mod_menu.kind.content - horizon.mod_menu.no_id"
        );
    }

    #[test]
    fn mod_menu_itself_has_no_buttons_and_a_credit() {
        let me = named(ID, &format!("{NAME} {VERSION}"), true, true);
        assert!(is_self(&me) && !is_self(&named("zoom", "Zoom 1.0", true, true)));
        let l = layout(427.0, 240.0);
        let with = detail(&l, 0.0, true);
        let without = detail(&l, 0.0, false);
        assert!(
            without.body[1] < with.body[1],
            "the text moves up into the room the buttons had"
        );
        assert!(
            without.body[1] >= without.info[1][1] + LINE,
            "but not over the credit line"
        );
        assert!(detail_height(3, false) < detail_height(3, true));
    }

    #[test]
    fn the_icon_is_the_blue_list() {
        assert_eq!(MENU_ICON.len(), 16);
        assert!(
            MENU_ICON
                .iter()
                .all(|r| r.len() == 16 && r.bytes().all(|b| b == b'.' || b == b'#'))
        );
        let runs = menu_icon_runs();
        assert_eq!(runs.len(), 16, "{runs:?}");
        let white: usize = runs.iter().map(|r| usize::from(r.2)).sum();
        assert_eq!(
            white,
            MENU_ICON
                .iter()
                .map(|r| r.bytes().filter(|b| *b == b'#').count())
                .sum::<usize>()
        );
        for (col, row, len) in runs {
            assert!(usize::from(col) + usize::from(len) <= 16 && row < 16);
        }
    }

    #[test]
    fn the_detail_pane_scrolls_with_its_contents() {
        let l = layout(427.0, 240.0);
        let top = detail(&l, 0.0, true);
        let scrolled = detail(&l, 12.0, true);
        assert_eq!(top.icon.y - scrolled.icon.y, 12.0);
        assert_eq!(top.toggle.y - scrolled.toggle.y, 12.0);
        assert_eq!(top.body[1] - scrolled.body[1], 12.0);
        assert!(top.toggle.x + top.toggle.w < top.settings.x);
        assert!(top.settings.x + top.settings.w <= l.detail.x + l.detail.w);
        assert!(top.toggle.y + top.toggle.h < top.body[1]);
        assert!(top.width <= l.detail.w);
        assert_eq!(detail_height(0, true), 76.0);
        assert_eq!(detail_height(3, true), 106.0);
    }
}
