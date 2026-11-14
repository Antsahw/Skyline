// Just Enough Items for Horizon: JEI's config screen. JEI 26.3 on Fabric opens
// MezzConfig GUI's `ConfigScreen` (https://github.com/mezz/MezzConfigGui,
// MIT) from Mod Menu's Configure button and from its own config button, with
// the categories `JeiConfigGuiPlugin` lays out. This is that screen, with the
// settings this port has: the window with JEI's mod tab, the title, the
// category tree on the left, the search field with the Apply and Undo
// buttons, the value rows (check boxes, drop-down lists, reset buttons), the
// section headers and the info panel at the foot. Values apply as they
// change, as JEI's do (`ConfigValueEditMode.IMMEDIATE`); Undo puts back what
// the screen changed. The window and the category column can be resized as
// in MezzConfig GUI. Numbers and colours are MezzConfig GUI's
// (`ConfigScreenLayout`, `ConfigEntryWidget`, `ConfigGuiColors`...).
use super::Game;
use super::justenoughitems::{self as jei, Mode, Settings};
use super::justenoughitems_config_sprites::{self as cs, Sprite};
use super::justenoughitems_glue as glue;
use horizon_client::hud::{HudBatch, rgba};
use horizon_client::ui::EditBox;
use std::cell::RefCell;
use std::time::Instant;

type R = [i32; 4];

// ---- MezzConfig GUI's numbers ----------------------------------------------------------

const TITLE_HEIGHT: i32 = 18;
const SEARCH_HEIGHT: i32 = 18;
const INFO_AREA_HEIGHT: i32 = 57;
const NAV_DIVIDER_WIDTH: i32 = 8;
const MIN_CONTENT_WIDTH: i32 = 160;
const MIN_NAVIGATION_WIDTH: i32 = 80;
const DEFAULT_NAVIGATION_WIDTH: i32 = 110;
const SCROLLBAR_WIDTH: i32 = 12;
const SCROLLBAR_GAP: i32 = 2;
const BORDER_PADDING: i32 = 6;
const SECTION_GAP: i32 = BORDER_PADDING / 2;
const ACTION_BUTTON_SIZE: i32 = 18;
const ACTION_BUTTON_GAP: i32 = 3;
const MIN_WINDOW_WIDTH: i32 = 320;
const MIN_WINDOW_HEIGHT: i32 = 230;
const DEFAULT_WINDOW_WIDTH: i32 = 380;
const DEFAULT_WINDOW_HEIGHT: i32 = 300;
const MAX_WINDOW_SIZE: i32 = 8192;
const RESIZE_HANDLE_SIZE: i32 = 5;
const NAV_ITEM_HEIGHT: i32 = 20;
const NAV_ITEM_GAP: i32 = 2;
const TAB_WIDTH: i32 = 36;
const TAB_HEIGHT: i32 = 32;
const TAB_GUI_OVERLAP: i32 = 3;
const TAB_SCREEN_MARGIN: i32 = 2;
const TAB_VERTICAL_MARGIN: i32 = 4;
const ICON_SIZE: i32 = 24;
/// `ConfigEntryWidget`: the comfortable row density's two name lines and padding.
const MIN_ROW_HEIGHT: i32 = 26;
const NAME_LEFT_PADDING: i32 = 5;
const PREFERRED_VALUE_CONTROL_WIDTH: i32 = 110;
const MIN_VALUE_NAME_WIDTH: i32 = 80;
const VALUE_CONTROL_GAP: i32 = 4;
const RESET_BUTTON_SIZE: i32 = 18;
const VALUE_CONTROL_RIGHT_RESERVE: i32 = RESET_BUTTON_SIZE + 2 + 2;
const SECTION_INDENT: i32 = 12;
const MIN_SECTION_WIDTH: i32 = 80;
const SCROLL_SPEED: f64 = 10.0;
const SCROLL_LERP: f64 = 0.35;
const LINE: i32 = 9;
const TOOLTIP_MAX_WIDTH: f32 = 280.0;

// `ConfigGuiColors` (0xAARRGGBB).
const SEARCH_TEXT: u32 = 0xFFE8_EEF7;
const SEARCH_HINT: u32 = 0xFF8F_98A6;
const INFO_BACKGROUND: u32 = 0xE010_1218;
const INFO_BORDER: u32 = 0x70FF_FFFF;
const INFO_TITLE_TEXT: u32 = 0xFFF3_F6FF;
const INFO_TEXT: u32 = 0xFFC9_D3E2;
const TITLE_TEXT: u32 = 0xFF40_4040;
const RESIZE_GRIP: u32 = 0x45FF_FFFF;
const RESIZE_HANDLE_HOVER: u32 = 0x80FF_FFFF;
const NAVIGATION_BACKGROUND: u32 = 0x1800_0000;
const VALUE_AREA_BACKGROUND: u32 = 0x8200_0000;
const INSET_BORDER_DARK: u32 = 0xB000_0000;
const INSET_BORDER_LIGHT: u32 = 0x35FF_FFFF;
const TOOLTIP_TITLE_TEXT: u32 = 0xFFFF_FFFF;
const TOOLTIP_TEXT: u32 = 0xFFAA_AAAA;
const ENTRY_TEXT: u32 = 0xFFFF_FFFF;
const ROW_STRIPE_LIGHT: u32 = 0x08FF_FFFF;
const ROW_STRIPE_DARK: u32 = 0x0800_0000;
const ROW_HOVER: u32 = 0x18FF_FFFF;
const BUTTON_UNDERLAY: u32 = 0xFF11_1216;
const SECTION_HEADER_BACKGROUND: u32 = 0xFF30_3844;
const SECTION_HEADER_TEXT: u32 = 0xFFF3_F6FF;
const SECTION_HEADER_DIVIDER: u32 = 0xFF63_7084;
const NAV_ITEM_BACKGROUND: u32 = 0x3300_0000;
const NAV_ITEM_HOVER_BACKGROUND: u32 = 0x22FF_FFFF;
const NAV_ITEM_ACTIVE_BACKGROUND: u32 = 0x6631_3A46;
const NAV_ITEM_ACTIVE_ACCENT: u32 = 0xFF5E_9AD6;
const NAV_ITEM_DIVIDER: u32 = 0x2AFF_FFFF;
const NAV_ITEM_TEXT: u32 = 0xFF20_242A;
const NAV_ITEM_HOVER_TEXT: u32 = 0xFF11_1820;
const NAV_ITEM_ACTIVE_TEXT: u32 = 0xFF10_1A24;
const SELECTOR_BACKGROUND: u32 = 0xF010_1218;
const SELECTOR_ROW_BACKGROUND: u32 = 0xAA1A_1D24;
const SELECTOR_ROW_HOVER: u32 = 0xFF31_3A46;
const SELECTOR_BORDER_DARK: u32 = 0xE000_0000;
const SELECTOR_BORDER_LIGHT: u32 = 0x45FF_FFFF;
const SELECTOR_DIVIDER: u32 = 0x22FF_FFFF;
const DISABLED_ICON_TINT: u32 = 0xFFA0_A0A0;

// ---- texts ------------------------------------------------------------------------------

