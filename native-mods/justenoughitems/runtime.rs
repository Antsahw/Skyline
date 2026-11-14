// Just Enough Items for Horizon: the mod's logic, with no game types, so the
// same file builds and tests on its own (`cargo test` at the mod's root) and is
// what the bundle adds to the game as `crate::game::justenoughitems`.
//
// A port of JustEnoughItems (JEI) by mezz (MIT,
// https://github.com/mezz/JustEnoughItems): its search grammar
// (`ElementPrefixParser`, `IngredientFilter`), the ingredient list and
// bookmark overlays, the recipe lookups and the recipes screen's paging.
// `justenoughitems_glue` reads the game's items and recipes into this model
// and draws it.
use std::collections::HashMap;

pub const ID: &str = "justenoughitems";
pub const NAME: &str = "Just Enough Items";
pub const VERSION: &str = "1.1.2";

/// Russian labels when the game's language is Russian, English otherwise.
pub fn russian(language: &str) -> bool {
    language.starts_with("ru")
}

// ---- search ------------------------------------------------------------------

/// How a search source takes part (`SearchMode`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Plain words search it too.
    Enabled,
    /// Only words with its prefix.
    RequirePrefix,
    /// Never; its prefix is an ordinary character.
    Disabled,
}

impl Mode {
    /// In JEI's order (`SearchMode`).
    pub const ALL: [Mode; 3] = [Mode::Enabled, Mode::RequirePrefix, Mode::Disabled];
    /// The settings file's word: JEI's value name in lower case.
    pub fn name(self) -> &'static str {
        match self {
            Mode::Enabled => "enabled",
            Mode::RequirePrefix => "require_prefix",
            Mode::Disabled => "disabled",
        }
    }
    pub fn parse(s: &str) -> Option<Mode> {
        Mode::ALL.into_iter().find(|m| m.name() == s.trim())
    }
}

/// Where a search word looks.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Source {
    /// The display name, and every enabled source.
    Any,
    /// `@`: the mod's name or id.
    Mod,
    /// `#`: the tooltip's text.
    Tooltip,
    /// `$`: an item tag.
    Tag,
    /// `%`: a creative tab's name.
    CreativeTab,
    /// `^`: a colour (Horizon has no colour data: never matches).
    Colour,
    /// `&`: the item's id.
    Identifier,
}

impl Source {
    fn of_prefix(c: char) -> Option<Source> {
        Some(match c {
            '@' => Source::Mod,
            '#' => Source::Tooltip,
            '$' => Source::Tag,
            '%' => Source::CreativeTab,
            '^' => Source::Colour,
            '&' => Source::Identifier,
            _ => return None,
        })
    }
}

/// JEI's search modes (`IngredientFilterConfig`), with the creative tab and
/// identifier prefixes on (JEI ships them off).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Modes {
    pub mod_name: Mode,
    pub tooltip: Mode,
    pub tag: Mode,
    pub creative_tab: Mode,
    pub colour: Mode,
    pub identifier: Mode,
}

impl Default for Modes {
    fn default() -> Self {
        Modes {
            mod_name: Mode::RequirePrefix,
            tooltip: Mode::Enabled,
            tag: Mode::RequirePrefix,
            creative_tab: Mode::RequirePrefix,
            colour: Mode::Disabled,
            identifier: Mode::RequirePrefix,
        }
    }
}

impl Modes {
    fn of(&self, source: Source) -> Mode {
        match source {
            Source::Any => Mode::Enabled,
            Source::Mod => self.mod_name,
            Source::Tooltip => self.tooltip,
            Source::Tag => self.tag,
            Source::CreativeTab => self.creative_tab,
            Source::Colour => self.colour,
            Source::Identifier => self.identifier,
        }
    }
}

/// What a search reads of one ingredient, lowercased.
pub struct Searchable<'a> {
    pub name: &'a str,
    /// The mod's display name and its id (`minecraft`).
    pub mod_name: &'a str,
    pub mod_id: &'a str,
    /// `minecraft:stone`.
    pub id: &'a str,
    /// `minecraft:logs`, no `#`.
    pub tags: &'a [String],
    pub tabs: &'a [String],
    /// The tooltip's lines below the name, None while not worked out.
    pub tooltip: Option<&'a str>,
}

#[derive(Clone, PartialEq, Eq, Debug)]
struct Token {
    negate: bool,
    source: Source,
    text: String,
}

/// A parsed search: groups split by `|`, any of which may match; every word
/// of a group must.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Query {
    groups: Vec<Vec<Token>>,
}

/// Splits one `|` group into words: spaces separate them, except inside
/// quotes; a word may start with `-` and then a prefix.
fn words(group: &str) -> Vec<(String, bool)> {
    let mut out = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut had_quote = false;
    for c in group.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                had_quote = true;
            }
            c if c.is_whitespace() && !quoted => {
                if !word.is_empty() || had_quote {
                    out.push((std::mem::take(&mut word), had_quote));
                }
                had_quote = false;
            }
            c => word.push(c),
        }
    }
    if !word.is_empty() || had_quote {
        out.push((word, had_quote));
    }
    out
}

impl Query {
    /// Parses the search box's text as JEI does, with `modes` deciding which
    /// prefixes count.
    pub fn parse(text: &str, modes: &Modes) -> Query {
        let lower = text.to_lowercase();
        let groups = lower
            .split('|')
            .map(|group| {
                words(group)
                    .into_iter()
                    .filter_map(|(word, _)| {
                        let mut rest = word.as_str();
                        // A dash on its own (an exclusion being typed) is no word yet.
                        if rest == "-" {
                            return None;
                        }
                        let negate = rest.len() > 1 && rest.starts_with('-');
                        if negate {
                            rest = &rest[1..];
                        }
                        let first = rest.chars().next()?;
                        let (source, text) = match Source::of_prefix(first) {
                            Some(s) if modes.of(s) != Mode::Disabled => (s, &rest[first.len_utf8()..]),
                            _ => (Source::Any, rest),
                        };
                        let text = text.trim();
                        (!text.is_empty()).then(|| Token {
                            negate,
                            source,
                            text: text.to_owned(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|g| !g.is_empty())
            .collect();
        Query { groups }
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    /// The search reads tooltips: a `#` word, or plain words while tooltips
    /// are searched without the prefix.
    pub fn needs_tooltips(&self, modes: &Modes) -> bool {
        self.groups.iter().flatten().any(|t| match t.source {
            Source::Tooltip => true,
            Source::Any => modes.tooltip == Mode::Enabled,
            _ => false,
        })
    }

    pub fn matches(&self, s: &Searchable<'_>, modes: &Modes) -> bool {
        self.is_empty()
            || self
                .groups
                .iter()
                .any(|group| group.iter().all(|t| token_hits(t, s, modes) != t.negate))
    }
}

fn token_hits(t: &Token, s: &Searchable<'_>, modes: &Modes) -> bool {
    let text = t.text.as_str();
    let in_mod = || s.mod_name.contains(text) || s.mod_id.contains(text);
    let in_tooltip = || s.tooltip.is_some_and(|tip| tip.contains(text));
    let in_tags = || s.tags.iter().any(|tag| tag.contains(text));
    let in_tabs = || s.tabs.iter().any(|tab| tab.contains(text));
    let in_id = || s.id.contains(text);
    match t.source {
        Source::Mod => in_mod(),
        Source::Tooltip => in_tooltip(),
        Source::Tag => in_tags(),
        Source::CreativeTab => in_tabs(),
        Source::Colour => false,
        Source::Identifier => in_id(),
        Source::Any => {
            s.name.contains(text)
                || (modes.mod_name == Mode::Enabled && in_mod())
                || (modes.tooltip == Mode::Enabled && in_tooltip())
                || (modes.tag == Mode::Enabled && in_tags())
                || (modes.creative_tab == Mode::Enabled && in_tabs())
                || (modes.identifier == Mode::Enabled && in_id())
        }
    }
}

// ---- grids and pages -----------------------------------------------------------

/// A rectangle in GUI pixels: x, y, width, height.
pub type Rect = [f32; 4];

pub fn inside(p: (f32, f32), r: Rect) -> bool {
    p.0 >= r[0] && p.1 >= r[1] && p.0 < r[0] + r[2] && p.1 < r[1] + r[3]
}

/// How many pages `len` items take, `per_page` a page: at least one.
pub fn pages(len: usize, per_page: usize) -> usize {
    if per_page == 0 {
        return 1;
    }
    len.div_ceil(per_page).max(1)
}

/// The page after `page` turning by `delta`, wrapping around as JEI's
/// navigation does.
pub fn turn(page: usize, delta: i32, pages: usize) -> usize {
    let pages = pages.max(1) as i64;
    (page as i64 + i64::from(delta)).rem_euclid(pages) as usize
}

/// Which of `len` stacks a slot shows at `millis`: one a second, as JEI's
/// cycling slots (`CycleTicker`), shifted by `offset` so slots don't change in
/// step.
pub fn cycle(len: usize, millis: u64, offset: usize) -> usize {
    if len == 0 {
        return 0;
    }
    ((millis / 1000) as usize + offset) % len
}

// ---- bookmarks -----------------------------------------------------------------

/// The bookmark list, by stack key (`minecraft:potion[...]`).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Bookmarks {
    pub keys: Vec<String>,
}

impl Bookmarks {
    /// Adds `key` (to the front when `front`, as JEI's "add bookmarks to the
    /// front"), or takes it away when already there: true when added.
    pub fn toggle(&mut self, key: &str, front: bool) -> bool {
        if let Some(i) = self.keys.iter().position(|k| k == key) {
            self.keys.remove(i);
            return false;
        }
        if front {
            self.keys.insert(0, key.to_owned());
        } else {
            self.keys.push(key.to_owned());
        }
        true
    }
    /// One key a line; `#` starts a comment.
    pub fn parse(text: &str) -> Bookmarks {
        let mut keys: Vec<String> = Vec::new();
        for line in text.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') || keys.iter().any(|k| k == line) {
                continue;
            }
            keys.push(line.to_owned());
        }
        Bookmarks { keys }
    }
    pub fn write(&self) -> String {
        let mut out = String::from("# Just Enough Items bookmarks: one item a line, as /give writes it\n");
        for key in &self.keys {
            out.push_str(key);
            out.push('\n');
        }
        out
    }
}

// ---- recipes ---------------------------------------------------------------------

/// JEI's vanilla recipe categories, in the order JEI registers them
/// (`VanillaPlugin.registerCategories`).
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub enum Category {
    Crafting,
    Stonecutting,
    Smelting,
    Smoking,
    Blasting,
    Campfire,
    SmeltingFuel,
    SmokingFuel,
    BlastingFuel,
    Brewing,
    Anvil,
    Grindstone,
    Smithing,
    Compostable,
}

impl Category {
    pub const ALL: [Category; 14] = [
        Category::Crafting,
        Category::Stonecutting,
        Category::Smelting,
        Category::Smoking,
        Category::Blasting,
        Category::Campfire,
        Category::SmeltingFuel,
        Category::SmokingFuel,
        Category::BlastingFuel,
        Category::Brewing,
        Category::Anvil,
        Category::Grindstone,
        Category::Smithing,
        Category::Compostable,
    ];
    pub fn index(self) -> usize {
        Category::ALL.iter().position(|c| *c == self).unwrap_or(0)
    }
    /// The items that make its recipes (`registerRecipeCatalysts`), first one
    /// the tab's icon.
    pub fn catalysts(self) -> &'static [&'static str] {
        match self {
            Category::Crafting => &["minecraft:crafting_table", "minecraft:crafter"],
            Category::Stonecutting => &["minecraft:stonecutter"],
            Category::Smelting | Category::SmeltingFuel => &["minecraft:furnace"],
            Category::Smoking | Category::SmokingFuel => &["minecraft:smoker"],
            Category::Blasting | Category::BlastingFuel => &["minecraft:blast_furnace"],
            Category::Campfire => &["minecraft:campfire", "minecraft:soul_campfire"],
            Category::Brewing => &["minecraft:brewing_stand"],
            Category::Anvil => &["minecraft:anvil"],
            Category::Grindstone => &["minecraft:grindstone"],
            Category::Smithing => &["minecraft:smithing_table"],
            Category::Compostable => &["minecraft:composter"],
        }
    }
}

/// A stack of a recipe: `id` is the ingredient's (one per item and
/// components, damage and count aside), with its count and damage here.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Stack {
    pub id: u32,
    pub count: u32,
    pub damage: u32,
}

impl Stack {
    pub fn one(id: u32) -> Stack {
        Stack { id, count: 1, damage: 0 }
    }
}

/// One recipe as JEI shows it.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Recipe {
    pub category: Option<Category>,
    /// `minecraft:oak_planks`, or "" for one JEI makes up.
    pub id: String,
    /// Each input slot's stacks, any one of which fits; empty for an empty
    /// slot. Crafting: the grid row by row, `width` wide.
    pub inputs: Vec<Vec<Stack>>,
    /// Each output slot's stacks (one slot but for the grindstone's XP-less
    /// case, several stacks when it varies).
    pub outputs: Vec<Vec<Stack>>,
    pub width: u32,
    pub height: u32,
    pub shapeless: bool,
    /// Cooking: ticks and experience. Fuel: burn ticks. Compost: chance.
    /// Anvil: the level cost.
    pub ticks: u32,
    pub experience: f32,
    pub chance: f32,
    pub cost: u32,
    /// The recipe book's id, -1 when the book doesn't show it.
    pub book_id: i32,
    /// Shown but not looked up: a furnace's fuel slot (any fuel).
    pub fuel: Vec<Stack>,
    /// Fuel: the ticks one item takes to smelt (`FuelingRecipe.cookingTime`).
    pub item_ticks: u32,
    /// Brewing: the steps from a water bottle, `u32::MAX` when unknown.
    pub steps: u32,
    /// The output is only drawn: "show recipes" doesn't find it (the grindstone).
    pub render_only_output: bool,
    /// Anvil: the cost of each output, when it varies (enchanted books).
    pub costs: Vec<u32>,
}

impl Recipe {
    pub fn category(&self) -> Category {
        self.category.unwrap_or(Category::Crafting)
    }