/// JEI's texts (`assets/jei/lang`, en_us and ru_ru) and MezzConfig GUI's
/// (`assets/mezz_config_gui/lang`: en_us only, so Russian games show them in
/// English, as Java does). An empty Russian text is the English one.
const TEXTS: &[(&str, &str, &str)] = &[
    ("jei.config", "JEI Config", "Конфигурация JEI"),
    ("jei.config.client.lists", "Lists", "Списки"),
    ("jei.config.client.lists.description", "Options shared by the ingredient and bookmark lists.", "Общие настройки списков ингредиентов и закладок."),
    ("jei.config.client.ingredientList", "Ingredient List", "Список ингредиентов"),
    ("jei.config.client.ingredientList.description", "Options for the ingredient list shown on the right side of the screen.", "Настройки списка ингредиентов в правой части экрана."),
    ("jei.config.client.bookmarkList", "Bookmark List", "Список закладок"),
    ("jei.config.client.bookmarkList.description", "Options for saved bookmarks and the bookmark list shown on the left side of the screen.", "Настройки сохранённых закладок и их списка в левой части экрана."),
    ("jei.config.client.bookmarks", "Bookmarks", "Закладки"),
    ("jei.config.client.bookmarks.description", "Options for saving ingredients to the bookmark list.", "Настройки сохранения ингредиентов в закладки."),
    ("jei.config.client.search", "Search", "Параметры поиска"),
    ("jei.config.client.search.description", "Options for what JEI checks when filtering the ingredient list.", "Параметры, связанные с полем поиска."),
    ("jei.config.client.search.matching", "Matching", "Условия поиска"),
    ("jei.config.client.search.matching.description", "Choose which ingredient details and alternate names are searched.", "Выберите, какие сведения и дополнительные названия ингредиентов учитывать при поиске."),
    ("jei.config.client.cheating", "Cheats", "Читы"),
    ("jei.config.client.cheating.description", "Options for giving ingredients from JEI in cheat mode.", "Настройки выдачи ингредиентов через чит-режим JEI."),
    ("jei.config.client.ingredientList.overlaysEnabled", "Show JEI Overlays", "Показывать панели JEI"),
    ("jei.config.client.ingredientList.overlaysEnabled.description", "Show or hide the ingredient and bookmark overlays. This preference is saved across game launches.", "Показывать или скрывать панели ингредиентов и закладок. Настройка сохраняется между запусками игры."),
    ("jei.config.client.ingredientList.overlaysEnabled.value.true.description", "Show JEI's ingredient and bookmark overlays.", "Показывать панели ингредиентов и закладок JEI."),
    ("jei.config.client.ingredientList.overlaysEnabled.value.false.description", "Hide JEI's ingredient and bookmark overlays.", "Скрывать панели ингредиентов и закладок JEI."),
    ("jei.config.client.bookmarkList.bookmarkOverlayEnabled", "Show Bookmark Overlay", "Показывать панель закладок"),
    ("jei.config.client.bookmarkList.bookmarkOverlayEnabled.description", "Show or hide the bookmark overlay for the current session when JEI overlays are enabled. This value is not saved to the configuration file.", "Показывать или скрывать панель закладок в текущем сеансе, когда панели JEI включены. Значение не сохраняется в файле настроек."),
    ("jei.config.client.bookmarkList.bookmarkOverlayEnabled.value.true.description", "Show bookmarked ingredients when JEI overlays are enabled.", "Показывать ингредиенты в закладках, когда панели JEI включены."),
    ("jei.config.client.bookmarkList.bookmarkOverlayEnabled.value.false.description", "Hide the bookmark overlay.", "Скрывать панель закладок."),
    ("jei.config.client.bookmarkList.addBookmarksToFrontEnabled", "New Bookmark Placement", "Расположение новых закладок"),
    ("jei.config.client.bookmarkList.addBookmarksToFrontEnabled.description", "Choose where newly added bookmarks appear in the bookmark list.", "Выберите, где в списке появляются новые закладки."),
    ("jei.config.client.bookmarkList.addBookmarksToFrontEnabled.end", "End of List", "Конец списка"),
    ("jei.config.client.bookmarkList.addBookmarksToFrontEnabled.end.description", "Add new bookmarks to the end of the bookmark list.", "Добавлять новые закладки в конец списка."),
    ("jei.config.client.bookmarkList.addBookmarksToFrontEnabled.front", "Front of List", "Начало списка"),
    ("jei.config.client.bookmarkList.addBookmarksToFrontEnabled.front.description", "Add new bookmarks to the front of the bookmark list.", "Добавлять новые закладки в начало списка."),
    ("jei.config.client.search.centerSearch", "JEI Search Bar Position", "Положение строки поиска JEI"),
    ("jei.config.client.search.centerSearch.description", "Choose where the JEI search bar appears.", "Выберите расположение строки поиска JEI."),
    ("jei.config.client.search.centerSearch.standard", "Bottom Right", "Внизу справа"),
    ("jei.config.client.search.centerSearch.standard.description", "Place the search bar in the lower-right corner.", "Разместить строку поиска в правом нижнем углу."),
    ("jei.config.client.search.centerSearch.centered", "Bottom Center", "Внизу по центру"),
    ("jei.config.client.search.centerSearch.centered.description", "Center the search bar at the bottom of the screen.", "Разместить строку поиска внизу по центру экрана."),
    ("jei.config.client.search.modNameSearchMode", "Search Mod Names", "@Название мода"),
    ("jei.config.client.search.modNameSearchMode.description", "Search by the name of the mod that adds each ingredient. Prefix: @", "Режим поиска по названиям мода (префикс: @)."),
    ("jei.config.client.search.modNameSearchMode.enabled.description", "Match mod names from normal search text.", "Искать по названиям модов в обычных запросах."),
    ("jei.config.client.search.modNameSearchMode.require_prefix.description", "Match mod names only when @ is typed first, like @minecraft.", "Искать по названиям модов только с префиксом @, например @minecraft."),
    ("jei.config.client.search.modNameSearchMode.disabled.description", "Do not search mod names.", "Не искать по названиям модов."),
    ("jei.config.client.search.tagSearchMode", "Search Ingredient Tags", "#Тег"),
    ("jei.config.client.search.tagSearchMode.description", "Search tags that group related ingredients, such as logs or ingots. Prefix: #", "Режим поиска по тегам (префикс: #)."),
    ("jei.config.client.search.tagSearchMode.enabled.description", "Match ingredient tags from normal search text.", "Искать по тегам ингредиентов в обычных запросах."),
    ("jei.config.client.search.tagSearchMode.require_prefix.description", "Match ingredient tags only when # is typed first, like #forge.", "Искать по тегам ингредиентов только с префиксом #, например #forge."),
    ("jei.config.client.search.tagSearchMode.disabled.description", "Do not search ingredient tags.", "Не искать по тегам ингредиентов."),
    ("jei.config.client.search.tooltipSearchMode", "Search Tooltip Text", "$Подсказка"),
    ("jei.config.client.search.tooltipSearchMode.description", "Search words shown in ingredient tooltips. Prefix: $", "Режим поиска по подсказкам (префикс: $)."),
    ("jei.config.client.search.tooltipSearchMode.enabled.description", "Match tooltip text from normal search text.", "Искать по тексту подсказок в обычных запросах."),
    ("jei.config.client.search.tooltipSearchMode.require_prefix.description", "Match tooltip text only when $ is typed first, like $damage.", "Искать по тексту подсказок только с префиксом $, например $damage."),
    ("jei.config.client.search.tooltipSearchMode.disabled.description", "Do not search ingredient tooltips.", "Не искать по подсказкам ингредиентов."),
    ("jei.config.client.search.identifierSearchMode", "Search Ingredient IDs", "&Идентификатор ресурса"),
    ("jei.config.client.search.identifierSearchMode.description", "Search ingredient IDs like minecraft:stone. Prefix: &", "Режим поиска по идентификаторам ресурсов (префикс: &)."),
    ("jei.config.client.search.identifierSearchMode.enabled.description", "Match ingredient IDs from normal search text.", "Искать по идентификаторам ингредиентов в обычных запросах."),
    ("jei.config.client.search.identifierSearchMode.require_prefix.description", "Match ingredient IDs only when & is typed first, like &minecraft:stone.", "Искать по идентификаторам ингредиентов только с префиксом &, например &minecraft:stone."),
    ("jei.config.client.search.identifierSearchMode.disabled.description", "Do not search ingredient IDs.", "Не искать по идентификаторам ингредиентов."),
    ("jei.config.client.search.creativeTabSearchMode", "Search Creative Inventory Tabs", "%Творческая вкладка"),
    ("jei.config.client.search.creativeTabSearchMode.description", "Search by the creative inventory tab that contains an ingredient. Prefix: %", "Режим поиска по названиям творческих вкладок (префикс: %)."),
    ("jei.config.client.search.creativeTabSearchMode.enabled.description", "Match creative inventory tab names from normal search text.", "Искать по названиям вкладок творческого инвентаря в обычных запросах."),
    ("jei.config.client.search.creativeTabSearchMode.require_prefix.description", "Match creative inventory tab names only when % is typed first, like %building.", "Искать по названиям вкладок творческого инвентаря только с префиксом %, например %building."),
    ("jei.config.client.search.creativeTabSearchMode.disabled.description", "Do not search creative inventory tab names.", "Не искать по названиям вкладок творческого инвентаря."),
    ("jei.config.client.cheating.cheatModeEnabled", "Enable Cheat Mode", "Включить чит-режим"),
    ("jei.config.client.cheating.cheatModeEnabled.description", "Turn cheat mode on or off for the current session. This value is not saved to the configuration file. The server must allow JEI cheats.", "Включить или выключить чит-режим в текущем сеансе. Значение не сохраняется в файле настроек. Для использования читов JEI нужно разрешение сервера."),
    ("jei.config.client.cheating.cheatModeEnabled.value.true.description", "Enable cheat mode, subject to server permission.", "Включить чит-режим с разрешения сервера."),
    ("jei.config.client.cheating.cheatModeEnabled.value.false.description", "Disable cheat mode.", "Выключить чит-режим."),
    // MezzConfig GUI.
    ("mezz_config_gui.config.screen.search", "find configs by name", ""),
    ("mezz_config_gui.config.screen.sectionSettings", "Settings", ""),
    ("mezz_config_gui.config.screen.search.info.title", "Search", ""),
    ("mezz_config_gui.config.screen.search.info", "Filter config options by name.", ""),
    ("mezz_config_gui.config.screen.reset", "Reset", ""),
    ("mezz_config_gui.config.screen.reset.value.info", "Reset this config option to its default value.", ""),
    ("mezz_config_gui.config.screen.undo.title", "Undo Changes", ""),
    ("mezz_config_gui.config.screen.undo.info", "Discard pending changes and revert changes already applied from this screen.", ""),
    ("mezz_config_gui.config.screen.undo.disabled.info", "There are no changes to undo.", ""),
    ("mezz_config_gui.config.screen.applyPending.title", "Apply Pending Changes", ""),
    ("mezz_config_gui.config.screen.applyPending.info", "Apply pending changes without closing the config screen.", ""),
    ("mezz_config_gui.config.screen.applyPending.disabled.info", "There are no pending changes to apply.", ""),
    ("mezz_config_gui.config.screen.resize.title", "Resize", ""),
    ("mezz_config_gui.config.screen.resize.info", "Drag a screen edge or corner to resize it. The window size is saved when released.", ""),
    ("mezz_config_gui.config.screen.navigationResize.title", "Resize Navigation", ""),
    ("mezz_config_gui.config.screen.navigationResize.info", "Drag this divider left or right to change the width of the navigation column. The width is saved when released.", ""),
    ("mezz_config_gui.config.value.boolean.action.enable", "Click to enable this setting.", ""),
    ("mezz_config_gui.config.value.boolean.action.disable", "Click to disable this setting.", ""),
];

fn lookup(ru: bool, key: &str) -> Option<&'static str> {
    TEXTS.iter().find(|t| t.0 == key).map(|&(_, en, r)| if ru && !r.is_empty() { r } else { en })
}

/// A text by its key; the key itself when there is none (`Component.translatable`).
fn tr(ru: bool, key: &str) -> String {
    lookup(ru, key).map_or_else(|| key.to_owned(), str::to_owned)
}

// ---- the values --------------------------------------------------------------------------

/// A value row: the JEI config value it is, with the port's setting behind it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Opt {
    /// `ingredientList.overlaysEnabled`.
    Overlay,
    /// The session toggle `bookmarkOverlayEnabled`.
    BookmarkOverlay,
    /// `bookmarkList.addBookmarksToFrontEnabled` (`BookmarkAddPosition`).
    AddPosition,
    /// `search.centerSearch` (`SearchBarPosition`).
    CenterSearch,
    /// One of the search modes, by `Settings::MODE_KEYS`.
    Mode(usize),
    /// The session toggle `cheatModeEnabled`.
    Cheat,
}

const MODE_VALUES: [&str; 5] =
    ["modNameSearchMode", "tagSearchMode", "tooltipSearchMode", "identifierSearchMode", "creativeTabSearchMode"];