    /// JEI's focus link (`IRecipeLayoutBuilder.createFocusLink`, as the anvil
    /// and the grindstone set it): the slots whose alternatives go together
    /// index by index (a book and the item it enchants, an enchanted item and
    /// what the grindstone gives back), limited together to the alternatives
    /// that hold `focus` (`FocusLink.getVisibleIngredientIndexes`).
    pub fn focus_link(&self, focus: Option<u32>) -> FocusLink {
        let mut link = FocusLink::default();
        if !matches!(self.category, Some(Category::Anvil | Category::Grindstone)) {
            return link;
        }
        let empty: Vec<Stack> = Vec::new();
        let input = |i: usize| self.inputs.get(i).unwrap_or(&empty);
        let output = self.outputs.first().unwrap_or(&empty);
        let n = output.len();
        // JEI links the slots that have as many alternatives as the output
        // when the other one has a single stack (an empty slot here is JEI's
        // list of empty stacks, as long as the others).
        let (left, right) = (input(0).len(), input(1).len());
        let fits = |len: usize| len == n || len == 0;
        let linked: Vec<usize> = match (fits(left), fits(right)) {
            (true, true) => vec![0, 1],
            (true, false) if right == 1 => vec![0],
            (false, true) if left == 1 => vec![1],
            _ => Vec::new(),
        };
        let linked: Vec<usize> = linked.into_iter().filter(|&i| !input(i).is_empty()).collect();
        if n < 2 || linked.is_empty() {
            return link;
        }
        if let Some(f) = focus {
            link.indexes = (0..n)
                .filter(|&k| output[k].id == f || linked.iter().any(|&i| input(i)[k].id == f))
                .collect();
        }
        link.inputs = linked;
        link.output = true;
        link
    }
}

/// The slots of a recipe that show the same alternative, and the alternatives
/// the focus limits them to (none: all, cycling together).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct FocusLink {
    pub inputs: Vec<usize>,
    pub output: bool,
    pub indexes: Vec<usize>,
}

impl FocusLink {
    /// The index of the alternative the linked slots show now, of `len`.
    pub fn index(&self, len: usize, millis: u64) -> usize {
        if self.indexes.is_empty() {
            cycle(len, millis, 0)
        } else {
            self.indexes[cycle(self.indexes.len(), millis, 0)]
        }
    }

    fn limit(&self, stacks: &[Stack]) -> Vec<Stack> {
        if self.indexes.is_empty() {
            return stacks.to_vec();
        }
        self.indexes.iter().filter_map(|&k| stacks.get(k).copied()).collect()
    }

    /// Input slot `i`'s stacks as shown.
    pub fn input(&self, r: &Recipe, i: usize) -> Vec<Stack> {
        let stacks = r.inputs.get(i).map_or(&[][..], Vec::as_slice);
        if self.inputs.contains(&i) { self.limit(stacks) } else { stacks.to_vec() }
    }

    /// The output slot's stacks as shown.
    pub fn output(&self, r: &Recipe) -> Vec<Stack> {
        let stacks = r.outputs.first().map_or(&[][..], Vec::as_slice);
        if self.output { self.limit(stacks) } else { stacks.to_vec() }
    }
}

/// What the recipes screen shows: an ingredient's recipes or uses, or a
/// whole category (a catalyst's tab, or a tab of all its recipes).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Lookup {
    Recipes(u32),
    Uses(u32),
    Category(Category),
}

impl Lookup {
    pub fn focus(self) -> Option<u32> {
        match self {
            Lookup::Recipes(id) | Lookup::Uses(id) => Some(id),
            Lookup::Category(_) => None,
        }
    }
}

/// Which recipes make and use each ingredient, and each category's.
#[derive(Clone, Debug, Default)]
pub struct Index {
    pub recipes: Vec<Recipe>,
    by_output: HashMap<u32, Vec<usize>>,
    by_input: HashMap<u32, Vec<usize>>,
    by_category: HashMap<Category, Vec<usize>>,
    /// The ingredient ids of each category's catalysts.
    catalysts: HashMap<Category, Vec<u32>>,
}

impl Index {
    /// `catalysts` maps an item id (`minecraft:furnace`) to its ingredient.
    pub fn new(recipes: Vec<Recipe>, catalyst: impl Fn(&str) -> Option<u32>) -> Index {
        let mut index = Index::default();
        for (i, r) in recipes.iter().enumerate() {
            let mut seen_out = Vec::new();
            let outputs = if r.render_only_output { &[][..] } else { r.outputs.as_slice() };
            for s in outputs.iter().flatten() {
                if !seen_out.contains(&s.id) {
                    seen_out.push(s.id);
                    index.by_output.entry(s.id).or_default().push(i);
                }
            }
            let mut seen_in = Vec::new();
            for s in r.inputs.iter().flatten() {
                if !seen_in.contains(&s.id) {
                    seen_in.push(s.id);
                    index.by_input.entry(s.id).or_default().push(i);
                }
            }
            index.by_category.entry(r.category()).or_default().push(i);
        }
        for c in Category::ALL {
            let ids: Vec<u32> = c.catalysts().iter().filter_map(|id| catalyst(id)).collect();
            index.catalysts.insert(c, ids);
        }
        index.recipes = recipes;
        index
    }
    pub fn catalysts(&self, c: Category) -> &[u32] {
        self.catalysts.get(&c).map_or(&[], Vec::as_slice)
    }
    pub fn of_category(&self, c: Category) -> &[usize] {
        self.by_category.get(&c).map_or(&[], Vec::as_slice)
    }
    /// The tabs `lookup` shows: each category with its recipes, in JEI's
    /// order; uses of a catalyst start with its categories' every recipe.
    pub fn lookup(&self, lookup: Lookup) -> Vec<(Category, Vec<usize>)> {
        let mut tabs: Vec<(Category, Vec<usize>)> = Vec::new();
        let add = |c: Category, i: usize, tabs: &mut Vec<(Category, Vec<usize>)>| {
            match tabs.iter_mut().find(|(t, _)| *t == c) {
                Some((_, list)) => {
                    if !list.contains(&i) {
                        list.push(i);
                    }
                }
                None => tabs.push((c, vec![i])),
            }
        };
        match lookup {
            Lookup::Recipes(id) => {
                for &i in self.by_output.get(&id).map_or(&[][..], Vec::as_slice) {
                    add(self.recipes[i].category(), i, &mut tabs);
                }
            }
            Lookup::Uses(id) => {
                for c in Category::ALL {
                    if self.catalysts(c).contains(&id) {
                        for &i in self.of_category(c) {
                            add(c, i, &mut tabs);
                        }
                    }
                }
                for &i in self.by_input.get(&id).map_or(&[][..], Vec::as_slice) {
                    add(self.recipes[i].category(), i, &mut tabs);
                }
            }
            // "Show all recipes": every category, this one selected.
            Lookup::Category(_) => {
                for c in Category::ALL {
                    for &i in self.of_category(c) {
                        add(c, i, &mut tabs);
                    }
                }
            }
        }
        tabs.sort_by_key(|(c, _)| c.index());
        tabs
    }
}

// ---- the recipes screen's state ------------------------------------------------

/// The recipes screen: what it shows, the tab and the page, and the
/// lookups before it (Backspace goes back).
#[derive(Clone, Debug, Default)]
pub struct Screen {
    pub lookup: Option<Lookup>,
    pub tabs: Vec<(Category, Vec<usize>)>,
    pub tab: usize,
    pub page: usize,
    pub history: Vec<(Lookup, Category, usize)>,
}

impl Screen {
    pub fn is_open(&self) -> bool {
        self.lookup.is_some()
    }
    /// Shows `lookup`; false (and nothing changes) when it has no recipes.
    /// The screen open before goes on the history.
    pub fn open(&mut self, index: &Index, lookup: Lookup) -> bool {
        let tabs = index.lookup(lookup);
        if tabs.is_empty() {
            return false;
        }
        if self.lookup == Some(lookup) {
            return true;
        }
        if let (Some(old), Some((category, _))) = (self.lookup, self.tabs.get(self.tab)) {
            self.history.push((old, *category, self.page));
        }
        self.lookup = Some(lookup);
        self.tab = match lookup {
            Lookup::Category(c) => tabs.iter().position(|(t, _)| *t == c).unwrap_or(0),
            _ => 0,
        };
        self.tabs = tabs;
        self.page = 0;
        true
    }
    /// The lookup before: false when there was none (the screen closes).
    pub fn back(&mut self, index: &Index) -> bool {
        while let Some((lookup, category, page)) = self.history.pop() {
            let tabs = index.lookup(lookup);
            if tabs.is_empty() {
                continue;
            }
            self.tab = tabs.iter().position(|(c, _)| *c == category).unwrap_or(0);
            self.tabs = tabs;
            self.lookup = Some(lookup);
            self.page = page;
            return true;
        }
        false
    }
    pub fn close(&mut self) {
        *self = Screen::default();
    }
    pub fn category(&self) -> Option<Category> {
        self.tabs.get(self.tab).map(|(c, _)| *c)
    }
    pub fn recipes(&self) -> &[usize] {
        self.tabs.get(self.tab).map_or(&[], |(_, r)| r.as_slice())
    }
    pub fn select_tab(&mut self, tab: usize) {
        if tab < self.tabs.len() && tab != self.tab {
            self.tab = tab;
            self.page = 0;
        }
    }
    pub fn turn_tab(&mut self, delta: i32) {
        let tab = turn(self.tab, delta, self.tabs.len());
        self.select_tab(tab);
    }
    pub fn turn_page(&mut self, delta: i32, per_page: usize) {
        self.page = turn(self.page, delta, pages(self.recipes().len(), per_page));
    }
}

// ---- settings ----------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Settings {
    /// The ingredient list beside container screens (Ctrl+O).
    pub overlay: bool,
    /// Clicks on the list give the item (creative, or `/give` where cheats
    /// are allowed).
    pub cheat: bool,
    pub bookmarks: bool,
    /// New bookmarks go first.
    pub bookmarks_front: bool,
    /// The mod's name at the foot of item tooltips, blue and italic.
    pub mod_name: bool,
    /// The search box sits in the middle of the screen's foot.
    pub center_search: bool,
    /// The tag (#) and list badges on recipe slots with several items.
    pub badges: bool,
    /// How each prefixed source takes part in a search (JEI's Search >
    /// Matching): `@` mod name, `$` tag, `#` tooltip, `&` id, `%` creative tab.
    pub search_mod_name: Mode,
    pub search_tag: Mode,
    pub search_tooltip: Mode,
    pub search_identifier: Mode,
    pub search_creative_tab: Mode,
}

impl Default for Settings {
    fn default() -> Self {
        let m = Modes::default();
        Settings {
            overlay: true,
            cheat: false,
            bookmarks: true,
            bookmarks_front: false,
            mod_name: true,
            center_search: false,
            badges: true,
            search_mod_name: m.mod_name,
            search_tag: m.tag,
            search_tooltip: m.tooltip,
            search_identifier: m.identifier,
            search_creative_tab: m.creative_tab,
        }
    }
}