impl Opt {
    fn key(self) -> String {
        match self {
            Opt::Overlay => "jei.config.client.ingredientList.overlaysEnabled".into(),
            Opt::BookmarkOverlay => "jei.config.client.bookmarkList.bookmarkOverlayEnabled".into(),
            Opt::AddPosition => "jei.config.client.bookmarkList.addBookmarksToFrontEnabled".into(),
            Opt::CenterSearch => "jei.config.client.search.centerSearch".into(),
            Opt::Mode(i) => format!("jei.config.client.search.{}", MODE_VALUES[i]),
            Opt::Cheat => "jei.config.client.cheating.cheatModeEnabled".into(),
        }
    }
    /// The enum's values (lower case, as their keys spell them); none for a
    /// switch.
    fn choices(self) -> &'static [&'static str] {
        match self {
            Opt::AddPosition => &["end", "front"],
            Opt::CenterSearch => &["standard", "centered"],
            Opt::Mode(_) => &["enabled", "require_prefix", "disabled"],
            _ => &[],
        }
    }
    fn is_bool(self) -> bool {
        self.choices().is_empty()
    }
    fn get(self, s: &Settings) -> usize {
        let index = |m: Mode| Mode::ALL.iter().position(|&x| x == m).unwrap_or(0);
        match self {
            Opt::Overlay => usize::from(s.overlay),
            Opt::BookmarkOverlay => usize::from(s.bookmarks),
            Opt::AddPosition => usize::from(s.bookmarks_front),
            Opt::CenterSearch => usize::from(s.center_search),
            Opt::Mode(i) => index(s.mode(Settings::MODE_KEYS[i])),
            Opt::Cheat => usize::from(s.cheat),
        }
    }
    fn set(self, s: &mut Settings, v: usize) {
        match self {
            Opt::Overlay => s.overlay = v != 0,
            Opt::BookmarkOverlay => s.bookmarks = v != 0,
            Opt::AddPosition => s.bookmarks_front = v != 0,
            Opt::CenterSearch => s.center_search = v != 0,
            Opt::Mode(i) => s.set_mode(Settings::MODE_KEYS[i], Mode::ALL[v.min(2)]),
            Opt::Cheat => s.cheat = v != 0,
        }
    }
    fn default(self) -> usize {
        self.get(&Settings::default())
    }
    fn name(self, ru: bool) -> String {
        tr(ru, &self.key())
    }
    fn description(self, ru: bool) -> String {
        tr(ru, &format!("{}.description", self.key()))
    }
    /// A value's name (`ConfigValueLocalization.getValueName`): its key, else
    /// the enum name in words.
    fn value_name(self, ru: bool, v: usize) -> String {
        let Some(&id) = self.choices().get(v) else {
            return if v != 0 { "Enabled".into() } else { "Disabled".into() };
        };
        lookup(ru, &format!("{}.{id}", self.key())).map_or_else(
            || {
                id.split('_')
                    .map(|w| {
                        let mut c = w.chars();
                        c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            },
            str::to_owned,
        )
    }
    /// A value's description, when the language has one.
    fn value_description(self, ru: bool, v: usize) -> Option<String> {
        match self.choices().get(v) {
            Some(id) => lookup(ru, &format!("{}.{id}.description", self.key())).map(str::to_owned),
            None => lookup(ru, &format!("{}.value.{}.description", self.key(), v != 0)).map(str::to_owned),
        }
    }
}

/// A category of the tree (`JeiConfigGuiPlugin`'s, as `ConfigCategoryTree`
/// lays it out: the top categories and their children in the navigation).
struct Cat {
    title: &'static str,
    description: &'static str,
    depth: i32,
    parent: Option<usize>,
    opts: &'static [Opt],
}

const CATS: [Cat; 8] = [
    Cat { title: "jei.config.client.lists", description: "jei.config.client.lists.description", depth: 0, parent: None, opts: &[] },
    Cat { title: "jei.config.client.ingredientList", description: "jei.config.client.ingredientList.description", depth: 1, parent: Some(0), opts: &[Opt::Overlay] },
    Cat { title: "jei.config.client.bookmarkList", description: "jei.config.client.bookmarkList.description", depth: 1, parent: Some(0), opts: &[Opt::BookmarkOverlay] },
    Cat { title: "jei.config.client.bookmarks", description: "jei.config.client.bookmarks.description", depth: 1, parent: Some(0), opts: &[Opt::AddPosition] },
    Cat { title: "jei.config.client.search", description: "jei.config.client.search.description", depth: 0, parent: None, opts: &[] },
    // The search category's own values, beside its children: "Settings".
    Cat { title: "mezz_config_gui.config.screen.sectionSettings", description: "jei.config.client.search.description", depth: 1, parent: Some(4), opts: &[Opt::CenterSearch] },
    Cat {
        title: "jei.config.client.search.matching",
        description: "jei.config.client.search.matching.description",
        depth: 1,
        parent: Some(4),
        opts: &[Opt::Mode(0), Opt::Mode(1), Opt::Mode(2), Opt::Mode(3), Opt::Mode(4)],
    },
    Cat { title: "jei.config.client.cheating", description: "jei.config.client.cheating.description", depth: 0, parent: None, opts: &[Opt::Cheat] },
];

/// Index of the "Settings" category, which is the search category's own values.
const SETTINGS_CAT: usize = 5;

fn children(c: usize) -> impl Iterator<Item = usize> {
    (0..CATS.len()).filter(move |&i| CATS[i].parent == Some(c))
}

fn cat_of(opt: Opt) -> usize {
    CATS.iter().position(|c| c.opts.contains(&opt)).unwrap_or(0)
}

/// The name searching shows (`ConfigValueCategoryPath.getContextualName`):
/// "Category › Name" for a value of a child category.
fn contextual_name(ru: bool, opt: Opt) -> String {
    let c = cat_of(opt);
    if CATS[c].depth > 0 && c != SETTINGS_CAT {
        format!("{} › {}", tr(ru, CATS[c].title), opt.name(ru))
    } else {
        opt.name(ru)
    }
}

// ---- rectangles ---------------------------------------------------------------------------

fn contains(r: R, p: (f32, f32)) -> bool {
    p.0 >= r[0] as f32 && p.0 < (r[0] + r[2]) as f32 && p.1 >= r[1] as f32 && p.1 < (r[1] + r[3]) as f32
}
fn inset(r: R, n: i32) -> R {
    [r[0] + n, r[1] + n, (r[2] - 2 * n).max(0), (r[3] - 2 * n).max(0)]
}
fn keep_top(r: R, h: i32) -> R {
    [r[0], r[1], r[2], h.min(r[3]).max(0)]
}
fn keep_bottom(r: R, h: i32) -> R {
    let h = h.min(r[3]).max(0);
    [r[0], r[1] + r[3] - h, r[2], h]
}
fn keep_left(r: R, w: i32) -> R {
    [r[0], r[1], w.min(r[2]).max(0), r[3]]
}
fn keep_right(r: R, w: i32) -> R {
    let w = w.min(r[2]).max(0);
    [r[0] + r[2] - w, r[1], w, r[3]]
}
fn crop_top(r: R, n: i32) -> R {
    let n = n.min(r[3]).max(0);
    [r[0], r[1] + n, r[2], r[3] - n]
}
fn crop_bottom(r: R, n: i32) -> R {
    [r[0], r[1], r[2], (r[3] - n).max(0)]
}
fn crop_left(r: R, n: i32) -> R {
    let n = n.min(r[2]).max(0);
    [r[0] + n, r[1], r[2] - n, r[3]]
}
fn crop_right(r: R, n: i32) -> R {
    [r[0], r[1], (r[2] - n).max(0), r[3]]
}
fn center_v(r: R, h: i32) -> R {
    let h = h.min(r[3]);
    [r[0], r[1] + (r[3] - h) / 2, r[2], h]
}
fn intersects(a: R, b: R) -> bool {
    a[0] < b[0] + b[2] && b[0] < a[0] + a[2] && a[1] < b[1] + b[3] && b[1] < a[1] + a[3]
}
fn is_empty(r: R) -> bool {
    r[2] <= 0 || r[3] <= 0
}

// ---- drawing --------------------------------------------------------------------------------

fn argb(c: u32) -> u32 {
    let [a, r, g, b] = c.to_be_bytes();
    rgba(r, g, b, a)
}

fn fill(game: &Game<'_>, batch: &mut HudBatch, r: R, color: u32) {
    if r[2] > 0 && r[3] > 0 {
        batch.fill(&game.gui.atlas, r[0] as f32, r[1] as f32, r[2] as f32, r[3] as f32, argb(color));
    }
}

/// A sprite pixel tinted by an 0xAARRGGBB colour (`drawTinted`).
fn tint(c: u32, t: u32) -> u32 {
    if t == u32::MAX {
        return c;
    }
    let [ta, tr, tg, tb] = t.to_be_bytes();
    let m = |v: u32, k: u8| ((v & 0xff) * u32::from(k) / 255) & 0xff;
    let (r, g, b, a) = (c & 0xff, (c >> 8) & 0xff, (c >> 16) & 0xff, c >> 24);
    (m(a, ta) << 24) | (m(b, tb) << 16) | (m(g, tg) << 8) | m(r, tr)
}

/// Columns `x0..x0+w` and rows `y0..y0+h` of a sprite at `(x, y)`.
#[allow(clippy::too_many_arguments)]
fn blit_part(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, x0: u16, y0: u16, w: u16, h: u16, x: i32, y: i32, t: u32) {
    let atlas = &game.gui.atlas;
    let sw = usize::from(s.width);
    for row in y0..y0 + h {
        let mut col = x0;
        while col < x0 + w {
            let c = s.pixels[usize::from(row) * sw + usize::from(col)];
            let mut len = 1;
            while col + len < x0 + w && s.pixels[usize::from(row) * sw + usize::from(col + len)] == c {
                len += 1;
            }
            if c >> 24 != 0 {
                batch.fill(
                    atlas,
                    (x + i32::from(col - x0)) as f32,
                    (y + i32::from(row - y0)) as f32,
                    f32::from(len),
                    1.0,
                    tint(c, t),
                );
            }
            col += len;
        }
    }
}

fn blit(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, x: i32, y: i32) {
    blit_part(game, batch, s, 0, 0, s.width, s.height, x, y, u32::MAX);
}

fn blit_tinted(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, x: i32, y: i32, t: u32) {
    blit_part(game, batch, s, 0, 0, s.width, s.height, x, y, t);
}

/// The one colour of a part of a sprite, when it has one.
fn uniform(s: &Sprite, x0: u16, y0: u16, w: u16, h: u16) -> Option<u32> {
    let sw = usize::from(s.width);
    let first = s.pixels[usize::from(y0) * sw + usize::from(x0)];
    for row in y0..y0 + h {
        for col in x0..x0 + w {
            if s.pixels[usize::from(row) * sw + usize::from(col)] != first {
                return None;
            }
        }
    }
    Some(first)
}

/// A part of a sprite repeated over `dest` (the game's nine-slice tiling).
#[allow(clippy::too_many_arguments)]
fn tile(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, x0: u16, y0: u16, w: u16, h: u16, dest: R) {
    if w == 0 || h == 0 || is_empty(dest) {
        return;
    }
    if let Some(c) = uniform(s, x0, y0, w, h) {
        if c >> 24 != 0 {
            batch.fill(&game.gui.atlas, dest[0] as f32, dest[1] as f32, dest[2] as f32, dest[3] as f32, c);
        }
        return;
    }
    let mut ty = 0;
    while ty < dest[3] {
        let th = (dest[3] - ty).min(i32::from(h)) as u16;
        let mut tx = 0;
        while tx < dest[2] {
            let tw = (dest[2] - tx).min(i32::from(w)) as u16;
            blit_part(game, batch, s, x0, y0, tw, th, dest[0] + tx, dest[1] + ty, u32::MAX);
            tx += i32::from(w);
        }
        ty += i32::from(h);
    }
}

/// A nine-slice sprite over `r`: corners as they are, edges and middle tiled.
fn nine(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, r: R) {
    if is_empty(r) {
        return;
    }
    let Some([l, t, rt, b]) = s.border else {
        blit(game, batch, s, r[0], r[1]);
        return;
    };
    let (w, h) = (s.width, s.height);
    let (l32, t32, r32, b32) = (i32::from(l), i32::from(t), i32::from(rt), i32::from(b));
    // A smaller area takes the corners' near halves.
    let lw = l32.min(r[2] / 2);
    let rw = r32.min(r[2] - lw);
    let th = t32.min(r[3] / 2);
    let bh = b32.min(r[3] - th);
    let (x, y, x2, y2) = (r[0], r[1], r[0] + r[2], r[1] + r[3]);
    let (mw, mh) = (r[2] - lw - rw, r[3] - th - bh);
    let (lwu, rwu, thu, bhu) = (lw as u16, rw as u16, th as u16, bh as u16);
    blit_part(game, batch, s, 0, 0, lwu, thu, x, y, u32::MAX);
    blit_part(game, batch, s, w - rwu, 0, rwu, thu, x2 - rw, y, u32::MAX);
    blit_part(game, batch, s, 0, h - bhu, lwu, bhu, x, y2 - bh, u32::MAX);
    blit_part(game, batch, s, w - rwu, h - bhu, rwu, bhu, x2 - rw, y2 - bh, u32::MAX);
    let (cw, ch) = (w - l - rt, h - t - b);
    tile(game, batch, s, l, 0, cw, thu, [x + lw, y, mw, th]);
    tile(game, batch, s, l, h - bhu, cw, bhu, [x + lw, y2 - bh, mw, bh]);
    tile(game, batch, s, 0, t, lwu, ch, [x, y + th, lw, mh]);
    tile(game, batch, s, w - rwu, t, rwu, ch, [x2 - rw, y + th, rw, mh]);
    tile(game, batch, s, l, t, cw, ch, [x + lw, y + th, mw, mh]);
}

fn text(game: &Game<'_>, batch: &mut HudBatch, s: &str, x: i32, y: i32, color: u32) {
    batch.text(&game.gui.atlas, s, x as f32, y as f32, argb(color), false);
}

fn width(game: &Game<'_>, s: &str) -> i32 {
    HudBatch::text_width(&game.gui.atlas, s).ceil() as i32
}

/// `Font.split`: the text broken at spaces into lines of at most `max`
/// pixels; a word longer than a line is broken where it reaches the edge.
fn wrap(game: &Game<'_>, s: &str, max: i32) -> Vec<String> {
    let mut lines = Vec::new();
    for para in s.split('\n') {
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if width(game, &candidate) <= max || line.is_empty() && word.is_empty() {
                line = candidate;
                continue;
            }
            if !line.is_empty() {
                lines.push(std::mem::take(&mut line));
            }
            // The word alone, cut where it reaches the edge.
            for c in word.chars() {
                let next = format!("{line}{c}");
                if width(game, &next) > max && !line.is_empty() {
                    lines.push(std::mem::take(&mut line));
                    line.push(c);
                } else {
                    line = next;
                }
            }
        }
        lines.push(line);
    }
    lines
}

/// `StringUtil.truncateStringToWidth`: cut to fit `max` with "...".
fn truncate(game: &Game<'_>, s: &str, max: i32) -> String {
    let ellipsis = width(game, "...");
    let mut out = String::new();
    for c in s.chars() {
        let next = format!("{out}{c}");
        if width(game, &next) > max - ellipsis {
            break;
        }
        out = next;
    }
    format!("{out}...")
}

/// `StringUtil.splitLines`: lines broken at spaces (long words hyphenated),
/// at most `max_lines`; the last one cut with "..." when more would follow.
fn split_lines(game: &Game<'_>, s: &str, max: i32, max_lines: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for para in s.split('\n') {
        let mut lines = Vec::new();
        let mut line = String::new();
        for word in para.split(' ') {
            let candidate = if line.is_empty() { word.to_owned() } else { format!("{line} {word}") };
            if width(game, &candidate) <= max || line.is_empty() {
                line = candidate;
            } else {
                lines.push(std::mem::take(&mut line));
                line = word.to_owned();
            }
        }
        lines.push(line);
        for l in lines {
            // Too long for a line: hyphenated.
            let mut rest = l;
            while width(game, &rest) > max {
                let hyphen = width(game, "-");
                let mut head = String::new();
                for c in rest.chars() {
                    if width(game, &format!("{head}{c}")) > max - hyphen {
                        break;
                    }
                    head.push(c);
                }
                if head.is_empty() {
                    break;
                }
                let tail: String = rest.chars().skip(head.chars().count()).collect();
                if out.len() == max_lines {
                    let last = out.pop().unwrap_or_default();
                    out.push(truncate(game, &last, max));
                    return out;
                }
                out.push(format!("{head}-"));
                rest = tail;
            }
            if out.len() == max_lines {
                let last = out.pop().unwrap_or_default();
                out.push(truncate(game, &last, max));
                return out;
            }
            out.push(rest);
        }
    }
    out
}

/// `ConfigEntryWidget.drawFittedText`: as many lines as fit the area's
/// height, centred vertically (and across when `centered`).
fn fitted_text(game: &Game<'_>, batch: &mut HudBatch, s: &str, area: R, color: u32, centered: bool) {
    let max_lines = ((area[3] - 4 + 1) / LINE).max(1) as usize;
    let lines = split_lines(game, s, area[2], max_lines);
    let h = lines.len() as i32 * LINE - 1;
    let mut y = area[1] + ((area[3] - h) as f32 / 2.0).round() as i32;
    for line in &lines {
        let mut x = area[0];
        if centered {
            x += ((area[2] - width(game, line)) as f32 / 2.0).round() as i32;
        }
        text(game, batch, line, x, y, color);
        y += LINE;
    }
}

fn inset_border(game: &Game<'_>, batch: &mut HudBatch, r: R) {
    if is_empty(r) {
        return;
    }
    let (x, y, right, bottom) = (r[0], r[1], r[0] + r[2], r[1] + r[3]);
    fill(game, batch, [x, y, r[2], 1], INSET_BORDER_DARK);
    fill(game, batch, [x, y, 1, r[3]], INSET_BORDER_DARK);
    fill(game, batch, [x, bottom - 1, r[2], 1], INSET_BORDER_LIGHT);
    fill(game, batch, [right - 1, y, 1, r[3]], INSET_BORDER_LIGHT);
}

/// `ConfigEntryWidget.drawButtonBackground`.
fn button(game: &Game<'_>, batch: &mut HudBatch, r: R, active: bool, hovered: bool) {
    fill(game, batch, r, BUTTON_UNDERLAY);
    let s = match (active, hovered) {
        (false, _) => &cs::BUTTON_DISABLED,
        (true, false) => &cs::BUTTON_ENABLED,
        (true, true) => &cs::BUTTON_HIGHLIGHT,
    };
    nine(game, batch, s, r);
}

/// `ConfigButtonIcon.draw`: centred, grey when inactive.
fn icon(game: &Game<'_>, batch: &mut HudBatch, s: &Sprite, r: R, active: bool) {
    let x = r[0] + (r[2] - i32::from(s.width)) / 2;
    let y = r[1] + (r[3] - i32::from(s.height)) / 2;
    if active {
        blit(game, batch, s, x, y);
    } else {
        blit_tinted(game, batch, s, x, y, DISABLED_ICON_TINT);
    }
}

// ---- the screen ------------------------------------------------------------------------------

/// An info panel or tooltip text: a title and lines (`ConfigInfo`).
#[derive(Clone, PartialEq, Debug, Default)]
struct Info {
    title: String,
    lines: Vec<String>,
}

impl Info {
    fn new(title: String, line: String) -> Info {
        Info { title, lines: vec![line] }
    }
}

/// `ConfigScrollState`: smooth scrolling and scrollbar dragging.
#[derive(Default, Clone, Copy)]
struct Scroll {
    current: f64,
    target: f64,
    /// Dragging the marker, this far below its top.
    drag: Option<f64>,
    viewport: R,
    bar: R,
    content: i32,
}

impl Scroll {
    fn maximum(&self) -> f64 {
        f64::from((self.content - self.viewport[3]).max(0))
    }
    fn update(&mut self, viewport: R, content: i32, bar: R) {
        self.viewport = viewport;
        self.content = content;
        self.bar = bar;
        self.current = self.current.clamp(0.0, self.maximum());
        self.target = self.target.clamp(0.0, self.maximum());
    }
    fn reset(&mut self) {
        self.current = 0.0;
        self.target = 0.0;
        self.drag = None;
    }
    fn scroll(&mut self, amount: f64) {
        self.target = (self.target - amount * SCROLL_SPEED).clamp(0.0, self.maximum());
    }
    fn step(&mut self) {
        if (self.target - self.current).abs() > 0.5 {
            self.current += (self.target - self.current) * SCROLL_LERP;
        } else {
            self.current = self.target;
        }
    }
    fn track(&self) -> R {
        if self.maximum() <= 0.0 { [0; 4] } else { inset(self.bar, 1) }
    }
    fn marker(&self) -> R {
        let track = self.track();
        if is_empty(track) {
            return [0; 4];
        }
        let h = track[3].min((track[3] * self.viewport[3] / self.content.max(1)).max(10));
        let y = track[1] + (f64::from(track[3] - h) * self.current / self.maximum()) as i32;
        [track[0], y, track[2], h]
    }
    fn start_drag(&mut self, p: (f32, f32)) -> bool {
        let marker = self.marker();
        if is_empty(marker) || !contains(self.bar, p) {
            return false;
        }
        if contains(marker, p) {
            self.drag = Some(f64::from(p.1) - f64::from(marker[1]));
        } else {
            self.drag = Some(f64::from(marker[3]) / 2.0);
            self.drag_to(p.1);
        }
        true
    }
    fn drag_to(&mut self, y: f32) {
        let Some(offset) = self.drag else {
            return;
        };
        let track = self.track();
        let space = track[3] - self.marker()[3];
        self.current = 0.0;
        if space > 0 {
            self.current =
                ((f64::from(y) - offset - f64::from(track[1])) * self.maximum() / f64::from(space)).clamp(0.0, self.maximum());
        }
        self.target = self.current;
    }
}

/// A value row laid out.
struct Row {
    opt: Opt,
    area: R,
    name: Vec<String>,
    name_y: i32,
    value: R,
    reset: R,
}

/// A section header laid out (a child category's title above its rows).
struct Header {
    cat: usize,
    area: R,
    lines: Vec<String>,
    bottom: i32,
}

/// A navigation item laid out.
struct NavItem {
    cat: usize,
    area: R,
    hover: R,
    lines: Vec<String>,
}

#[derive(Default)]
struct Layout {
    area: R,
    title: R,
    tab: R,
    nav: R,
    divider: R,
    content: R,
    search: R,
    info: R,
    apply: R,
    undo: R,
    navs: Vec<NavItem>,
    rows: Vec<Row>,
    headers: Vec<Header>,
}

/// What a mouse press started.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Press {
    Popup,
    Tab,
    Action,
    Entry(Opt),
    Header(usize),
    Nav(usize),
    Resize { left: bool, right: bool, top: bool, bottom: bool, start: R },
    Divider { mouse: f32, start: i32 },
    ContentBar,
    NavBar,
    /// The search field: a drag selects its text (`EditBox.onDrag`).
    Search,
}

/// The drop-down list of a value's other choices (`ConfigValueSelector`).
#[derive(Clone, Copy)]
struct Popup {
    opt: Opt,
}

pub(super) struct ConfigScreen {
    ru: bool,
    active: usize,
    search: EditBox,
    nav_scroll: Scroll,
    content_scroll: Scroll,
    collapsed: [bool; CATS.len()],
    popup: Option<Popup>,
    /// `requestInfoAreaHeight`: it only grows while the screen is open.
    info_height: i32,
    /// The values as they were before the screen changed them (`AppliedConfigChangeTracker`).
    undo: Vec<(Opt, usize)>,
    press: Option<Press>,
    /// The window's size after a resize, and the stored ones.
    custom: Option<(i32, i32)>,
    window: (i32, i32),
    nav_width: i32,
    /// The navigation column's width while its divider is dragged.
    nav_drag: Option<i32>,
    lay: Layout,
    last: Instant,
    /// Escape, E or Done: the screen goes.
    pub(super) closed: bool,
    /// Clicks to play (the sound needs the game mutably).
    pub(super) clicks: u32,
    /// Ctrl+V/C/X in the search field, run in `frame`.
    pub(super) clipboard: Option<char>,
}