impl Settings {
    /// The switches of the mod's settings page.
    pub const KEYS: [&'static str; 8] = [
        "overlay",
        "cheat",
        "bookmarks",
        "bookmarks_front",
        "mod_name",
        "tooltip_search",
        "center_search",
        "badges",
    ];
    /// The search modes' keys in the settings file.
    pub const MODE_KEYS: [&'static str; 5] =
        ["search_mod_name", "search_tag", "search_tooltip", "search_identifier", "search_creative_tab"];
    /// Reads the settings, `get` giving one key's stored value.
    pub fn read(get: impl Fn(&str) -> Option<String>) -> Settings {
        let d = Settings::default();
        let flag = |key: &str, default: bool| match get(key).as_deref().map(str::trim) {
            Some("true") => true,
            Some("false") => false,
            _ => default,
        };
        let mut s = Settings {
            overlay: flag("overlay", d.overlay),
            cheat: flag("cheat", d.cheat),
            bookmarks: flag("bookmarks", d.bookmarks),
            bookmarks_front: flag("bookmarks_front", d.bookmarks_front),
            mod_name: flag("mod_name", d.mod_name),
            center_search: flag("center_search", d.center_search),
            badges: flag("badges", d.badges),
            ..d
        };
        // 1.0.0 kept the tooltip's mode as an on/off "tooltip_search".
        s.set("tooltip_search", flag("tooltip_search", d.search_tooltip == Mode::Enabled));
        for key in Settings::MODE_KEYS {
            if let Some(mode) = get(key).as_deref().and_then(Mode::parse) {
                s.set_mode(key, mode);
            }
        }
        s
    }
    pub fn get(&self, key: &str) -> bool {
        match key {
            "overlay" => self.overlay,
            "cheat" => self.cheat,
            "bookmarks" => self.bookmarks,
            "bookmarks_front" => self.bookmarks_front,
            "mod_name" => self.mod_name,
            "tooltip_search" => self.search_tooltip == Mode::Enabled,
            "center_search" => self.center_search,
            "badges" => self.badges,
            _ => false,
        }
    }
    pub fn set(&mut self, key: &str, on: bool) {
        match key {
            "overlay" => self.overlay = on,
            "cheat" => self.cheat = on,
            "bookmarks" => self.bookmarks = on,
            "bookmarks_front" => self.bookmarks_front = on,
            "mod_name" => self.mod_name = on,
            "tooltip_search" => self.search_tooltip = if on { Mode::Enabled } else { Mode::RequirePrefix },
            "center_search" => self.center_search = on,
            "badges" => self.badges = on,
            _ => {}
        }
    }
    /// A search source's mode, by its key in `MODE_KEYS`.
    pub fn mode(&self, key: &str) -> Mode {
        match key {
            "search_mod_name" => self.search_mod_name,
            "search_tag" => self.search_tag,
            "search_tooltip" => self.search_tooltip,
            "search_identifier" => self.search_identifier,
            _ => self.search_creative_tab,
        }
    }
    pub fn set_mode(&mut self, key: &str, mode: Mode) {
        match key {
            "search_mod_name" => self.search_mod_name = mode,
            "search_tag" => self.search_tag = mode,
            "search_tooltip" => self.search_tooltip = mode,
            "search_identifier" => self.search_identifier = mode,
            "search_creative_tab" => self.search_creative_tab = mode,
            _ => {}
        }
    }
    /// Every stored key with its value as the settings file keeps it.
    pub fn stored(&self) -> Vec<(&'static str, &'static str)> {
        let word = |on: bool| if on { "true" } else { "false" };
        let mut out: Vec<(&'static str, &'static str)> = Settings::KEYS.iter().map(|&k| (k, word(self.get(k)))).collect();
        out.extend(Settings::MODE_KEYS.iter().map(|&k| (k, self.mode(k).name())));
        out
    }
    pub fn modes(&self) -> Modes {
        Modes {
            mod_name: self.search_mod_name,
            tooltip: self.search_tooltip,
            tag: self.search_tag,
            creative_tab: self.search_creative_tab,
            identifier: self.search_identifier,
            ..Modes::default()
        }
    }
}

// ---- recipe makers ---------------------------------------------------------------------

/// Where input `i` of a `width` x `height` recipe goes in the 3x3 grid
/// (`CraftingGridHelper.getCraftingIndex`): narrow and short recipes sit in
/// the middle column or row.
pub fn crafting_index(i: usize, width: u32, height: u32) -> usize {
    if width == 1 {
        if height >= 2 { i * 3 + 1 } else { 4 }
    } else if height == 1 {
        i + 3
    } else if width == 2 {
        i + i / 2
    } else if height == 2 {
        i + 3
    } else {
        i
    }
}

/// A crafting recipe's inputs laid on the 3x3 grid, with the grid size used:
/// shaped by its own size, shapeless in the smallest square that holds them.
pub fn crafting_grid(
    inputs: Vec<Vec<Stack>>,
    width: u32,
    height: u32,
    shapeless: bool,
) -> (Vec<Vec<Stack>>, u32, u32) {
    let (w, h) = if shapeless || width == 0 || height == 0 {
        let n = inputs.len();
        let side = if n > 4 {
            3
        } else if n > 1 {
            2
        } else {
            1
        };
        (side, side)
    } else {
        (width.min(3), height.min(3))
    };
    let mut grid = vec![Vec::new(); 9];
    for (i, slot) in inputs.into_iter().enumerate() {
        let at = crafting_index(i, w, h);
        if let Some(cell) = grid.get_mut(at) {
            *cell = slot;
        }
    }
    (grid, w, h)
}

/// The damage left when two worn items of `max` durability are combined:
/// their durability added with a bonus of `bonus_percent` of `max` (12 on an
/// anvil, `AnvilMenu.createResult`; 5 on a grindstone, `mergeItems`).
pub fn combined_damage(max: u32, a: u32, b: u32, bonus_percent: u32) -> u32 {
    let left = (max - a.min(max)) + (max - b.min(max)) + max * bonus_percent / 100;
    max.saturating_sub(left)
}

/// The chance vanilla's named compost providers add a layer
/// (`context_int_provider/compostable/*`: `ComposterBlock.COMPOSTABLES`).
pub fn compost_chance_named(name: &str) -> Option<f32> {
    let name = name.strip_prefix("minecraft:").unwrap_or(name);
    Some(match name.strip_prefix("compostable/")? {
        "low" => 0.3,
        "low_medium" => 0.5,
        "medium" => 0.65,
        "high" => 0.85,
        "always_add_one" => 1.0,
        _ => return None,
    })
}

// ---- keys and labels ------------------------------------------------------------------

/// The mod's keys on Options > Mods: name, label, default key (JEI's
/// `KeyBindings`; Ctrl goes with the overlay and search keys).
pub const KEYS: [(&str, &str, &str); 10] = [
    ("show_recipe", "key_show_recipe", "key.keyboard.r"),
    ("show_uses", "key_show_uses", "key.keyboard.u"),
    ("bookmark", "key_bookmark", "key.keyboard.a"),
    ("recipe_back", "key_recipe_back", "key.keyboard.backspace"),
    ("toggle_overlay", "key_toggle_overlay", "key.keyboard.o"),
    ("focus_search", "key_focus_search", "key.keyboard.f"),
    ("previous_page", "key_previous_page", "key.keyboard.page.up"),
    ("next_page", "key_next_page", "key.keyboard.page.down"),
    ("toggle_cheat", "key_toggle_cheat", "key.keyboard.unknown"),
    ("toggle_bookmarks", "key_toggle_bookmarks", "key.keyboard.unknown"),
];

/// A label in English or Russian (JEI's `en_us` and `ru_ru`; the mod's own
/// settings where JEI has none).
pub fn tr(ru: bool, key: &str) -> &'static str {
    let (en, ru_text) = match key {
        "description" => (
            "Item and recipe viewer: every item beside container screens, its recipes and uses.",
            "Просмотр предметов и рецептов: все предметы рядом с инвентарём, их рецепты и применение.",
        ),
        "modmenu_configure" => ("Configure...", "Настройки..."),
        "on" => ("ON", "ВКЛ"),
        "off" => ("OFF", "ВЫКЛ"),
        "overlay" => ("Ingredient list", "Список ингредиентов"),
        "cheat" => ("Cheat mode", "Режим жульничества"),
        "bookmarks" => ("Bookmarks", "Закладки"),
        "bookmarks_front" => ("Add new bookmarks to the front", "Новые закладки в начало"),
        "mod_name" => ("Mod name in tooltips", "Название мода в подсказках"),
        "tooltip_search" => ("Search tooltips", "Поиск по подсказкам"),
        "center_search" => ("Search bar in the middle", "Строка поиска по центру"),
        "clear_bookmarks" => ("Clear bookmarks", "Очистить закладки"),
        "badges" => ("Tag badges (#) on recipe slots", "Значки тегов (#) на слотах рецептов"),
        "key_show_recipe" => ("Show Recipe", "Показать рецепт"),
        "key_show_uses" => ("Show Uses", "Показать применение"),
        "key_bookmark" => ("Add/Remove Bookmark", "Добавить/удалить закладку"),
        "key_recipe_back" => ("Show Previous Recipe Page", "Предыдущая страница рецептов"),
        "key_toggle_overlay" => ("Show/Hide Item List (Ctrl+)", "Показать/скрыть список (Ctrl+)"),
        "key_focus_search" => ("Select Search Bar (Ctrl+)", "Выбрать строку поиска (Ctrl+)"),
        "key_previous_page" => ("Show Previous Page", "Предыдущая страница"),
        "key_next_page" => ("Show Next Page", "Следующая страница"),
        "key_toggle_cheat" => ("Toggle Cheat Mode", "Режим жульничества"),
        "key_toggle_bookmarks" => ("Show/Hide Bookmarks", "Показать/скрыть закладки"),
        "show_recipes" => ("Show all recipes", "Отобразить рецепты"),
        "show_all_hotkey" => ("CLICK to show all recipes.", "КЛИК — показать все рецепты."),
        "transfer" => ("Move Items", "Переместить предметы"),
        "missing" => ("Missing Items", "Отсутствуют предметы."),
        "no_room" => ("Inventory full", "Инвентарь полон"),
        "no_fit" => ("Recipe does not fit this menu", "Рецепт не подходит к этому окну"),
        "too_large" => (
            "Recipe is too large to fit in the player's inventory crafting grid.",
            "Рецепт слишком большой для сетки крафта в инвентаре.",
        ),
        "shapeless" => ("Shapeless Recipe", "Бесформенный рецепт"),
        "config" => ("JEI Config", "Конфигурация JEI"),
        "cheat_on" => ("Cheat mode enabled.", "Доступный режим жульничества"),
        "cheat_hint" => ("Press \"Ctrl + CLICK\" here to toggle it.", "Ctrl + КЛИК сюда, чтобы вкл/выкл."),
        "overlay_off" => ("JEI overlays are hidden.", "Наложения JEI отключены."),
        "overlay_off_fix" => ("Press \"Ctrl + O\" to show them again.", "Нажми \"Ctrl + O\", чтобы их включить."),
        "bookmarks_enable" => ("Show Bookmarks", "Показать закладки"),
        "bookmarks_disable" => ("Hide Bookmarks", "Скрыть закладки"),
        "bookmarks_usage" => (
            "Hover over an ingredient and press \"A\" to bookmark it.",
            "Наведи курсором на ингредиент и нажми \"A\", чтобы добавить его в закладки.",
        ),
        "any_fuel" => ("Any Fuel", "Любое топливо"),
        "recipe_id" => ("Recipe ID: ", "ID рецепта: "),
        _ => ("", ""),
    };
    if ru { ru_text } else { en }
}