thread_local! {
    static SCREEN: RefCell<Option<ConfigScreen>> = const { RefCell::new(None) };
    /// `ConfigScreenHistory`: the category last chosen this game session.
    static LAST_CATEGORY: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

pub(super) fn with<T>(f: impl FnOnce(&mut ConfigScreen) -> T) -> Option<T> {
    SCREEN.with(|s| s.try_borrow_mut().ok().and_then(|mut s| s.as_mut().map(f)))
}

pub(super) fn is_open() -> bool {
    SCREEN.with(|s| s.try_borrow().is_ok_and(|s| s.is_some()))
}

pub(super) fn open(game: &Game<'_>) {
    let ru = jei::russian(&horizon_client::lang::code());
    let stored = |key: &str, default: i32, min: i32| {
        game.native_setting(jei::ID, key)
            .and_then(|v| v.trim().parse::<i32>().ok())
            .map_or(default, |v| v.clamp(min, MAX_WINDOW_SIZE))
    };
    let mut search = EditBox::new(0.0, 0.0, 0.0, f32::from(SEARCH_HEIGHT as u8));
    search.bordered = false;
    search.max_length = 64;
    search.focused = false;
    search.text_color = argb(SEARCH_TEXT);
    let screen = ConfigScreen {
        ru,
        active: LAST_CATEGORY.with(std::cell::Cell::get).min(CATS.len() - 1),
        search,
        nav_scroll: Scroll::default(),
        content_scroll: Scroll::default(),
        collapsed: [false; CATS.len()],
        popup: None,
        info_height: INFO_AREA_HEIGHT,
        undo: Vec::new(),
        press: None,
        custom: None,
        window: (
            stored("mezz_window_width", DEFAULT_WINDOW_WIDTH, MIN_WINDOW_WIDTH),
            stored("mezz_window_height", DEFAULT_WINDOW_HEIGHT, MIN_WINDOW_HEIGHT),
        ),
        nav_width: stored("mezz_navigation_width", DEFAULT_NAVIGATION_WIDTH, MIN_NAVIGATION_WIDTH),
        nav_drag: None,
        lay: Layout::default(),
        last: Instant::now(),
        closed: false,
        clicks: 0,
        clipboard: None,
    };
    SCREEN.with(|s| *s.borrow_mut() = Some(screen));
}

pub(super) fn close() {
    SCREEN.with(|s| *s.borrow_mut() = None);
}

/// The port's settings now.
fn settings(game: &Game<'_>) -> Settings {
    glue::with(|s| glue::settings(game, s)).unwrap_or_default()
}

impl ConfigScreen {
    fn searching(&self) -> bool {
        !self.search.value.is_empty()
    }

    fn matches(&self, opt: Opt) -> bool {
        let q = self.search.value.to_lowercase();
        let ru = self.ru;
        opt.name(ru).to_lowercase().contains(&q)
            || contextual_name(ru, opt).to_lowercase().contains(&q)
            || opt.description(ru).to_lowercase().contains(&q)
    }

    /// `ConfigScreenController.selectCategory`.
    fn select(&mut self, c: usize) {
        self.active = c;
        LAST_CATEGORY.with(|l| l.set(c));
        self.search.set_value("");
        self.content_scroll.reset();
        self.popup = None;
    }

    fn set_value(&mut self, game: &Game<'_>, opt: Opt, v: usize) {
        let mut s = settings(game);
        let old = opt.get(&s);
        if old == v {
            return;
        }
        // `AppliedConfigChangeTracker.add`: the first old value is kept, and
        // a value back where it was is no change.
        match self.undo.iter().position(|(o, _)| *o == opt) {
            Some(i) if self.undo[i].1 == v => {
                self.undo.remove(i);
            }
            Some(_) => {}
            None => self.undo.push((opt, old)),
        }
        opt.set(&mut s, v);
        glue::with(|st| glue::set_settings(game, st, s));
    }

    fn undo_all(&mut self, game: &Game<'_>) {
        let mut s = settings(game);
        for (opt, v) in self.undo.drain(..).rev() {
            opt.set(&mut s, v);
        }
        glue::with(|st| glue::set_settings(game, st, s));
    }

    // ---- layout ----

    fn layout(&mut self, game: &Game<'_>, screen_w: i32, screen_h: i32, s: &Settings) {
        let ru = self.ru;
        // `ConfigScreenResizer`: centred between the mod tabs' inset and the right margin.
        let left = (TAB_WIDTH - TAB_GUI_OVERLAP + TAB_SCREEN_MARGIN).clamp(0, screen_w - 1);
        let right = TAB_SCREEN_MARGIN.clamp(0, (screen_w - left - 1).max(0));
        let avail = (screen_w - left - right).max(1);
        let (ww, wh) = self.custom.unwrap_or(self.window);
        let gw = ww.clamp(MIN_WINDOW_WIDTH.min(avail), avail);
        let gh = wh.clamp(MIN_WINDOW_HEIGHT.min(screen_h.max(1)), screen_h.max(1));
        let area = [left + (avail - gw) / 2, (screen_h - gh) / 2, gw, gh];
        let inner = inset(area, BORDER_PADDING);
        let title = keep_top(inner, TITLE_HEIGHT);
        let info_h = self.info_height.min(inner[3] / 3);
        let info = keep_bottom(inner, info_h);
        let main = crop_bottom(crop_top(inner, TITLE_HEIGHT + SECTION_GAP), info_h + SECTION_GAP);
        let max_nav = (main[2] - NAV_DIVIDER_WIDTH - MIN_CONTENT_WIDTH).max(0);
        let nav_w = self.nav_drag.unwrap_or(self.nav_width).clamp(MIN_NAVIGATION_WIDTH.min(max_nav), max_nav);
        let nav = keep_left(main, nav_w);
        let divider = keep_left(crop_left(main, nav_w), NAV_DIVIDER_WIDTH);
        let with_scroll = crop_left(main, nav_w + NAV_DIVIDER_WIDTH);
        let top_bar = keep_top(with_scroll, SEARCH_HEIGHT);
        let actions = keep_right(top_bar, ACTION_BUTTON_SIZE * 2 + ACTION_BUTTON_GAP);
        let apply = center_v(keep_right(actions, ACTION_BUTTON_SIZE), ACTION_BUTTON_SIZE);
        let undo = center_v(keep_right(crop_right(actions, ACTION_BUTTON_SIZE + ACTION_BUTTON_GAP), ACTION_BUTTON_SIZE), ACTION_BUTTON_SIZE);
        let search = crop_right(top_bar, ACTION_BUTTON_SIZE * 2 + ACTION_BUTTON_GAP + SECTION_GAP);
        self.search.x = (search[0] + 5) as f32;
        self.search.y = (search[1] + (search[3] - 8) / 2) as f32;
        self.search.width = (search[2] - 9) as f32;
        let tab_x = (area[0] - TAB_WIDTH + TAB_GUI_OVERLAP).max(0);
        let tab = [tab_x, area[1] + TAB_VERTICAL_MARGIN, TAB_WIDTH, TAB_HEIGHT];

        // The navigation, with its scrollbar when it overflows.
        let mut navs = Vec::new();
        let mut nav_item_width = nav[2];
        for pass in 0..2 {
            navs.clear();
            let mut y = nav[1] - self.nav_scroll.current as i32;
            let mut total = 0;
            for (i, c) in CATS.iter().enumerate() {
                let indent = (c.depth * 12).min(48);
                let text_w = (nav_item_width - indent - 8 - 6).max(0);
                let lines = split_lines(game, &tr(ru, c.title), text_w, 2);
                let h = NAV_ITEM_HEIGHT.max(lines.len() as i32 * LINE + 5);
                let hover_h = if i + 1 < CATS.len() { h + NAV_ITEM_GAP } else { h };
                let row = [nav[0] + indent, y, nav_item_width - indent, h];
                navs.push(NavItem { cat: i, area: row, hover: [row[0], row[1], row[2], hover_h], lines });
                y += h + NAV_ITEM_GAP;
                total += h + NAV_ITEM_GAP;
            }
            total -= NAV_ITEM_GAP;
            let overflow = total > nav[3];
            let bar = if overflow { keep_right(nav, SCROLLBAR_WIDTH) } else { [0; 4] };
            self.nav_scroll.update(nav, total, bar);
            let wanted = if overflow { nav[2] - SCROLLBAR_WIDTH - SCROLLBAR_GAP } else { nav[2] };
            if wanted == nav_item_width || pass == 1 {
                break;
            }
            nav_item_width = wanted;
        }

        // The rows, with the content's scrollbar when they overflow.
        let viewport_full = crop_top(with_scroll, SEARCH_HEIGHT + SECTION_GAP);
        let mut bar_shown = self.content_scroll.content > viewport_full[3];
        let (mut rows, mut headers, mut content);
        loop {
            let column = if bar_shown { crop_right(with_scroll, SCROLLBAR_WIDTH + SCROLLBAR_GAP) } else { with_scroll };
            content = crop_top(column, SEARCH_HEIGHT + SECTION_GAP);
            rows = Vec::new();
            headers = Vec::new();
            let start = content[1] - self.content_scroll.current as i32;
            let mut y = start;
            if self.searching() {
                for c in &CATS {
                    for &opt in c.opts {
                        if self.matches(opt) {
                            rows.push(self.row(game, content, opt, 0, &mut y, true, s));
                        }
                    }
                }
            } else {
                for &opt in CATS[self.active].opts {
                    rows.push(self.row(game, content, opt, 0, &mut y, false, s));
                }
                for child in children(self.active) {
                    self.section(game, content, child, 0, &mut y, &mut rows, &mut headers, s);
                }
            }
            let total = y - start;
            let shown = total > viewport_full[3];
            if shown == bar_shown {
                let bar = if shown { crop_top(keep_right(with_scroll, SCROLLBAR_WIDTH), SEARCH_HEIGHT + SECTION_GAP) } else { [0; 4] };
                self.content_scroll.update(content, total, bar);
                break;
            }
            bar_shown = shown;
            self.content_scroll.content = total;
        }

        self.lay = Layout { area, title, tab, nav, divider, content, search, info, apply, undo, navs, rows, headers };
    }

    fn indent(content: R, depth: i32) -> i32 {
        (depth * SECTION_INDENT).min((content[2] - 4 - MIN_SECTION_WIDTH).max(0))
    }

    /// `ConfigScreenController.layoutCategorySection`.
    #[allow(clippy::too_many_arguments)]
    fn section(&self, game: &Game<'_>, content: R, c: usize, depth: i32, y: &mut i32, rows: &mut Vec<Row>, headers: &mut Vec<Header>, s: &Settings) {
        let indent = Self::indent(content, depth);
        let x = content[0] + 2 + indent;
        let w = (content[2] - 4 - indent).max(1);
        let lines = wrap(game, &tr(self.ru, CATS[c].title), (w - 2 * 6 - 12).max(1));
        let h = lines.len().max(1) as i32 * LINE + 8 + 5;
        let index = headers.len();
        headers.push(Header { cat: c, area: [x, *y, w, h], lines, bottom: *y + h });
        *y += h;
        if !self.collapsed[c] {
            for &opt in CATS[c].opts {
                rows.push(self.row(game, content, opt, depth + 1, y, false, s));
            }
            for child in children(c) {
                self.section(game, content, child, depth + 1, y, rows, headers, s);
            }
        }
        headers[index].bottom = (*y).max(headers[index].area[1] + h);
    }

    /// `ConfigScreenController.layoutEntry` and `CustomConfigEntry.updateBounds`.
    #[allow(clippy::too_many_arguments)]
    fn row(&self, game: &Game<'_>, content: R, opt: Opt, depth: i32, y: &mut i32, path: bool, s: &Settings) -> Row {
        let _ = s;
        let indent = Self::indent(content, depth);
        let x = content[0] + 2 + indent;
        let w = (content[2] - 4 - indent).max(1);
        let column = PREFERRED_VALUE_CONTROL_WIDTH
            .min(18.max(w - VALUE_CONTROL_RIGHT_RESERVE - VALUE_CONTROL_GAP - NAME_LEFT_PADDING - MIN_VALUE_NAME_WIDTH));
        let preferred = if opt.is_bool() { 18 } else { PREFERRED_VALUE_CONTROL_WIDTH };
        let control_w = preferred.min(column);
        let name_w = 40.max(w - (column + VALUE_CONTROL_RIGHT_RESERVE + VALUE_CONTROL_GAP) - NAME_LEFT_PADDING);
        let label = if path { contextual_name(self.ru, opt) } else { opt.name(self.ru) };
        let name = wrap(game, &label, name_w);
        let h = MIN_ROW_HEIGHT.max(name.len() as i32 * LINE + 8);
        let area = [x, *y, w, h];
        let name_y = *y + ((h - name.len() as i32 * LINE) as f32 / 2.0).round() as i32;
        let control_h = 18.min(h);
        let value = [x + w - control_w - VALUE_CONTROL_RIGHT_RESERVE, *y + (h - control_h) / 2, control_w, control_h];
        let reset = [x + w - RESET_BUTTON_SIZE - 2, *y + (h - RESET_BUTTON_SIZE) / 2, RESET_BUTTON_SIZE, RESET_BUTTON_SIZE];
        *y += h;
        Row { opt, area, name, name_y, value, reset }
    }

    fn row_of(&self, opt: Opt) -> Option<&Row> {
        self.lay.rows.iter().find(|r| r.opt == opt)
    }

    /// The popup's rectangle and its values (`ConfigPopupPlacement.placeNearAnchor`).
    fn popup_area(&self, game: &Game<'_>, s: &Settings) -> Option<(R, Vec<usize>)> {
        let popup = self.popup?;
        let row = self.row_of(popup.opt)?;
        let anchor = row.value;
        let clip = inset(self.lay.content, 1);
        let current = popup.opt.get(s);
        let values: Vec<usize> = (0..popup.opt.choices().len()).filter(|&v| v != current).collect();
        if values.is_empty() || !intersects(anchor, clip) {
            return None;
        }
        let text_w = values.iter().map(|&v| width(game, &popup.opt.value_name(self.ru, v))).max().unwrap_or(50);
        let w = (text_w + 12).clamp(50, 160).max(anchor[2]).min(clip[2]);
        let h = (values.len() as i32 * 20 + 2).min(clip[3]);
        let preferred_x = if w > anchor[2] { anchor[0] + anchor[2] - w } else { anchor[0] };
        let x = preferred_x.clamp(clip[0], (clip[0] + clip[2] - w).max(clip[0]));
        let (clip_top, clip_bottom) = (clip[1], clip[1] + clip[3]);
        let below = anchor[1] + anchor[3] - 1;
        let above = anchor[1] - h + 1;
        let visible = |y: i32| ((y + h).min(clip_bottom) - y.max(clip_top)).max(0);
        let y = if below + h <= clip_bottom {
            below
        } else if above >= clip_top {
            above
        } else {
            let preferred = if visible(above) > visible(below) { above } else { below };
            preferred.clamp(clip_top, (clip_bottom - h).max(clip_top))
        };
        Some(([x, y, w, h], values))
    }

    fn popup_value_at(&self, game: &Game<'_>, s: &Settings, p: (f32, f32)) -> Option<usize> {
        let (area, values) = self.popup_area(game, s)?;
        let inner = inset(area, 1);
        if !contains(inner, p) {
            return None;
        }
        values.iter().enumerate().find_map(|(i, &v)| contains([area[0] + 1, area[1] + 1 + i as i32 * 20, area[2] - 2, 20], p).then_some(v))
    }

    fn resize_handle(&self, p: (f32, f32)) -> Option<(bool, bool, bool, bool)> {
        let a = self.lay.area;
        if is_empty(a) || !contains(a, p) {
            return None;
        }
        // The tab covers part of the frame: no handle behind it.
        let tab = self.lay.tab;
        if p.1 >= tab[1] as f32 && p.1 < (tab[1] + tab[3]) as f32 && p.0 < (a[0] + RESIZE_HANDLE_SIZE) as f32 {
            return None;
        }
        let left = p.0 < (a[0] + RESIZE_HANDLE_SIZE) as f32;
        let right = p.0 >= (a[0] + a[2] - RESIZE_HANDLE_SIZE) as f32;
        let top = p.1 < (a[1] + RESIZE_HANDLE_SIZE) as f32;
        let bottom = p.1 >= (a[1] + a[3] - RESIZE_HANDLE_SIZE) as f32;
        (left || right || top || bottom).then_some((left, right, top, bottom))
    }

    // ---- drawing ----

    fn draw(&mut self, game: &Game<'_>, batch: &mut HudBatch) -> Option<Info> {
        let now = Instant::now();
        let _ = now.duration_since(self.last);
        self.last = now;
        let s = settings(game);
        self.content_scroll.step();
        self.nav_scroll.step();
        let (sw, sh) = (batch.width as i32, batch.height as i32);
        self.layout(game, sw, sh, &s);
        let ru = self.ru;
        let p = game.cursor;
        let lay = &self.lay;
        let atlas = &game.gui.atlas;
        let popup = self.popup_area(game, &s);
        let resizing = matches!(self.press, Some(Press::Resize { .. }));
        let handle = if resizing { Some((false, false, false, false)) } else { self.resize_handle(p) };
        let dividing = matches!(self.press, Some(Press::Divider { .. }));

        // The window, its tab, the title.
        nine(game, batch, &cs::GUI_BACKGROUND, lay.area);
        nine(game, batch, &cs::MOD_TAB_SELECTED, lay.tab);
        blit(game, batch, &cs::JEI_ICON, lay.tab[0] + (lay.tab[2] - ICON_SIZE) / 2, lay.tab[1] + (TAB_HEIGHT - ICON_SIZE) / 2);
        fitted_text(game, batch, &tr(ru, "jei.config"), lay.title, TITLE_TEXT, true);

        // Undo and Apply.
        let undoable = !self.undo.is_empty();
        button(game, batch, lay.undo, undoable, undoable && contains(lay.undo, p));
        icon(game, batch, &cs::ICON_X, lay.undo, undoable);
        button(game, batch, lay.apply, false, false);
        icon(game, batch, &cs::ICON_CHECK, lay.apply, false);

        // The navigation.
        fill(game, batch, lay.nav, NAVIGATION_BACKGROUND);
        let start = batch.vertices.len();
        let mut hovered_nav = None;
        for item in &lay.navs {
            let active = !self.searching() && item.cat == self.active;
            let hovered = contains(item.hover, p) && contains(lay.nav, p);
            if hovered {
                hovered_nav = Some(item.cat);
            }
            let r = item.area;
            fill(game, batch, r, if active { NAV_ITEM_ACTIVE_BACKGROUND } else { NAV_ITEM_BACKGROUND });
            if hovered {
                fill(game, batch, item.hover, NAV_ITEM_HOVER_BACKGROUND);
            }
            if active {
                fill(game, batch, [r[0], r[1], 2, r[3]], NAV_ITEM_ACTIVE_ACCENT);
            }
            fill(game, batch, [r[0], r[1] + r[3] - 1, r[2], 1], NAV_ITEM_DIVIDER);
            let color = if active {
                NAV_ITEM_ACTIVE_TEXT
            } else if hovered {
                NAV_ITEM_HOVER_TEXT
            } else {
                NAV_ITEM_TEXT
            };
            let tx = r[0] + if active { 8 } else { 6 };
            let mut ty = r[1] + ((r[3] - item.lines.len() as i32 * LINE) as f32 / 2.0).round() as i32;
            for line in &item.lines {
                text(game, batch, line, tx, ty, color);
                ty += LINE;
            }
        }
        batch.clip_since(start, lay.nav[0] as f32, lay.nav[1] as f32, (lay.nav[0] + lay.nav[2]) as f32, (lay.nav[1] + lay.nav[3]) as f32);
        inset_border(game, batch, lay.nav);
        let nav_marker = self.nav_scroll.marker();
        if !is_empty(nav_marker) {
            nine(game, batch, &cs::SCROLLBAR_BACKGROUND, self.nav_scroll.bar);
            nine(game, batch, &cs::SCROLLBAR_MARKER, nav_marker);
        }

        // The divider's grip.
        let d = lay.divider;
        if !is_empty(d) {
            let active = dividing || contains(d, p);
            if active {
                fill(game, batch, d, ROW_HOVER);
            }
            let gh = 20.min(d[3]);
            fill(game, batch, [d[0] + (d[2] - 2) / 2, d[1] + (d[3] - gh) / 2, 2, gh], if active { RESIZE_HANDLE_HOVER } else { RESIZE_GRIP });
        }

        // The search field.
        nine(game, batch, &cs::SEARCH_BACKGROUND, lay.search);
        let empty = self.search.value.is_empty();
        self.search.text_color = argb(if empty { SEARCH_HINT } else { SEARCH_TEXT });
        if empty && !self.search.focused {
            batch.text(atlas, &tr(ru, "mezz_config_gui.config.screen.search"), self.search.text_x(), self.search.text_y(), argb(SEARCH_HINT), true);
        }
        self.search.draw(batch, atlas, None);

        // The rows.
        let content = lay.content;
        fill(game, batch, content, VALUE_AREA_BACKGROUND);
        let start = batch.vertices.len();
        let hover_ok = popup.is_none() && contains(content, p);
        let mut hovered_entry: Option<Info> = None;
        for (index, row) in lay.rows.iter().enumerate() {
            if !intersects(row.area, content) {
                continue;
            }
            let a = row.area;
            let band = [a[0] + 1, a[1], a[2] - 2, a[3]];
            fill(game, batch, band, if index % 2 == 0 { ROW_STRIPE_LIGHT } else { ROW_STRIPE_DARK });
            let hovered = hover_ok && contains(a, p);
            if hovered {
                fill(game, batch, band, ROW_HOVER);
                hovered_entry = Some(Info::new(row.opt.name(ru), row.opt.description(ru)));
            }
            let mut y = row.name_y;
            for line in &row.name {
                text(game, batch, line, a[0] + NAME_LEFT_PADDING, y, ENTRY_TEXT);
                y += LINE;
            }
            let v = row.opt.get(&s);
            let over_value = hovered && contains(row.value, p);
            if row.opt.is_bool() {
                let r = row.value;
                let box_sprite = if over_value { &cs::CHECKBOX_HIGHLIGHT } else { &cs::CHECKBOX };
                blit(game, batch, box_sprite, r[0] + (r[2] - 18) / 2, r[1] + (r[3] - 18) / 2);
                if v != 0 {
                    icon(game, batch, &cs::ICON_CHECK, r, true);
                }
            } else {
                let r = row.value;
                button(game, batch, r, true, over_value);
                fitted_text(game, batch, &row.opt.value_name(ru, v), [r[0] + 4, r[1], (r[2] - 4 - 9 - 6).max(0), r[3]], ENTRY_TEXT, false);
                icon(game, batch, &cs::ICON_ARROW_DOWN, [r[0] + r[2] - 9 - 3, r[1], 9, r[3]], true);
            }
            let modified = v != row.opt.default();
            button(game, batch, row.reset, modified, modified && hovered && contains(row.reset, p));
            icon(game, batch, &cs::ICON_RESET, row.reset, modified);
        }
        if !self.searching() {
            for h in &lay.headers {
                let a = h.area;
                let bottom = a[1] + a[3];
                fill(game, batch, [a[0] + 1, bottom, 2, h.bottom - bottom], SECTION_HEADER_DIVIDER);
                if !intersects(a, content) {
                    continue;
                }
                fill(game, batch, [a[0] + 1, a[1] + 4, a[2] - 2, a[3] - 4], SECTION_HEADER_BACKGROUND);
                fill(game, batch, [a[0] + 1, bottom - 1, a[2] - 2, 1], SECTION_HEADER_DIVIDER);
                let disclosure = if self.collapsed[h.cat] { "▶" } else { "▼" };
                text(game, batch, disclosure, a[0] + 6, a[1] + 8, SECTION_HEADER_TEXT);
                let mut ty = a[1] + 8;
                for line in &h.lines {
                    text(game, batch, line, a[0] + 6 + 12, ty, SECTION_HEADER_TEXT);
                    ty += LINE;
                }
                if hover_ok && contains(a, p) {
                    hovered_entry = Some(Info::new(tr(ru, CATS[h.cat].title), tr(ru, CATS[h.cat].description)));
                }
            }
        }
        batch.clip_since(start, content[0] as f32, content[1] as f32, (content[0] + content[2]) as f32, (content[1] + content[3]) as f32);
        inset_border(game, batch, content);

        // The info panel: what the pointer is over, else the category.
        let control_info = if handle.is_some() && !dividing {
            Some(Info::new(tr(ru, "mezz_config_gui.config.screen.resize.title"), tr(ru, "mezz_config_gui.config.screen.resize.info")))
        } else if dividing || contains(lay.divider, p) {
            Some(Info::new(
                tr(ru, "mezz_config_gui.config.screen.navigationResize.title"),
                tr(ru, "mezz_config_gui.config.screen.navigationResize.info"),
            ))
        } else if contains(lay.search, p) {
            Some(Info::new(tr(ru, "mezz_config_gui.config.screen.search.info.title"), tr(ru, "mezz_config_gui.config.screen.search.info")))
        } else {
            None
        };
        let popup_info = self.popup.map(|pp| Info::new(pp.opt.name(ru), pp.opt.description(ru)));
        let over_popup = popup.as_ref().is_some_and(|(a, _)| contains(*a, p) && contains(inset(content, 1), p));
        let info = if over_popup {
            popup_info.clone()
        } else {
            control_info
                .or(hovered_entry)
                .or_else(|| hovered_nav.map(|c| Info::new(tr(ru, CATS[c].title), tr(ru, CATS[c].description))))
                .or(popup_info)
                .or_else(|| {
                    Some(if self.searching() {
                        Info::new(tr(ru, "mezz_config_gui.config.screen.search.info.title"), tr(ru, "mezz_config_gui.config.screen.search.info"))
                    } else {
                        Info::new(tr(ru, CATS[self.active].title), tr(ru, CATS[self.active].description))
                    })
                })
        };
        let info_area = lay.info;
        let text_w = (info_area[2] - 10).max(1);
        let (title_lines, body_lines) = match &info {
            Some(i) => (wrap(game, &i.title, text_w), i.lines.iter().flat_map(|l| wrap(game, l, text_w)).collect::<Vec<_>>()),
            None => (Vec::new(), Vec::new()),
        };
        let mut needed = 10 + (title_lines.len() + body_lines.len()) as i32 * LINE;
        if !title_lines.is_empty() {
            needed += 2;
        }
        fill(game, batch, info_area, INFO_BACKGROUND);
        fill(game, batch, [info_area[0], info_area[1], info_area[2], 1], INFO_BORDER);
        let start = batch.vertices.len();
        let bottom = info_area[1] + info_area[3] - 5;
        let mut y = info_area[1] + 5;
        for line in &title_lines {
            if y + LINE > bottom {
                break;
            }
            text(game, batch, line, info_area[0] + 5, y, INFO_TITLE_TEXT);
            y += LINE;
        }
        if !title_lines.is_empty() {
            y += 2;
        }
        for line in &body_lines {
            if y + LINE > bottom {
                break;
            }
            text(game, batch, line, info_area[0] + 5, y, INFO_TEXT);
            y += LINE;
        }
        batch.clip_since(start, info_area[0] as f32, info_area[1] as f32, (info_area[0] + info_area[2]) as f32, (info_area[1] + info_area[3]) as f32);

        // The content's scrollbar.
        let marker = self.content_scroll.marker();
        if !is_empty(marker) {
            nine(game, batch, &cs::SCROLLBAR_BACKGROUND, self.content_scroll.bar);
            nine(game, batch, &cs::SCROLLBAR_MARKER, marker);
        }

        // The drop-down list.
        if let (Some((area, values)), Some(pp)) = (&popup, self.popup) {
            let clip = inset(content, 1);
            let start = batch.vertices.len();
            let (x, y, right, bottom) = (area[0], area[1], area[0] + area[2], area[1] + area[3]);
            fill(game, batch, *area, SELECTOR_BACKGROUND);
            fill(game, batch, [x, y, area[2], 1], SELECTOR_BORDER_DARK);
            fill(game, batch, [x, y, 1, area[3]], SELECTOR_BORDER_DARK);
            fill(game, batch, [right - 1, y, 1, area[3]], SELECTOR_BORDER_LIGHT);
            fill(game, batch, [x, bottom - 1, area[2], 1], SELECTOR_BORDER_LIGHT);
            let inner = inset(*area, 1);
            let start_rows = batch.vertices.len();
            for (i, &v) in values.iter().enumerate() {
                let r = [x + 1, y + 1 + i as i32 * 20, area[2] - 2, 20];
                let hovered = contains(r, p);
                fill(game, batch, r, if hovered { SELECTOR_ROW_HOVER } else { SELECTOR_ROW_BACKGROUND });
                if i > 0 {
                    fill(game, batch, [r[0], r[1], r[2], 1], SELECTOR_DIVIDER);
                }
                fitted_text(game, batch, &pp.opt.value_name(ru, v), [r[0] + 4, r[1], (r[2] - 8).max(0), r[3]], ENTRY_TEXT, false);
            }
            batch.clip_since(start_rows, inner[0] as f32, inner[1] as f32, (inner[0] + inner[2]) as f32, (inner[1] + inner[3]) as f32);
            batch.clip_since(start, clip[0] as f32, clip[1] as f32, (clip[0] + clip[2]) as f32, (clip[1] + clip[3]) as f32);
        }

        // The window's resize grip.
        let a = self.lay.area;
        let (gr, gb) = (a[0] + a[2] - 4, a[1] + a[3] - 4);
        for i in 0..3 {
            let len = 11 - i * 3;
            let line_right = gr - i * 3;
            let x1 = line_right - len;
            for j in 0..=len {
                fill(game, batch, [x1 + j, gb - j, 1, 1], RESIZE_GRIP);
            }
        }

        if needed > self.info_height {
            self.info_height = needed;
        }

        // The tooltip.
        self.tooltip(game, &s, p, popup.as_ref().map(|(a, _)| *a))
    }

    /// `ConfigScreenView.getTooltipInfo`, the tab's title after it.
    fn tooltip(&self, game: &Game<'_>, s: &Settings, p: (f32, f32), popup: Option<R>) -> Option<Info> {
        let ru = self.ru;
        let lay = &self.lay;
        if contains(lay.undo, p) {
            let key = if self.undo.is_empty() { "undo.disabled.info" } else { "undo.info" };
            return Some(Info::new(tr(ru, "mezz_config_gui.config.screen.undo.title"), tr(ru, &format!("mezz_config_gui.config.screen.{key}"))));
        }
        if contains(lay.apply, p) {
            return Some(Info::new(
                tr(ru, "mezz_config_gui.config.screen.applyPending.title"),
                tr(ru, "mezz_config_gui.config.screen.applyPending.disabled.info"),
            ));
        }
        let content = lay.content;
        if let Some(area) = popup
            && contains(inset(content, 1), p)
        {
            if let (Some(v), Some(pp)) = (self.popup_value_at(game, s, p), self.popup) {
                return pp.opt.value_description(ru, v).map(|d| Info { title: d, lines: Vec::new() });
            }
            if contains(area, p) {
                return None;
            }
        }
        if contains(content, p) {
            for row in &lay.rows {
                if !contains(row.area, p) {
                    continue;
                }
                if contains(row.reset, p) {
                    return Some(Info::new(tr(ru, "mezz_config_gui.config.screen.reset"), tr(ru, "mezz_config_gui.config.screen.reset.value.info")));
                }
                if contains(row.value, p) {
                    let v = row.opt.get(s);
                    if row.opt.is_bool() {
                        let title = row.opt.value_description(ru, v).unwrap_or_default();
                        let action = if v != 0 { "disable" } else { "enable" };
                        return Some(Info::new(title, tr(ru, &format!("mezz_config_gui.config.value.boolean.action.{action}"))));
                    }
                    return row.opt.value_description(ru, v).map(|d| Info { title: d, lines: Vec::new() });
                }
            }
        }
        if contains(lay.tab, p) {
            return Some(Info { title: jei::NAME.to_owned(), lines: Vec::new() });
        }
        None
    }

    // ---- input ----

    fn press(&mut self, game: &Game<'_>, p: (f32, f32), button: i32) {
        let s = settings(game);
        self.press = None;
        if button == 0 && contains(self.lay.tab, p) {
            self.press = Some(Press::Tab);
            return;
        }
        if self.popup.is_some() {
            self.press = Some(Press::Popup);
            return;
        }
        if button == 0
            && let Some((left, right, top, bottom)) = self.resize_handle(p)
        {
            self.press = Some(Press::Resize { left, right, top, bottom, start: self.lay.area });
            return;
        }
        if button == 0 && contains(self.lay.divider, p) {
            self.press = Some(Press::Divider { mouse: p.0, start: self.lay.nav[2] });
            return;
        }
        if button == 0 && (contains(self.lay.undo, p) || contains(self.lay.apply, p)) {
            self.press = Some(Press::Action);
            return;
        }
        let field = [self.search.x as i32, self.search.y as i32, self.search.width as i32, SEARCH_HEIGHT];
        if button == 1 && contains(field, p) {
            self.search.set_value("");
            self.search.focused = true;
            self.search.reset_blink();
            self.content_scroll.reset();
            return;
        }
        if button == 0 && self.content_scroll.start_drag(p) {
            self.press = Some(Press::ContentBar);
            return;
        }
        if button == 0 && self.nav_scroll.start_drag(p) {
            self.press = Some(Press::NavBar);
            return;
        }
        if contains(field, p) {
            if button == 0 {
                let shift = game.has_shift_down();
                self.search.focused = true;
                self.search.click(&game.gui.atlas, p.0, shift, false);
                self.search.reset_blink();
                self.press = Some(Press::Search);
            }
            return;
        }
        self.search.focused = false;
        // The rows, then the headers, then the navigation (`ConfigInputRouter`).
        if contains(self.lay.content, p) {
            for row in &self.lay.rows {
                if contains(row.area, p) && self.entry_click(row, &s, p, button, true) {
                    self.press = Some(Press::Entry(row.opt));
                    return;
                }
            }
            if !self.searching() {
                for h in &self.lay.headers {
                    if button == 0 && contains(h.area, p) {
                        self.press = Some(Press::Header(h.cat));
                        return;
                    }
                }
            }
        }
        if button == 0 && contains(self.lay.nav, p) {
            for item in &self.lay.navs {
                if contains(item.hover, p) {
                    self.press = Some(Press::Nav(item.cat));
                    return;
                }
            }
        }
    }

    /// `CustomConfigEntry.onMouseClicked`: the reset button, the check box or
    /// the drop-down. `simulate` only tells whether it would take the click.
    fn entry_click(&self, row: &Row, s: &Settings, p: (f32, f32), button: i32, simulate: bool) -> bool {
        let v = row.opt.get(s);
        if v != row.opt.default() && contains(row.reset, p) {
            return simulate || button >= 0;
        }
        contains(row.value, p) && button == 0
    }

    fn release(&mut self, game: &Game<'_>, p: (f32, f32), button: i32) {
        let s = settings(game);
        let Some(press) = self.press.take() else {
            return;
        };
        match press {
            Press::Tab => {}
            Press::Resize { .. } => {
                if let Some((w, h)) = self.custom.take() {
                    let (w, h) = (w.clamp(MIN_WINDOW_WIDTH, MAX_WINDOW_SIZE), h.clamp(MIN_WINDOW_HEIGHT, MAX_WINDOW_SIZE));
                    self.window = (w, h);
                    game.native_set_setting(jei::ID, "mezz_window_width", &w.to_string());
                    game.native_set_setting(jei::ID, "mezz_window_height", &h.to_string());
                }
            }
            Press::Divider { .. } => {
                if let Some(w) = self.nav_drag.take()
                    && w != self.nav_width
                {
                    self.nav_width = w.clamp(MIN_NAVIGATION_WIDTH, MAX_WINDOW_SIZE);
                    game.native_set_setting(jei::ID, "mezz_navigation_width", &self.nav_width.to_string());
                }
            }
            Press::ContentBar => self.content_scroll.drag = None,
            Press::NavBar => self.nav_scroll.drag = None,
            Press::Action => {
                if button == 0 && contains(self.lay.undo, p) && !self.undo.is_empty() {
                    self.undo_all(game);
                    self.clicks += 1;
                }
            }
            Press::Popup => {
                let clip = inset(self.lay.content, 1);
                let area = self.popup_area(game, &s).map(|(a, _)| a);
                if let Some(a) = area
                    && contains(clip, p)
                    && contains(a, p)
                {
                    if let (Some(v), Some(pp)) = (self.popup_value_at(game, &s, p), self.popup)
                        && button == 0
                    {
                        self.set_value(game, pp.opt, v);
                        self.popup = None;
                        self.clicks += 1;
                    }
                } else {
                    self.popup = None;
                    self.clicks += 1;
                }
            }
            Press::Entry(opt) => {
                let Some(index) = self.lay.rows.iter().position(|r| r.opt == opt) else {
                    return;
                };
                let row = &self.lay.rows[index];
                if !contains(row.area, p) {
                    return;
                }
                let v = opt.get(&s);
                if v != opt.default() && contains(row.reset, p) {
                    self.set_value(game, opt, opt.default());
                    self.clicks += 1;
                } else if contains(row.value, p) && button == 0 {
                    if opt.is_bool() {
                        self.set_value(game, opt, 1 - v.min(1));
                    } else {
                        self.popup = Some(Popup { opt });
                    }
                    self.clicks += 1;
                }
            }
            Press::Header(c) => {
                if self.lay.headers.iter().any(|h| h.cat == c && contains(h.area, p)) {
                    self.collapsed[c] = !self.collapsed[c];
                    self.clicks += 1;
                }
            }
            Press::Nav(c) => {
                if contains(self.lay.nav, p) && self.lay.navs.iter().any(|n| n.cat == c && contains(n.hover, p)) {
                    self.select(c);
                    self.clicks += 1;
                }
            }
            Press::Search => {}
        }
    }

    fn drag(&mut self, game: &Game<'_>, p: (f32, f32)) {
        match self.press {
            Some(Press::Resize { left, right, top, bottom, start }) => {
                let (sw, sh) = gui_size();
                let lx = TAB_WIDTH - TAB_GUI_OVERLAP + TAB_SCREEN_MARGIN;
                let avail = (sw - lx - TAB_SCREEN_MARGIN).max(1) as f32;
                let cx = lx as f32 + avail / 2.0;
                let cy = sh as f32 / 2.0;
                let mut w = start[2];
                let mut h = start[3];
                if left {
                    w = ((cx - p.0) * 2.0).round() as i32;
                } else if right {
                    w = ((p.0 - cx) * 2.0).round() as i32;
                }
                if top {
                    h = ((cy - p.1) * 2.0).round() as i32;
                } else if bottom {
                    h = ((p.1 - cy) * 2.0).round() as i32;
                }
                let w = w.clamp(MIN_WINDOW_WIDTH.min(avail as i32), avail as i32);
                let h = h.clamp(MIN_WINDOW_HEIGHT.min(sh), sh.max(1));
                self.custom = Some((w, h));
            }
            Some(Press::Divider { mouse, start }) => {
                self.nav_drag = Some(start + (p.0 - mouse).round() as i32);
            }
            Some(Press::ContentBar) => self.content_scroll.drag_to(p.1),
            Some(Press::NavBar) => self.nav_scroll.drag_to(p.1),
            Some(Press::Search) if self.search.focused => {
                self.search.drag_to(&game.gui.atlas, p.0);
                self.search.reset_blink();
            }
            _ => {}
        }
    }

    fn scroll(&mut self, game: &Game<'_>, p: (f32, f32), dy: f64) {
        let s = settings(game);
        if let Some((area, _)) = self.popup_area(game, &s)
            && contains(area, p)
        {
            return;
        }
        if contains(self.lay.nav, p) {
            self.nav_scroll.scroll(dy);
        } else if contains(self.lay.content, p) {
            self.content_scroll.scroll(dy);
        }
    }

    /// A key; Escape and the inventory key close the screen.
    fn key(&mut self, game: &Game<'_>, key: &str, modifiers: u32) {
        let control = modifiers & 2 != 0;
        let shift = modifiers & 1 != 0;
        if self.popup.is_some() && key == "key.keyboard.escape" {
            self.popup = None;
            return;
        }
        if self.search.focused {
            match key {
                "key.keyboard.escape" => {}
                "key.keyboard.v" if control => {
                    self.clipboard = Some('v');
                    return;
                }
                "key.keyboard.c" if control => {
                    self.clipboard = Some('c');
                    return;
                }
                "key.keyboard.x" if control => {
                    self.clipboard = Some('x');
                    return;
                }
                _ => {
                    let before = self.search.value.clone();
                    if let Some(code) = horizon_client::keybinds::key_code(key) {
                        self.search.key(code, control, shift);
                    }
                    self.search.reset_blink();
                    if self.search.value != before {
                        self.content_scroll.reset();
                    }
                    return;
                }
            }
        }
        let inventory = game.settings.keys.get("key.inventory") == key;
        if key == "key.keyboard.escape" || inventory {
            self.closed = true;
        }
    }

    fn char_typed(&mut self, c: char) {
        if self.search.focused && !c.is_control() {
            self.search.insert_text(&c.to_string());
            self.search.reset_blink();
            self.content_scroll.reset();
        }
    }

    /// Ctrl+V/C/X in the search field, with the game's clipboard.
    pub(super) fn clipboard(&mut self, op: char, get: impl FnOnce() -> String, set: impl FnOnce(&str)) {
        match op {
            'v' => {
                let text: String = get().chars().filter(|c| !c.is_control()).collect();
                self.search.insert_text(&text);
                self.content_scroll.reset();
            }
            _ => {
                let selected = self.search.selected_text();
                if !selected.is_empty() {
                    set(&selected);
                    if op == 'x' {
                        self.search.delete_chars(0);
                        self.content_scroll.reset();
                    }
                }
            }
        }
    }
}

fn gui_size() -> (i32, i32) {
    let (w, h) = LAST_SIZE.with(std::cell::Cell::get);
    (w.max(1), h.max(1))
}

thread_local! {
    /// The GUI's size the last frame (the drag hook has no batch).
    static LAST_SIZE: std::cell::Cell<(i32, i32)> = const { std::cell::Cell::new((0, 0)) };
}

// ---- entry points ---------------------------------------------------------------------------

/// Draws the screen; the tooltip it wants, as lines with their colours.
pub(super) fn draw(game: &Game<'_>, batch: &mut HudBatch) -> Option<Vec<(String, u32)>> {
    LAST_SIZE.with(|s| s.set((batch.width as i32, batch.height as i32)));
    let info = with(|c| c.draw(game, batch)).flatten()?;
    let mut lines = Vec::new();
    let max = TOOLTIP_MAX_WIDTH as i32;
    for l in wrap(game, &info.title, max) {
        lines.push((l, TOOLTIP_TITLE_TEXT));
    }
    for line in &info.lines {
        for l in wrap(game, line, max) {
            lines.push((l, TOOLTIP_TEXT));
        }
    }
    Some(lines)
}

pub(super) fn press(game: &Game<'_>, p: (f32, f32), button: i32) {
    with(|c| c.press(game, p, button));
}

pub(super) fn release(game: &Game<'_>, p: (f32, f32), button: i32) {
    with(|c| c.release(game, p, button));
}

pub(super) fn drag(game: &Game<'_>, p: (f32, f32)) {
    with(|c| c.drag(game, p));
}

pub(super) fn scroll(game: &Game<'_>, p: (f32, f32), dy: f64) {
    with(|c| c.scroll(game, p, dy));
}

pub(super) fn key(game: &Game<'_>, key: &str, modifiers: u32) {
    with(|c| c.key(game, key, modifiers));
}

pub(super) fn char_typed(c: char) {
    with(|s| s.char_typed(c));
}