/// A category's title (`gui.jei.category.*`); the anvil's and the smithing
/// table's are the game's own block names, which the glue looks up.
pub fn category_title(ru: bool, c: Category) -> &'static str {
    let (en, ru_text) = match c {
        Category::Crafting => ("Crafting", "Создание"),
        Category::Stonecutting => ("Stonecutting", "Резка камня"),
        Category::Smelting => ("Smelting", "Плавка"),
        Category::Smoking => ("Smoking", "Копчение"),
        Category::Blasting => ("Blasting", "Дутьё"),
        Category::Campfire => ("Campfire Cooking", "Приготовление на костре"),
        Category::SmeltingFuel => ("Smelting Fuel", "Топливо для переплавки"),
        Category::SmokingFuel => ("Smoking Fuel", "Топливо для копчения"),
        Category::BlastingFuel => ("Blasting Fuel", "Топливо для плавки"),
        Category::Brewing => ("Brewing", "Варка"),
        Category::Anvil => ("Anvil", "Наковальня"),
        Category::Grindstone => ("Grinding", "Заточка"),
        Category::Smithing => ("Smithing Table", "Кузнечный стол"),
        Category::Compostable => ("Composting", "Биоразлагаемость"),
    };
    if ru { ru_text } else { en }
}

/// Java's `Float.toString` for the plain numbers JEI prints: `0.1`, `1.0`.
pub fn java_float(v: f32) -> String {
    let s = format!("{v}");
    if s.contains('.') || s.contains('e') || s.contains("inf") || s.contains("NaN") {
        s
    } else {
        format!("{s}.0")
    }
}

/// `gui.jei.category.smelting.time.seconds`: whole seconds, cut down.
pub fn cooking_time(ru: bool, ticks: u32) -> String {
    let s = ticks / 20;
    if ru { format!("{s} секунд") } else { format!("{s}s") }
}

/// `gui.jei.category.smelting.experience`.
pub fn experience(ru: bool, xp: f32) -> String {
    let xp = java_float(xp);
    if ru { format!("Опыт: {xp}") } else { format!("{xp} XP") }
}

/// `gui.jei.category.fuel.smeltCount`: `ticks` of fuel smelt this many items
/// at `per_item` ticks an item (up to two decimals).
pub fn smelt_count(ru: bool, ticks: u32, per_item: u32) -> String {
    let count = f64::from(ticks) / f64::from(per_item.max(1));
    if count == 1.0 {
        return if ru { "Плавит 1 предмет" } else { "Smelts 1 item" }.into();
    }
    let text = format!("{count:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    if ru { format!("Плавит {text} предметов") } else { format!("Smelts {text} items") }
}

/// `gui.jei.category.compostable.chance`.
pub fn compost_chance(ru: bool, chance: f32) -> String {
    let percent = (chance * 100.0).floor() as u32;
    if ru { format!("Вероятность: {percent}%") } else { format!("Chance: {percent}%") }
}

/// `gui.jei.category.grindstone.experience`: `least` to twice that.
pub fn grindstone_xp(ru: bool, least: u32) -> String {
    let most = least * 2;
    if ru { format!("От {least} до {most} опыта") } else { format!("{least} to {most} XP") }
}

/// `gui.jei.category.brewing.steps`.
pub fn brewing_steps_text(ru: bool, steps: u32) -> String {
    let n = if steps == u32::MAX { "?".to_owned() } else { steps.to_string() };
    if ru { format!("Шаги: {n}") } else { format!("Steps: {n}") }
}

/// JEI's brewing steps (`BrewingRecipeUtil`): the water bottle is 0, a potion
/// one more than the fewest steps of a potion brewed into it; a potion nothing
/// reaches is left out. `recipes` are (input potions, output potion).
pub fn brewing_steps(recipes: &[(Vec<u32>, u32)], water: Option<u32>) -> HashMap<u32, u32> {
    let mut steps: HashMap<u32, u32> = HashMap::new();
    if let Some(w) = water {
        steps.insert(w, 0);
    }
    loop {
        let mut changed = false;
        for (inputs, output) in recipes {
            let best = inputs.iter().filter_map(|i| steps.get(i)).min().copied();
            if let Some(b) = best {
                let s = b.saturating_add(1);
                if steps.get(output).is_none_or(|&o| s < o) {
                    steps.insert(*output, s);
                    changed = true;
                }
            }
        }
        if !changed {
            return steps;
        }
    }
}

// ---- formatting --------------------------------------------------------------------

/// A mod's display name from its id when it has no other: `minecraft` as
/// `Minecraft`, `my_mod` as `My Mod`.
pub fn mod_display_name(id: &str) -> String {
    id.split(['_', '-'])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().chain(c).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item<'a>(name: &'a str, id: &'a str, tags: &'a [String], tooltip: Option<&'a str>) -> Searchable<'a> {
        Searchable {
            name,
            mod_name: "minecraft",
            mod_id: id.split(':').next().unwrap_or(""),
            id,
            tags,
            tabs: &[],
            tooltip,
        }
    }

    #[test]
    fn plain_words_must_all_be_in_the_name_or_tooltip() {
        let m = Modes::default();
        let tags = vec!["minecraft:logs".to_owned()];
        let log = item("oak log", "minecraft:oak_log", &tags, Some("building blocks"));
        assert!(Query::parse("oak", &m).matches(&log, &m));
        assert!(Query::parse("OAK LOG", &m).matches(&log, &m));
        assert!(Query::parse("log oak", &m).matches(&log, &m));
        assert!(!Query::parse("oak plank", &m).matches(&log, &m));
        // Tooltips are searched without the prefix by default.
        assert!(Query::parse("building", &m).matches(&log, &m));
        let m2 = Settings { search_tooltip: Mode::RequirePrefix, ..Settings::default() }.modes();
        assert!(!Query::parse("building", &m2).matches(&log, &m2));
        assert!(Query::parse("#building", &m2).matches(&log, &m2));
    }

    #[test]
    fn prefixes_exclusions_alternatives_and_quotes() {
        let m = Modes::default();
        let tags = vec!["minecraft:logs".to_owned()];
        let log = item("oak log", "minecraft:oak_log", &tags, None);
        let none: Vec<String> = Vec::new();
        let stone = item("stone", "minecraft:stone", &none, None);
        assert!(Query::parse("$logs", &m).matches(&log, &m));
        assert!(!Query::parse("$logs", &m).matches(&stone, &m));
        // Tags need their prefix.
        assert!(!Query::parse("logs", &m).matches(&stone, &m));
        assert!(Query::parse("@minecraft", &m).matches(&stone, &m));
        assert!(!Query::parse("@other", &m).matches(&stone, &m));
        assert!(Query::parse("-oak", &m).matches(&stone, &m));
        assert!(!Query::parse("-oak", &m).matches(&log, &m));
        assert!(Query::parse("stone | oak", &m).matches(&log, &m));
        assert!(Query::parse("stone|oak", &m).matches(&stone, &m));
        assert!(Query::parse("\"oak log\"", &m).matches(&log, &m));
        assert!(!Query::parse("\"log oak\"", &m).matches(&log, &m));
        assert!(Query::parse("&oak_log", &m).matches(&log, &m));
        // A lone prefix or dash is no word: everything shows.
        assert!(Query::parse("@", &m).is_empty());
        assert!(Query::parse("  ", &m).is_empty());
        assert!(Query::parse("-", &m).matches(&stone, &m) || Query::parse("-", &m).is_empty());
        // `^` (colour) is off: a plain character in the name.
        assert!(!Query::parse("^red", &m).matches(&stone, &m));
        assert!(Query::parse("#x", &m).needs_tooltips(&m));
        assert!(!Query::parse("@x", &m).needs_tooltips(&m));
    }

    #[test]
    fn pages_wrap_and_slots_cycle() {
        assert!(inside((5.0, 5.0), [0.0, 0.0, 10.0, 10.0]));
        assert!(!inside((10.0, 5.0), [0.0, 0.0, 10.0, 10.0]));
        assert_eq!(pages(0, 10), 1);
        assert_eq!(pages(21, 10), 3);
        assert_eq!(turn(0, -1, 3), 2);
        assert_eq!(turn(2, 1, 3), 0);
        assert_eq!(cycle(3, 2500, 0), 2);
        assert_eq!(cycle(3, 2500, 2), 1);
    }

    #[test]
    fn bookmarks_toggle_and_survive_a_round_trip() {
        let mut b = Bookmarks::default();
        assert!(b.toggle("minecraft:stone", false));
        assert!(b.toggle("minecraft:dirt", true));
        assert_eq!(b.keys, ["minecraft:dirt", "minecraft:stone"]);
        assert!(!b.toggle("minecraft:dirt", true));
        assert_eq!(b.keys, ["minecraft:stone"]);
        b.toggle("minecraft:potion[minecraft:potion_contents={potion:\"minecraft:swiftness\"}]", false);
        let again = Bookmarks::parse(&b.write());
        assert_eq!(again, b);
        assert_eq!(Bookmarks::parse("a\n\na\n# c\nb").keys, ["a", "b"]);
    }

    fn recipe(category: Category, inputs: &[u32], output: u32) -> Recipe {
        Recipe {
            category: Some(category),
            inputs: inputs.iter().map(|&i| vec![Stack::one(i)]).collect(),
            outputs: vec![vec![Stack::one(output)]],
            ..Recipe::default()
        }
    }

    #[test]
    fn lookups_group_by_category_and_catalysts_lead_uses() {
        // 1 log, 2 planks, 3 sticks, 4 crafting table, 5 furnace, 6 charcoal.
        let recipes = vec![
            recipe(Category::Crafting, &[1], 2),
            recipe(Category::Crafting, &[2, 2], 3),
            recipe(Category::Smelting, &[1], 6),
            recipe(Category::Crafting, &[2, 2, 2, 2], 4),
        ];
        let index = Index::new(recipes, |id| match id {
            "minecraft:crafting_table" => Some(4),
            "minecraft:furnace" => Some(5),
            _ => None,
        });
        let uses = index.lookup(Lookup::Uses(1));
        assert_eq!(uses, vec![(Category::Crafting, vec![0]), (Category::Smelting, vec![2])]);
        let table = index.lookup(Lookup::Uses(4));
        assert_eq!(table, vec![(Category::Crafting, vec![0, 1, 3])]);
        assert_eq!(index.lookup(Lookup::Recipes(3)), vec![(Category::Crafting, vec![1])]);
        assert_eq!(index.lookup(Lookup::Uses(5)), vec![(Category::Smelting, vec![2])]);
        assert!(index.lookup(Lookup::Recipes(1)).is_empty());

        let mut screen = Screen::default();
        assert!(!screen.open(&index, Lookup::Recipes(1)));
        assert!(!screen.is_open());
        assert!(screen.open(&index, Lookup::Uses(1)));
        screen.select_tab(1);
        assert_eq!(screen.category(), Some(Category::Smelting));
        assert!(screen.open(&index, Lookup::Recipes(3)));
        assert!(screen.back(&index));
        assert_eq!((screen.lookup, screen.category()), (Some(Lookup::Uses(1)), Some(Category::Smelting)));
        assert!(!screen.back(&index));
        assert!(screen.open(&index, Lookup::Category(Category::Smelting)));
        assert_eq!(screen.recipes(), &[2]);
    }

    #[test]
    fn settings_read_their_files() {
        let s = Settings::read(|k| (k == "cheat").then(|| "true\n".to_owned()));
        assert!(s.cheat && s.overlay && s.mod_name);
        let mut t = s;
        t.set("overlay", false);
        assert!(!t.get("overlay"));
    }

    #[test]
    fn crafting_recipes_sit_on_the_grid_as_in_jei() {
        let s = |i: u32| vec![Stack::one(i)];
        // A stick (1x2) in the middle column; a 2x2 in the top left corner.
        let (grid, w, h) = crafting_grid(vec![s(1), s(1)], 1, 2, false);
        assert_eq!((w, h), (1, 2));
        assert_eq!(grid[1], s(1));
        assert_eq!(grid[4], s(1));
        let (grid, ..) = crafting_grid(vec![s(1), s(2), s(3), s(4)], 2, 2, false);
        assert_eq!([&grid[0], &grid[1], &grid[3], &grid[4]], [&s(1), &s(2), &s(3), &s(4)]);
        // A 3x1 row in the middle row.
        let (grid, ..) = crafting_grid(vec![s(1), s(2), s(3)], 3, 1, false);
        assert_eq!([&grid[3], &grid[4], &grid[5]], [&s(1), &s(2), &s(3)]);
        // Shapeless: one in the middle, three in a 2x2, five in the 3x3.
        let (grid, w, _) = crafting_grid(vec![s(9)], 0, 0, true);
        assert_eq!((w, &grid[4]), (1, &s(9)));
        let (grid, w, _) = crafting_grid(vec![s(1), s(2), s(3)], 0, 0, true);
        assert_eq!((w, &grid[3]), (2, &s(3)));
        let (_, w, _) = crafting_grid(vec![s(1); 5], 0, 0, true);
        assert_eq!(w, 3);
    }

    #[test]
    fn repairs_and_compost() {
        // Two diamond swords worn to a quarter: the anvil's 12% bonus.
        assert_eq!(combined_damage(1561, 1170, 1170, 12), 1561 - (391 + 391 + 187));
        assert_eq!(combined_damage(100, 0, 0, 12), 0);
        assert_eq!(compost_chance_named("minecraft:compostable/low"), Some(0.3));
        assert_eq!(compost_chance_named("minecraft:compostable/always_add_one"), Some(1.0));
        assert_eq!(compost_chance_named("minecraft:cooking/x"), None);
        assert_eq!(compost_chance(false, 0.65), "Chance: 65%");
        assert_eq!(compost_chance(true, 0.3), "Вероятность: 30%");
        assert_eq!(experience(false, 0.1), "0.1 XP");
        assert_eq!(experience(false, 1.0), "1.0 XP");
        assert_eq!(cooking_time(false, 230), "11s");
        assert_eq!(smelt_count(false, 1600, 200), "Smelts 8 items");
        assert_eq!(smelt_count(false, 300, 200), "Smelts 1.5 items");
        assert_eq!(smelt_count(false, 200, 200), "Smelts 1 item");
    }

    #[test]
    fn brewing_steps_count_from_the_water_bottle() {
        // 1 water, 2 awkward, 3 swiftness, 4 long swiftness, 5 unreachable.
        let recipes = vec![(vec![1], 2), (vec![2], 3), (vec![3], 4), (vec![9], 5)];
        let steps = brewing_steps(&recipes, Some(1));
        assert_eq!((steps[&2], steps[&3], steps[&4]), (1, 2, 3));
        assert!(!steps.contains_key(&5));
        assert_eq!(brewing_steps_text(false, u32::MAX), "Steps: ?");
    }

    #[test]
    fn numbers_as_jei_writes_them() {
        assert_eq!(mod_display_name("minecraft"), "Minecraft");
        assert_eq!(mod_display_name("my_mod"), "My Mod");
    }

    fn stacks(ids: &[u32]) -> Vec<Stack> {
        ids.iter().map(|&id| Stack::one(id)).collect()
    }

    /// A sword (1) with Sharpness I..V books (11..15) making swords (21..25).
    fn sharpness() -> Recipe {
        Recipe {
            category: Some(Category::Anvil),
            inputs: vec![stacks(&[1]), stacks(&[11, 12, 13, 14, 15])],
            outputs: vec![stacks(&[21, 22, 23, 24, 25])],
            cost: 1,
            costs: vec![1, 2, 3, 4, 5],
            ..Recipe::default()
        }
    }

    #[test]
    fn anvil_books_and_results_cycle_together() {
        let r = sharpness();
        let link = r.focus_link(None);
        assert_eq!((link.inputs.as_slice(), link.output), (&[1][..], true));
        for second in 0..10 {
            let k = link.index(5, second * 1000);
            assert_eq!(k, second as usize % 5);
        }
        assert_eq!(link.input(&r, 1), r.inputs[1]);
        assert_eq!(link.input(&r, 0), stacks(&[1]));
    }

    #[test]
    fn a_focused_result_or_book_holds_its_partner() {
        let r = sharpness();
        // Recipes for the Sharpness V sword: the level V book, its cost.
        let link = r.focus_link(Some(25));
        assert_eq!(link.output(&r), stacks(&[25]));
        assert_eq!(link.input(&r, 1), stacks(&[15]));
        for second in 0..5 {
            assert_eq!(r.costs[link.index(r.costs.len(), second * 1000)], 5);
        }
        // Uses of the level III book: the Sharpness III sword.
        let link = r.focus_link(Some(13));
        assert_eq!((link.input(&r, 1), link.output(&r)), (stacks(&[13]), stacks(&[23])));
        // The plain sword is in no linked slot: all of them, together.
        let link = r.focus_link(Some(1));
        assert!(link.indexes.is_empty());
        assert_eq!(link.output(&r).len(), 5);
    }

    #[test]
    fn grindstone_keeps_the_enchanted_item_with_the_one_it_gives_back() {
        // Sharpness II off a sword (31) or an axe (32), giving 41 or 42 back.
        let r = Recipe {
            category: Some(Category::Grindstone),
            inputs: vec![stacks(&[31, 32]), Vec::new()],
            outputs: vec![stacks(&[41, 42])],
            render_only_output: true,
            ..Recipe::default()
        };
        let link = r.focus_link(Some(42));
        assert_eq!((link.input(&r, 0), link.output(&r)), (stacks(&[32]), stacks(&[42])));
        assert!(link.input(&r, 1).is_empty());
        let link = r.focus_link(Some(31));
        assert_eq!(link.output(&r), stacks(&[41]));
    }

    #[test]
    fn only_the_anvil_and_grindstone_link_their_slots() {
        let crafting = Recipe {
            category: Some(Category::Crafting),
            inputs: vec![stacks(&[1, 2])],
            outputs: vec![stacks(&[3, 4])],
            ..Recipe::default()
        };
        assert_eq!(crafting.focus_link(Some(4)), FocusLink::default());
        // Slots of other sizes stay apart, as JEI only links equal lists.
        let mut uneven = sharpness();
        uneven.inputs[1] = stacks(&[11, 12]);
        let link = uneven.focus_link(Some(25));
        assert_eq!(link, FocusLink::default());
        assert_eq!(link.input(&uneven, 1), stacks(&[11, 12]));
    }
}
