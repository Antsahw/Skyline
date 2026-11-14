// Just Enough Items for Horizon: the game's side. The game's items and
// recipes read into `justenoughitems`'s model, the player's mouse and keys on
// container screens, and what they change in the game (giving items, moving a
// recipe into the crafting grid). `justenoughitems_view` draws it.
//
// The screen hooks only read the game (`&Game`): what needs to change it waits
// in `State::pending` for the next frame. Hooks the game fires while this mod
// is drawing (item tooltips) read only the `Cell`s below, never `STATE`.
use super::justenoughitems::{
    self as jei, Bookmarks, Category, Index, Lookup, Query, Recipe, Settings, Stack,
};
use super::justenoughitems_options::Opened;
use super::{Game, Screen};
use horizon_client::clipboard::Clipboard as _;
use horizon_client::items::ItemStack;
use horizon_client::mod_api::NativeScreenView;
use horizon_client::ui::EditBox;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// GLFW's modifier bits.
const SHIFT: u32 = 1;
const CONTROL: u32 = 2;
/// The search box takes at most this many characters (JEI's 128).
const SEARCH_MAX: usize = 128;

/// One ingredient: an item with its components (count and damage aside).
pub(super) struct Ingredient {
    pub(super) stack: ItemStack,
    /// The item as `/give` writes it: `minecraft:potion[...]`.
    pub(super) key: String,
    pub(super) namespace: String,
    lower_name: String,
    lower_id: String,
    lower_mod: String,
    tags: Vec<String>,
    tabs: Vec<String>,
}

/// The game's items and recipes as JEI shows them, made once a world (again
/// when the recipes or the language change).
pub(super) struct Data {
    language: String,
    recipes_at: usize,
    pub(super) ingredients: Vec<Ingredient>,
    by_key: HashMap<String, u32>,
    /// The ingredient list's order: the creative inventory's search tab, then
    /// the items no tab shows.
    pub(super) list: Vec<u32>,
    pub(super) index: Index,
    /// Each ingredient's tooltip below its name, lowercased; made the first
    /// time a search reads tooltips.
    tooltips: Option<Vec<String>>,
}

impl Ingredient {
    /// Its item tags, `minecraft:logs`.
    pub(super) fn tags(&self) -> &[String] {
        &self.tags
    }
}

impl Data {
    pub(super) fn ingredient(&self, id: u32) -> Option<&Ingredient> {
        self.ingredients.get(id as usize)
    }
    pub(super) fn id_of(&self, stack: &ItemStack) -> Option<u32> {
        self.by_key.get(&stack_key(stack)).copied()
    }
    pub(super) fn ingredient_by_key(&self, key: &str) -> Option<u32> {
        self.by_key.get(key).copied()
    }
    /// A recipe stack as the game draws it.
    pub(super) fn stack(&self, s: Stack) -> Option<ItemStack> {
        let mut stack = self.ingredient(s.id)?.stack.clone();
        stack.count = s.count.max(1);
        stack.damage = s.damage;
        Some(stack)
    }
}

/// What a part of the overlays is, for the clicks and keys over it.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(super) enum Hit {
    /// An ingredient of the list, the bookmarks, a recipe or a catalyst.
    List(u32),
    Bookmark(u32),
    Slot(u32),
    Catalyst(u32),
    ListPage(i32),
    BookmarkPage(i32),
    /// Shows or hides the bookmarks.
    BookmarkButton,
    Search,
    Config,
    /// The recipes screen: a tab, the tab arrows, the category title and
    /// its arrows, the page arrows, a recipe's "+" button.
    Tab(usize),
    TabPage(i32),
    Title,
    CategoryTurn(i32),
    RecipePage(i32),
    Transfer(usize),
    /// Somewhere else on an overlay: the click goes nowhere.
    Area,
}

/// What changes the game, done at the next frame.
#[derive(Clone, Debug)]
pub(super) enum Action {
    Give { id: u32, count: u32 },
    Transfer { recipe: usize, all: bool },
    /// R, U or A over a slot of the screen's.
    SlotKey { key: SlotKey, at: (f32, f32) },
    Paste,
    /// Ctrl+C or Ctrl+X: the selection, taken when the key is pressed (a cut
    /// deletes it at once).
    Copy(String),
    OpenSettings,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum SlotKey {
    Recipes,
    Uses,
    Bookmark,
}

#[derive(Default)]
pub(super) struct State {
    pub(super) data: Option<Data>,
    pub(super) settings: Option<Settings>,
    pub(super) bookmarks: Option<Bookmarks>,
    pub(super) search: Option<EditBox>,
    /// The list as the search leaves it; None when it needs working out.
    pub(super) filtered: Option<Vec<u32>>,
    filtered_for: String,
    pub(super) list_page: usize,
    pub(super) bookmark_page: usize,
    pub(super) recipes: jei::Screen,
    /// The recipes shown per page the last frame, for the page keys.
    pub(super) per_page: usize,
    /// The cells of a page of the ingredient list and of the bookmarks the
    /// last frame (a page's room, not what the last page holds).
    pub(super) list_per: usize,
    pub(super) bookmark_per: usize,
    /// The first tab of the tab row shown.
    pub(super) tab_scroll: usize,
    /// The parts drawn the last frame, for the mouse.
    pub(super) hits: Vec<(jei::Rect, Hit)>,
    pub(super) pending: Vec<Action>,
    pub(super) started: Option<Instant>,
    /// Buttons whose press an overlay took, so their release is taken too.
    pressed: [bool; 8],
    /// A line shown over the overlays for a moment (a give that failed).
    pub(super) note: Option<(String, Instant)>,
    /// The language the labels are in.
    pub(super) ru: bool,
    /// The search text the list's page was last kept for.
    paged_for: String,
}

impl State {
    /// True once each time the search text changes (the list goes back to
    /// its first page).
    pub(super) fn filtered_text_changed(&mut self) -> bool {
        let text = self.search.as_ref().map_or("", |s| s.value.as_str());
        if text == self.paged_for {
            return false;
        }
        self.paged_for = text.to_owned();
        true
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
    /// The mod name in tooltips; read by the tooltip hook, which the game
    /// fires while this mod draws.
    static MOD_NAME: Cell<bool> = const { Cell::new(true) };
    /// This mod is drawing a tooltip of its own.
    pub(super) static OWN_TOOLTIP: Cell<bool> = const { Cell::new(false) };
    /// The recipes screen is open: the game's own tooltips under it stay hidden.
    pub(super) static COVERED: Cell<bool> = const { Cell::new(false) };
}

/// Runs `f` on the state; None while it is in use (a hook fired from inside
/// this mod's own call into the game).
pub(super) fn with<R>(f: impl FnOnce(&mut State) -> R) -> Option<R> {
    STATE.with(|s| s.try_borrow_mut().ok().map(|mut s| f(&mut s)))
}

pub(super) fn on(game: &Game<'_>) -> bool {
    game.native_on(jei::ID)
}

/// The item as `/give` writes it, damage and count aside.
pub(super) fn stack_key(stack: &ItemStack) -> String {
    let mut plain = stack.clone();
    plain.count = 1;
    plain.damage = 0;
    format!("{}{}", plain.item, super::native_recipes::components(&plain))
}

/// The stack `/give` syntax names: `minecraft:potion[...]`.
fn stack_of(key: &str) -> Option<ItemStack> {
    let (item, components) = match key.find('[') {
        Some(i) => (&key[..i], &key[i..]),
        None => (key, ""),
    };
    if item.is_empty() {
        return None;
    }
    let (damage, extra) = if components.is_empty() {
        (0, None)
    } else {
        horizon_client::item_data::command_components(components).map_or((0, None), |tag| {
            horizon_client::item_data::decode_components(Some(&tag))
        })
    };
    Some(ItemStack {
        item: item.into(),
        count: 1,
        damage,
        extra,
    })
}

// ---- settings and bookmarks --------------------------------------------------------

pub(super) fn settings(game: &Game<'_>, state: &mut State) -> Settings {
    if state.settings.is_none() {
        let s = Settings::read(|k| game.native_setting(jei::ID, k));
        MOD_NAME.with(|c| c.set(s.mod_name));
        state.settings = Some(s);
    }
    state.settings.unwrap_or_default()
}

fn set_setting(game: &Game<'_>, state: &mut State, key: &str, on: bool) {
    let mut s = settings(game, state);
    s.set(key, on);
    set_settings(game, state, s);
}

/// Keeps new settings (the config screen's, a switch's): every key whose
/// value changed is written.
pub(super) fn set_settings(game: &Game<'_>, state: &mut State, new: Settings) {
    let old = settings(game, state).stored();
    for (key, value) in new.stored() {
        if !old.contains(&(key, value)) {
            game.native_set_setting(jei::ID, key, value);
        }
    }
    MOD_NAME.with(|c| c.set(new.mod_name));
    state.settings = Some(new);
    state.filtered = None;
}

fn bookmarks_file() -> Option<std::path::PathBuf> {
    Some(Game::native_config_dir().ok()?.join(jei::ID).join("bookmarks.txt"))
}

pub(super) fn bookmarks(state: &mut State) -> &mut Bookmarks {
    state.bookmarks.get_or_insert_with(|| {
        bookmarks_file()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .map(|t| Bookmarks::parse(&t))
            .unwrap_or_default()
    })
}

fn save_bookmarks(state: &mut State) {
    let text = bookmarks(state).write();
    if let Some(file) = bookmarks_file() {
        if let Some(dir) = file.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(file, text);
    }
}

fn toggle_bookmark(game: &Game<'_>, state: &mut State, id: u32) {
    let Some(key) = state
        .data
        .as_ref()
        .and_then(|d| d.ingredient(id))
        .map(|i| i.key.clone())
    else {
        return;
    };
    let front = settings(game, state).bookmarks_front;
    bookmarks(state).toggle(&key, front);
    save_bookmarks(state);
}

// ---- the game's items and recipes ---------------------------------------------------

/// Makes the model once a world is open, and again when the game's recipe
/// tables or the language change; drops it outside a world.
fn sync(game: &mut Game<'_>) {
    if game.player.is_none() {
        with(|s| {
            s.data = None;
            s.recipes.close();
            s.filtered = None;
        });
        return;
    }
    let Some(recipes) = game.native_recipes() else {
        return;
    };
    let language = horizon_client::lang::code();
    let at = Arc::as_ptr(&recipes) as usize;
    let fresh = with(|s| {
        s.data
            .as_ref()
            .is_some_and(|d| d.recipes_at == at && d.language == language)
    })
    .unwrap_or(true);
    if fresh {
        return;
    }
    let data = build(game, &recipes, language, at);
    with(|s| {
        s.data = Some(data);
        s.recipes.close();
        s.filtered = None;
        s.ru = jei::russian(&horizon_client::lang::code());
    });
}

/// The creative tab titles each stack shows under, by key, lowercased.
fn tab_titles(game: &Game<'_>) -> HashMap<String, Vec<String>> {
    let mut out: HashMap<String, Vec<String>> = HashMap::new();
    for tab in &game.creative_screen.tabs {
        if tab.search || tab.inventory || tab.hotbar {
            continue;
        }
        let title = game.creative_tab_title(tab).to_lowercase();
        for stack in &tab.items {
            let list = out.entry(stack_key(stack)).or_default();
            if !list.contains(&title) {
                list.push(title.clone());
            }
        }
    }
    out
}

struct Builder<'a, 'g> {
    game: &'a Game<'g>,
    ingredients: Vec<Ingredient>,
    by_key: HashMap<String, u32>,
    tags: HashMap<String, Vec<String>>,
    tabs: HashMap<String, Vec<String>>,
}

impl Builder<'_, '_> {
    /// The ingredient of `stack`, made when new.
    fn id(&mut self, stack: &ItemStack) -> u32 {
        let key = stack_key(stack);
        if let Some(&id) = self.by_key.get(&key) {
            return id;
        }
        let mut plain = stack.clone();
        plain.count = 1;
        plain.damage = 0;
        let names = &self.game.pack.names;
        let name = horizon_client::item_tooltip::name_line(&plain, &|k| names.display_get(k));
        let id_text = plain.item.to_string();
        let namespace = id_text.split(':').next().unwrap_or("minecraft").to_owned();
        let tabs = self
            .tabs
            .get(&key)
            .or_else(|| self.tabs.get(&id_text))
            .cloned()
            .unwrap_or_default();
        let ingredient = Ingredient {
            lower_name: name.to_lowercase(),
            lower_id: id_text.to_lowercase(),
            lower_mod: jei::mod_display_name(&namespace).to_lowercase(),
            tags: self.tags.get(&id_text).cloned().unwrap_or_default(),
            tabs,
            stack: plain,
            key: key.clone(),
            namespace,
        };
        let id = self.ingredients.len() as u32;
        self.ingredients.push(ingredient);
        self.by_key.insert(key, id);
        id
    }
}

fn build(
    game: &Game<'_>,
    recipes: &super::native_hooks::NativeRecipes,
    language: String,
    at: usize,
) -> Data {
    let mut tags = HashMap::new();
    for item in &recipes.items {
        tags.insert(item.id.clone(), item.tags.clone());
    }
    let mut b = Builder {
        game,
        ingredients: Vec::new(),
        by_key: HashMap::new(),
        tags,
        tabs: tab_titles(game),
    };
    let mut list = Vec::new();
    let mut listed = std::collections::HashSet::new();
    let mut push = |b: &mut Builder<'_, '_>, stack: &ItemStack, list: &mut Vec<u32>| {
        if &*stack.item == "minecraft:air" {
            return;
        }
        let id = b.id(stack);
        if listed.insert(id) {
            list.push(id);
        }
    };
    let search: Vec<ItemStack> = game.creative_screen.search_items().to_vec();
    for stack in &search {
        push(&mut b, stack, &mut list);
    }
    for item in &recipes.items {
        if game.res.items().get(&item.id).is_some() {
            push(&mut b, &ItemStack::new(&item.id, 1), &mut list);
        }
    }

    let mut out: Vec<Recipe> = Vec::new();
    let native_stack = |b: &mut Builder<'_, '_>, s: &super::native_hooks::NativeRecipeStack| {
        let id = recipes.items.get(s.item as usize)?.id.clone();
        let mut key = id;
        key.push_str(&s.components);
        let stack = stack_of(&key)?;
        Some(Stack {
            id: b.id(&stack),
            count: s.count.max(1),
            damage: stack.damage,
        })
    };
    use super::native_hooks::NativeRecipeKind as K;
    for r in &recipes.recipes {
        if r.slots.iter().all(Vec::is_empty) || r.results.is_empty() {
            continue;
        }
        let category = match r.kind {
            K::CraftingShaped | K::CraftingShapeless => Category::Crafting,
            K::Smelting => Category::Smelting,
            K::Blasting => Category::Blasting,
            K::Smoking => Category::Smoking,
            K::Campfire => Category::Campfire,
            K::Stonecutting => Category::Stonecutting,
            K::Smithing => Category::Smithing,
            K::Brewing => Category::Brewing,
        };
        let mut inputs: Vec<Vec<Stack>> = r
            .slots
            .iter()
            .map(|slot| slot.iter().filter_map(|s| native_stack(&mut b, s)).collect())
            .collect();
        let shapeless = r.kind == K::CraftingShapeless;
        let (mut width, mut height) = (r.width, r.height);
        if category == Category::Crafting {
            (inputs, width, height) = jei::crafting_grid(inputs, width, height, shapeless);
        }
        let outputs = vec![r.results.iter().filter_map(|s| native_stack(&mut b, s)).collect()];
        out.push(Recipe {
            category: Some(category),
            id: r.id.clone(),
            inputs,
            outputs,
            width,
            height,
            shapeless,
            ticks: r.cooking_time,
            experience: r.experience,
            book_id: r.book_id,
            ..Recipe::default()
        });
    }
    extra_recipes(game, &mut b, &list, &mut out);
    let listed: Vec<ItemStack> = list.iter().map(|&id| b.ingredients[id as usize].stack.clone()).collect();
    made_up_recipes(game, &mut b, &listed, &mut out);
    let catalysts: HashMap<String, u32> = Category::ALL
        .iter()
        .flat_map(|c| c.catalysts().iter())
        .filter(|id| game.res.items().get(id).is_some())
        .map(|id| (id.to_string(), b.id(&ItemStack::new(id, 1))))
        .collect();
    let index = Index::new(out, |id| catalysts.get(id).copied());
    Data {
        language,
        recipes_at: at,
        ingredients: b.ingredients,
        by_key: b.by_key,
        list,
        index,
        tooltips: None,
    }
}

/// A stack `/give` syntax names, kept only when its components were read
/// (a key the game can't read would make a plain item of it).
fn made(key: &str) -> Option<ItemStack> {
    let stack = stack_of(key)?;
    (key.contains('[') == stack.extra.is_some()).then_some(stack)
}

/// The recipes JEI writes itself for vanilla's special crafting
/// (`TippedArrowRecipeMaker`, `ShieldDecorationRecipeMaker`, the firework
/// rocket extension), the anvil's enchanted books (`AnvilRecipeMaker`) and the
/// grindstone's disenchanting (`GrindstoneRecipeMaker`).
fn made_up_recipes(game: &Game<'_>, b: &mut Builder<'_, '_>, listed: &[ItemStack], out: &mut Vec<Recipe>) {
    let items = game.res.items();
    let e = game.enchantments();
    let one = |b: &mut Builder<'_, '_>, s: &ItemStack| Stack::one(b.id(s));
    let has = |id: &str| items.get(id).is_some();
    // Tipped arrows: eight arrows round a lingering potion.
    if has("minecraft:arrow") && has("minecraft:tipped_arrow") {
        let arrow = one(b, &ItemStack::new("minecraft:arrow", 1));
        for potion in listed.iter().filter(|s| &*s.item == "minecraft:lingering_potion" && s.extra.is_some()) {
            let key = stack_key(potion);
            let components = &key["minecraft:lingering_potion".len()..];
            let Some(tipped) = made(&format!("minecraft:tipped_arrow{components}")) else {
                continue;
            };
            let p = one(b, potion);
            let mut inputs = vec![vec![arrow]; 9];
            inputs[4] = vec![p];
            out.push(Recipe {
                category: Some(Category::Crafting),
                id: "minecraft:tipped_arrow".into(),
                inputs,
                outputs: vec![vec![Stack { id: b.id(&tipped), count: 8, damage: 0 }]],
                width: 3,
                height: 3,
                book_id: -1,
                ..Recipe::default()
            });
        }
    }
    // Shields with a banner's pattern and colour.
    if has("minecraft:shield") {
        let shield = one(b, &ItemStack::new("minecraft:shield", 1));
        for banner in listed.iter().filter(|s| s.item.ends_with("_banner") && s.extra.is_none()) {
            let colour = banner.item.trim_start_matches("minecraft:").trim_end_matches("_banner");
            let Some(decorated) = made(&format!("minecraft:shield[minecraft:base_color=\"{colour}\"]")) else {
                continue;
            };
            let (inputs, w, h) = jei::crafting_grid(vec![vec![shield], vec![one(b, banner)]], 0, 0, true);
            out.push(Recipe {
                category: Some(Category::Crafting),
                id: "minecraft:shield_decoration".into(),
                inputs,
                outputs: vec![vec![Stack::one(b.id(&decorated))]],
                width: w,
                height: h,
                shapeless: true,
                book_id: -1,
                ..Recipe::default()
            });
        }
    }
    // Firework rockets: paper and one to three gunpowder.
    if has("minecraft:paper") && has("minecraft:gunpowder") {
        let paper = one(b, &ItemStack::new("minecraft:paper", 1));
        let powder = one(b, &ItemStack::new("minecraft:gunpowder", 1));
        for flight in 1..=3 {
            let Some(rocket) = made(&format!("minecraft:firework_rocket[minecraft:fireworks={{flight_duration:{flight}}}]")) else {
                continue;
            };
            let mut slots = vec![vec![paper]];
            slots.extend(std::iter::repeat_n(vec![powder], flight));
            let (inputs, w, h) = jei::crafting_grid(slots, 0, 0, true);
            out.push(Recipe {
                category: Some(Category::Crafting),
                id: "minecraft:firework_rocket".into(),
                inputs,
                outputs: vec![vec![Stack { id: b.id(&rocket), count: 3, damage: 0 }]],
                width: w,
                height: h,
                shapeless: true,
                book_id: -1,
                ..Recipe::default()
            });
        }
    }
    // Enchantable items: those with the component and no enchantment yet.
    let enchantable: Vec<&ItemStack> = listed
        .iter()
        .filter(|s| {
            s.extra.is_none()
                && e.enchantable(&s.item) > 0
                && !matches!(&*s.item, "minecraft:book" | "minecraft:enchanted_book")
        })
        .collect();
    // Anvil: each item with each book it takes, every level (a book's cost
    // is half its anvil cost, at least 1, a level).
    for stack in &enchantable {
        let left = one(b, stack);
        for ench in &e.list {
            if !e.can_enchant(items, &ench.id, &stack.item) {
                continue;
            }
            let (mut books, mut outs, mut costs) = (Vec::new(), Vec::new(), Vec::new());
            for level in 1..=ench.max_level {
                let id = &ench.id;
                let (Some(book), Some(result)) = (
                    made(&format!("minecraft:enchanted_book[minecraft:stored_enchantments={{\"{id}\":{level}}}]")),
                    made(&format!("{}[minecraft:enchantments={{\"{id}\":{level}}}]", stack.item)),
                ) else {
                    continue;
                };
                books.push(one(b, &book));
                outs.push(one(b, &result));
                costs.push((ench.anvil_cost / 2).max(1) * level);
            }
            if books.is_empty() {
                continue;
            }
            out.push(Recipe {
                category: Some(Category::Anvil),
                inputs: vec![vec![left], books],
                outputs: vec![outs],
                cost: costs[0],
                costs,
                book_id: -1,
                ..Recipe::default()
            });
        }
    }
    // Grindstone: each enchantment and level taken off every item it goes on;
    // the experience back is half the enchantment's least cost, rounded up,
    // to twice that.
    for ench in &e.list {
        if e.is_curse(&ench.id) {
            continue;
        }
        for level in 1..=ench.max_level.min(10) {
            let (mut tops, mut outs) = (Vec::new(), Vec::new());
            for stack in &enchantable {
                if !e.can_enchant(items, &ench.id, &stack.item) {
                    continue;
                }
                let id = &ench.id;
                let Some(enchanted) = made(&format!("{}[minecraft:enchantments={{\"{id}\":{level}}}]", stack.item)) else {
                    continue;
                };
                tops.push(one(b, &enchanted));
                outs.push(one(b, stack));
            }
            if tops.is_empty() {
                continue;
            }
            let xp = ench.min_cost.calculate(level).max(0) as u32;
            out.push(Recipe {
                category: Some(Category::Grindstone),
                inputs: vec![tops, Vec::new()],
                outputs: vec![outs],
                cost: xp.div_ceil(2),
                book_id: -1,
                render_only_output: true,
                ..Recipe::default()
            });
        }
    }
}

/// The recipes JEI makes up from the game's tables (`FuelRecipeMaker`,
/// `CompostingRecipeMaker`, `AnvilRecipeMaker`, `GrindstoneRecipeMaker`), the
/// furnaces' "any fuel" slot and the brewing steps.
fn extra_recipes(game: &Game<'_>, b: &mut Builder<'_, '_>, list: &[u32], out: &mut Vec<Recipe>) {
    let items = game.res.items();
    let providers = game.res.providers();
    let listed: Vec<ItemStack> = list
        .iter()
        .map(|&id| b.ingredients[id as usize].stack.clone())
        .collect();
    // Fuels: one recipe per item, the shortest burn first; an item smelts in
    // 200 ticks divided by the fuel's speed.
    let mut any_fuel: Vec<Stack> = Vec::new();
    for (category, fast) in [
        (Category::SmeltingFuel, false),
        (Category::SmokingFuel, true),
        (Category::BlastingFuel, true),
    ] {
        let mut fuels: Vec<(u32, u32, u32)> = Vec::new();
        for stack in &listed {
            if items.get(&stack.item).is_none_or(|i| i.fuel.is_none()) {
                continue;
            }
            let ticks = providers.burn_time(items, stack, fast).max(0) as u32;
            if ticks == 0 {
                continue;
            }
            let speed = providers.speed(items, stack, fast);
            let item_ticks = if speed <= 0.0 { 200 } else { ((200.0 / speed).ceil() as u32).max(1) };
            let id = b.id(stack);
            if category == Category::SmeltingFuel {
                any_fuel.push(Stack::one(id));
            }
            fuels.push((ticks, item_ticks, id));
        }
        fuels.sort_by_key(|f| f.0);
        for (ticks, item_ticks, id) in fuels {
            out.push(Recipe {
                category: Some(category),
                inputs: vec![vec![Stack::one(id)]],
                ticks,
                item_ticks,
                book_id: -1,
                ..Recipe::default()
            });
        }
    }
    for r in out.iter_mut() {
        if matches!(r.category, Some(Category::Smelting | Category::Smoking | Category::Blasting)) {
            r.fuel = any_fuel.clone();
        }
    }
    // Compostables: the least likely first.
    let mut compost: Vec<(f32, u32)> = Vec::new();
    for stack in &listed {
        let Some(layers) = items.get(&stack.item).and_then(|i| i.compostable.as_ref()) else {
            continue;
        };
        let chance = compost_chance(layers);
        if chance > 0.0 {
            compost.push((chance, b.id(stack)));
        }
    }
    compost.sort_by(|a, b| a.0.total_cmp(&b.0));
    for (chance, id) in compost {
        out.push(Recipe {
            category: Some(Category::Compostable),
            inputs: vec![vec![Stack::one(id)]],
            chance,
            book_id: -1,
            ..Recipe::default()
        });
    }
    // Anvil: two worn ones joined (a quarter left each, 12% bonus), and a
    // broken one mended by a quarter with its material; the grindstone joins
    // two worn ones with a 5% bonus, its result only drawn.
    let e = game.enchantments();
    let mut grind = Vec::new();
    for stack in &listed {
        let max = items.get(&stack.item).map_or(0, |i| i.max_damage);
        if max == 0 || stack.extra.is_some() {
            continue;
        }
        let id = b.id(stack);
        let worn = |damage: u32| Stack { id, count: 1, damage };
        let quarter = max * 3 / 4;
        out.push(Recipe {
            category: Some(Category::Anvil),
            inputs: vec![vec![worn(quarter)], vec![worn(quarter)]],
            outputs: vec![vec![worn(jei::combined_damage(max, quarter, quarter, 12))]],
            cost: 2,
            book_id: -1,
            ..Recipe::default()
        });
        let materials: Vec<Stack> = listed
            .iter()
            .filter(|m| m.extra.is_none() && horizon_client::stations::is_valid_repair_item(e, items, stack, m))
            .map(|m| Stack::one(b.id(m)))
            .collect();
        if !materials.is_empty() {
            out.push(Recipe {
                category: Some(Category::Anvil),
                inputs: vec![vec![worn(max)], materials],
                outputs: vec![vec![worn(max - max / 4)]],
                cost: 1,
                book_id: -1,
                ..Recipe::default()
            });
        }
        grind.push(Recipe {
            category: Some(Category::Grindstone),
            inputs: vec![vec![worn(quarter)], vec![worn(quarter)]],
            outputs: vec![vec![worn(jei::combined_damage(max, quarter, quarter, 5))]],
            book_id: -1,
            render_only_output: true,
            ..Recipe::default()
        });
    }
    out.extend(grind);
    // Brewing: the steps from a water bottle, the fewest first.
    let water = b
        .ingredients
        .iter()
        .position(|i| &*i.stack.item == "minecraft:potion" && i.key.contains("\"minecraft:water\""))
        .map(|i| i as u32);
    let brews: Vec<(Vec<u32>, u32)> = out
        .iter()
        .filter(|r| r.category == Some(Category::Brewing))
        .filter_map(|r| {
            let output = r.outputs.first()?.first()?.id;
            Some((r.inputs.first()?.iter().map(|s| s.id).collect(), output))
        })
        .collect();
    let steps = jei::brewing_steps(&brews, water);
    for r in out.iter_mut().filter(|r| r.category == Some(Category::Brewing)) {
        r.steps = r
            .outputs
            .first()
            .and_then(|o| o.first())
            .and_then(|s| steps.get(&s.id))
            .copied()
            .unwrap_or(u32::MAX);
    }
    out.sort_by_key(|r| {
        if r.category == Some(Category::Brewing) {
            u64::from(r.steps)
        } else {
            0
        }
    });
}

/// The chance a compostable item adds a layer: its `layers` provider, a
/// named one of vanilla's, a number, or a weighted list (inside a dispatcher).
fn compost_chance(layers: &serde_json::Value) -> f32 {
    use serde_json::Value;
    match layers {
        Value::String(name) => jei::compost_chance_named(name).unwrap_or(0.0),
        Value::Number(n) => {
            if n.as_f64().unwrap_or(0.0) >= 1.0 {
                1.0
            } else {
                0.0
            }
        }
        Value::Object(o) => match o.get("type").and_then(Value::as_str) {
            Some("minecraft:number_dispatcher") => {
                o.get("default").map_or(0.0, compost_chance)
            }
            Some("minecraft:weighted_list") => {
                let list = o.get("distribution").and_then(Value::as_array);
                let (mut hit, mut all) = (0.0, 0.0);
                for e in list.into_iter().flatten() {
                    let w = e.get("weight").and_then(Value::as_f64).unwrap_or(0.0);
                    let d = e.get("data").and_then(Value::as_f64).unwrap_or(0.0);
                    all += w;
                    if d >= 1.0 {
                        hit += w;
                    }
                }
                if all > 0.0 { (hit / all) as f32 } else { 0.0 }
            }
            _ => 0.0,
        },
        _ => 0.0,
    }
}

/// Each ingredient's tooltip lines below the name, lowercased.
fn tooltips(game: &Game<'_>, data: &Data) -> Vec<String> {
    let mut listed = vec![false; data.ingredients.len()];
    for &id in &data.list {
        listed[id as usize] = true;
    }
    data.ingredients
        .iter()
        .zip(listed)
        .map(|(i, listed)| {
            if !listed {
                return String::new();
            }
            game.stack_tooltip(&i.stack)
                .iter()
                .skip(1)
                .map(|(t, _)| t.plain().to_lowercase())
                .collect::<Vec<_>>()
                .join("\n")
        })
        .collect()
}

/// The list as the search box's text leaves it.
pub(super) fn filtered(game: &Game<'_>, state: &mut State) -> Vec<u32> {
    let text = state.search.as_ref().map_or("", |s| s.value.as_str()).to_owned();
    if let Some(f) = &state.filtered
        && state.filtered_for == text
    {
        return f.clone();
    }
    let modes = settings(game, state).modes();
    let query = Query::parse(&text, &modes);
    let Some(data) = state.data.as_mut() else {
        return Vec::new();
    };
    if query.needs_tooltips(&modes) && data.tooltips.is_none() {
        // Tooltips call back into the tooltip hook, which reads only cells.
        data.tooltips = Some(tooltips(game, data));
    }
    let out: Vec<u32> = data
        .list
        .iter()
        .copied()
        .filter(|&id| {
            let i = &data.ingredients[id as usize];
            let s = jei::Searchable {
                name: &i.lower_name,
                mod_name: &i.lower_mod,
                mod_id: &i.namespace,
                id: &i.lower_id,
                tags: &i.tags,
                tabs: &i.tabs,
                tooltip: data
                    .tooltips
                    .as_ref()
                    .and_then(|t| t.get(id as usize))
                    .map(String::as_str),
            };
            query.matches(&s, &modes)
        })
        .collect();
    state.filtered_for = text;
    state.filtered = Some(out.clone());
    out
}

// ---- hooks ---------------------------------------------------------------------------

/// Once a frame (`frame`): keeps the model and does what the screen hooks asked.
pub(super) fn frame(game: &mut Game<'_>) {
    if !on(game) {
        COVERED.with(|c| c.set(false));
        return;
    }
    sync(game);
    let open = matches!(game.screen, Screen::Inventory | Screen::Creative) && game.title.is_none();
    if !open {
        with(|s| {
            s.recipes.close();
            if let Some(search) = s.search.as_mut() {
                search.focused = false;
            }
        });
    }
    COVERED.with(|c| c.set(open && with(|s| s.recipes.is_open()).unwrap_or(false)));
    let actions = with(|s| std::mem::take(&mut s.pending)).unwrap_or_default();
    for action in actions {
        act(game, action);
    }
}

fn act(game: &mut Game<'_>, action: Action) {
    match action {
        Action::Give { id, count } => give(game, id, count),
        Action::Transfer { recipe, all } => transfer(game, recipe, all),
        Action::SlotKey { key, at } => slot_key(game, key, at),
        Action::Paste => {
            let text = game.clipboard.get();
            with(|s| {
                if let Some(search) = s.search.as_mut() {
                    let text: String = text.chars().filter(|c| !c.is_control()).collect();
                    search.insert_text(&text);
                }
            });
        }
        Action::Copy(text) => game.clipboard.set(&text),
        // The config button: JEI's config screen, over the Options screen
        // opened on JEI's page; closing it goes back to the container screen
        // (Java's config screen has it as its parent).
        Action::OpenSettings => {
            game.list_native_mods();
            if let Some(index) = game
                .settings
                .mods
                .installed
                .iter()
                .position(|entry| entry.id == jei::ID && entry.native.is_some())
            {
                with(|s| s.recipes.close());
                game.mod_browser_closed();
                game.open_options_at(false, horizon_client::options_ui::Page::NativeMod(index));
                game.native_settings_refresh(index);
                super::justenoughitems_options::open(game, Opened::ConfigButton);
            }
        }
    }
}

/// Cheat mode: a creative player takes the stack as from the creative
/// inventory; elsewhere it is `/give`, which the server allows to operators.
fn give(game: &mut Game<'_>, id: u32, count: u32) {
    let Some((stack, key)) = with(|s| {
        s.data
            .as_ref()
            .and_then(|d| d.ingredient(id))
            .map(|i| (i.stack.clone(), i.key.clone()))
    })
    .flatten() else {
        return;
    };
    let max = game.res.items().get(&stack.item).map_or(64, |i| i.max_stack.max(1));
    let count = count.clamp(1, max);
    if game.creative {
        let mut stack = stack;
        stack.count = count;
        let _ = game.give(stack);
    } else {
        game.send_command(&format!("give @s {key} {count}"));
    }
}

/// The "+" button: the recipe's items moved into the open crafting grid by
/// ordinary slot clicks, as a player would move them, so it works on any
/// server; `all` (Shift) fills it as many times as the items allow. The
/// grid's old contents go back to the inventory first. JEI goes back to the
/// crafting screen once the items are in.
fn transfer(game: &mut Game<'_>, recipe: usize, all: bool) {
    use horizon_client::menu::Input;
    let Some(cells) = with(|s| {
        let data = s.data.as_ref()?;
        let r = data.index.recipes.get(recipe)?;
        Some(
            r.inputs
                .iter()
                .map(|slot| {
                    slot.iter()
                        .filter_map(|st| data.ingredient(st.id).map(|i| i.key.clone()))
                        .collect::<Vec<String>>()
                })
                .collect::<Vec<_>>(),
        )
    })
    .flatten() else {
        return;
    };
    let note = |key: &str| {
        with(|s| s.note = Some((jei::tr(s.ru, key).to_owned(), Instant::now())));
    };
    if game.carried.is_some() {
        note("no_room");
        return;
    }
    let size = game.craft_size.clamp(2, 3);
    // A cell of the 3x3 layout as a slot of the open grid.
    let grid_slot = |i: usize| {
        let (row, col) = (i / 3, i % 3);
        (row < size && col < size).then_some(100 + row * size + col)
    };
    let mut plan: Vec<(usize, &Vec<String>)> = Vec::new();
    for (i, keys) in cells.iter().enumerate() {
        if keys.is_empty() {
            continue;
        }
        let Some(slot) = grid_slot(i) else {
            note("too_large");
            return;
        };
        plan.push((slot, keys));
    }
    // The grid back into the inventory.
    for slot in 100..100 + size * size {
        if game.slot_view(slot).is_some() {
            game.menu_click(Some(slot), 0, Input::QuickMove);
        }
    }
    if (100..100 + size * size).any(|slot| game.slot_view(slot).is_some()) {
        note("no_room");
        return;
    }
    // What the inventory holds, by key.
    let mut have: HashMap<String, u32> = HashMap::new();
    let mut max_stack: HashMap<String, u32> = HashMap::new();
    for slot in 0..36 {
        if let Some(stack) = game.slot_view(slot) {
            let key = stack_key(&stack);
            let max = game.res.items().get(&stack.item).map_or(64, |i| i.max_stack.max(1));
            max_stack.insert(key.clone(), max);
            *have.entry(key).or_default() += stack.count;
        }
    }
    // Each cell's item: the first that is there for every cell using it.
    let mut uses: HashMap<String, u32> = HashMap::new();
    let mut chosen: Vec<(usize, String)> = Vec::new();
    for (slot, keys) in &plan {
        let Some(key) = keys
            .iter()
            .find(|k| have.get(*k).copied().unwrap_or(0) > uses.get(*k).copied().unwrap_or(0))
        else {
            note("missing");
            return;
        };
        *uses.entry(key.clone()).or_default() += 1;
        chosen.push((*slot, key.clone()));
    }
    let times = if all {
        uses.iter()
            .map(|(k, n)| (have[k] / n).min(max_stack.get(k).copied().unwrap_or(64)))
            .min()
            .unwrap_or(1)
            .max(1)
    } else {
        1
    };
    for (cell, key) in chosen {
        let mut left = times;
        for source in 0..36 {
            if left == 0 {
                break;
            }
            let Some(stack) = game.slot_view(source) else {
                continue;
            };
            if stack_key(&stack) != key {
                continue;
            }
            game.menu_click(Some(source), 0, Input::Pickup);
            let mut placed = 0;
            while placed < left {
                let Some(carried) = game.carried.as_ref().map(|c| c.count) else {
                    break;
                };
                // The whole held stack when it all goes, else one at a time.
                if carried <= left - placed {
                    game.menu_click(Some(cell), 0, Input::Pickup);
                    placed += carried;
                } else {
                    game.menu_click(Some(cell), 1, Input::Pickup);
                    placed += 1;
                }
            }
            if game.carried.is_some() {
                game.menu_click(Some(source), 0, Input::Pickup);
            }
            left -= placed;
        }
    }
    with(|s| s.recipes.close());
}

/// Whether the "+" button can move this recipe here: a crafting recipe on
/// the crafting table, or on the inventory when every item falls in its 2x2.
pub(super) fn can_transfer(game: &Game<'_>, recipe: &Recipe) -> bool {
    if recipe.category() != Category::Crafting || !game.mod_recipe_screen() {
        return false;
    }
    let size = game.craft_size.clamp(2, 3);
    recipe
        .inputs
        .iter()
        .enumerate()
        .all(|(i, slot)| slot.is_empty() || (i / 3 < size && i % 3 < size))
}

/// R, U or A over one of the screen's own slots.
fn slot_key(game: &mut Game<'_>, key: SlotKey, at: (f32, f32)) {
    let Some(size) = game.renderer.as_ref().map(|r| [r.config.width, r.config.height]) else {
        return;
    };
    let batch = horizon_client::hud::HudBatch::new(size);
    let Some(slot) = game.slot_at(&batch, at.0, at.1) else {
        return;
    };
    let Some(stack) = game.slot_view(slot) else {
        return;
    };
    let game: &Game<'_> = game;
    with(|s| {
        let Some(id) = s.data.as_ref().and_then(|d| d.id_of(&stack)) else {
            return;
        };
        match key {
            SlotKey::Recipes => open(s, Lookup::Recipes(id)),
            SlotKey::Uses => open(s, Lookup::Uses(id)),
            SlotKey::Bookmark => toggle_bookmark(game, s, id),
        }
    });
}

/// Opens the recipes screen on `lookup`; nothing when it has no recipes.
pub(super) fn open(state: &mut State, lookup: Lookup) {
    let Some(data) = state.data.as_ref() else {
        return;
    };
    if state.recipes.open(&data.index, lookup) {
        state.tab_scroll = 0;
        if let Some(search) = state.search.as_mut() {
            search.focused = false;
        }
    }
}

fn hit_at(state: &State, p: (f32, f32)) -> Option<Hit> {
    state
        .hits
        .iter()
        .rev()
        .find(|(r, _)| jei::inside(p, *r))
        .map(|(_, h)| *h)
}

fn screen_shown(game: &Game<'_>, screen: &NativeScreenView<'_>) -> bool {
    on(game) && matches!(screen.id, "inventory" | "creative") && game.title.is_none()
}

/// A mouse button over a container screen
/// (`screen_mouse_button_pressed_pre`): true when an overlay took it.
pub(super) fn pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32, modifiers: u32) -> bool {
    if !screen_shown(game, screen) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    let shift = modifiers & SHIFT != 0;
    let control = modifiers & CONTROL != 0;
    with(|s| {
        let hit = hit_at(s, p);
        let covered = s.recipes.is_open();
        if hit != Some(Hit::Search)
            && let Some(search) = s.search.as_mut()
        {
            search.focused = false;
        }
        let Some(hit) = hit else {
            // The recipes screen covers the container: nothing under it is clicked.
            if covered {
                take_press(s, button);
                return true;
            }
            return false;
        };
        take_press(s, button);
        if button > 1 && !matches!(hit, Hit::List(_) | Hit::Bookmark(_)) {
            return true;
        }
        let right = button == 1;
        match hit {
            Hit::List(id) => {
                if settings(game, s).cheat {
                    let count = if right {
                        1
                    } else {
                        game.res
                            .items()
                            .get(&s.data.as_ref().and_then(|d| d.ingredient(id)).map(|i| i.stack.item.clone()).unwrap_or_default())
                            .map_or(64, |i| i.max_stack)
                    };
                    s.pending.push(Action::Give { id, count });
                } else if button <= 1 {
                    open(s, if right { Lookup::Uses(id) } else { Lookup::Recipes(id) });
                }
            }
            Hit::Bookmark(id) | Hit::Slot(id) | Hit::Catalyst(id) => {
                if button <= 1 {
                    open(s, if right { Lookup::Uses(id) } else { Lookup::Recipes(id) });
                }
            }
            Hit::ListPage(d) => {
                let per = s.list_per;
                let len = s.filtered.as_ref().map_or(0, Vec::len);
                s.list_page = jei::turn(s.list_page, d, jei::pages(len, per.max(1)));
            }
            Hit::BookmarkPage(d) => {
                let per = s.bookmark_per;
                let len = bookmarks(s).keys.len();
                s.bookmark_page = jei::turn(s.bookmark_page, d, jei::pages(len, per.max(1)));
            }
            Hit::BookmarkButton => {
                if !bookmarks(s).keys.is_empty() {
                    let shown = !settings(game, s).bookmarks;
                    set_setting(game, s, "bookmarks", shown);
                }
            }
            Hit::Search => {
                let atlas = &game.gui.atlas;
                if let Some(search) = s.search.as_mut() {
                    search.focused = true;
                    if right {
                        search.set_value("");
                    } else {
                        search.click(atlas, p.0, shift, false);
                    }
                    search.reset_blink();
                }
            }
            Hit::Config => {
                if control {
                    let cheat = !settings(game, s).cheat;
                    set_setting(game, s, "cheat", cheat);
                } else {
                    s.pending.push(Action::OpenSettings);
                }
            }
            Hit::Tab(t) => s.recipes.select_tab(t),
            Hit::TabPage(d) => {
                let per = s.tab_scroll.max(1);
                let pages = s.recipes.tabs.len().div_ceil(per);
                let page = jei::turn(s.recipes.tab / per, d, pages);
                s.recipes.select_tab(page * per);
            }
            Hit::Title => {
                if let Some(c) = s.recipes.category()
                    && let Some(data) = s.data.as_ref()
                {
                    s.recipes.open(&data.index, Lookup::Category(c));
                }
            }
            Hit::CategoryTurn(d) => s.recipes.turn_tab(d),
            Hit::RecipePage(d) => {
                let per = s.per_page.max(1);
                s.recipes.turn_page(d, per);
            }
            Hit::Transfer(recipe) => s.pending.push(Action::Transfer { recipe, all: shift }),
            Hit::Area => {}
        }
        true
    })
    .unwrap_or(false)
}

fn take_press(state: &mut State, button: i32) {
    if let Some(p) = state.pressed.get_mut(button.clamp(0, 7) as usize) {
        *p = true;
    }
}

/// A mouse button let go (`screen_mouse_button_released_pre`): taken when its
/// press was.
pub(super) fn released(game: &Game<'_>, screen: &NativeScreenView<'_>, button: i32) -> bool {
    if !screen_shown(game, screen) {
        return false;
    }
    with(|s| {
        s.pressed
            .get_mut(button.clamp(0, 7) as usize)
            .is_some_and(|p| std::mem::replace(p, false))
    })
    .unwrap_or(false)
}

/// The wheel (`screen_mouse_scrolled_pre`): turns the page of the overlay
/// under the pointer.
pub(super) fn scrolled(game: &Game<'_>, screen: &NativeScreenView<'_>, scroll: [f64; 2]) -> bool {
    if !screen_shown(game, screen) {
        return false;
    }
    let p = (screen.mouse[0], screen.mouse[1]);
    let delta = if scroll[1] > 0.0 {
        -1
    } else if scroll[1] < 0.0 {
        1
    } else {
        return false;
    };
    with(|s| {
        let hit = hit_at(s, p);
        if s.recipes.is_open() {
            match hit {
                Some(Hit::Tab(_) | Hit::TabPage(_)) => s.recipes.turn_tab(delta),
                _ => {
                    let per = s.per_page.max(1);
                    s.recipes.turn_page(delta, per);
                }
            }
            return true;
        }
        match hit {
            Some(Hit::List(_) | Hit::ListPage(_) | Hit::Search | Hit::Config) => {
                let per = s.list_per;
                let len = s.filtered.as_ref().map_or(0, Vec::len);
                s.list_page = jei::turn(s.list_page, delta, jei::pages(len, per.max(1)));
                true
            }
            Some(Hit::Bookmark(_) | Hit::BookmarkPage(_)) => {
                let per = s.bookmark_per;
                let len = bookmarks(s).keys.len();
                s.bookmark_page = jei::turn(s.bookmark_page, delta, jei::pages(len, per.max(1)));
                true
            }
            Some(Hit::Area) => true,
            _ => false,
        }
    })
    .unwrap_or(false)
}

/// The key a mod key is bound to (`key.keyboard.r`).
fn bound(game: &Game<'_>, name: &str) -> String {
    let default = jei::KEYS
        .iter()
        .find(|k| k.0 == name)
        .map_or("key.keyboard.unknown", |k| k.2);
    game.native_mod_key(jei::ID, name, default)
}

/// A key over a container screen (`screen_key_pressed_pre`): true when the
/// mod took it.
pub(super) fn key_pressed(game: &Game<'_>, screen: &NativeScreenView<'_>, key: &str, modifiers: u32) -> bool {
    if !screen_shown(game, screen) {
        return false;
    }
    let control = modifiers & CONTROL != 0;
    let shift = modifiers & SHIFT != 0;
    let p = (screen.mouse[0], screen.mouse[1]);
    let is = |name: &str| bound(game, name) == key;
    with(|s| {
        // The search box takes every key but Escape and Enter, which let it go.
        if s.search.as_ref().is_some_and(|b| b.focused) {
            let search = s.search.as_mut().expect("focused");
            match key {
                "key.keyboard.escape" | "key.keyboard.enter" | "key.keyboard.keypad.enter" => {
                    search.focused = false;
                }
                "key.keyboard.v" if control => s.pending.push(Action::Paste),
                // EditBox's copy and cut: the selection to the clipboard (the
                // copy waits for the game, so it takes the text now), and a
                // cut deletes it.
                "key.keyboard.c" | "key.keyboard.x" if control => {
                    let text = search.selected_text();
                    if !text.is_empty() {
                        s.pending.push(Action::Copy(text));
                        if key == "key.keyboard.x" {
                            search.delete_chars(0);
                        }
                    }
                }
                _ => {
                    if let Some(code) = horizon_client::keybinds::key_code(key) {
                        search.key(code, control, shift);
                    }
                }
            }
            search.reset_blink();
            return true;
        }
        if control && is("toggle_overlay") {
            let shown = !settings(game, s).overlay;
            set_setting(game, s, "overlay", shown);
            return true;
        }
        if control && is("focus_search") {
            if settings(game, s).overlay
                && let Some(search) = s.search.as_mut()
            {
                search.focused = true;
                search.reset_blink();
            }
            return true;
        }
        if is("toggle_bookmarks") {
            let shown = !settings(game, s).bookmarks;
            set_setting(game, s, "bookmarks", shown);
            return true;
        }
        if is("toggle_cheat") {
            let cheat = !settings(game, s).cheat;
            set_setting(game, s, "cheat", cheat);
            return true;
        }
        // A text box of the game's has the keys (creative search, recipe
        // book, anvil name): JEI's keys wait.
        let game_typing = (game.screen == Screen::Creative
            && game.creative_screen.tab().search
            && game.creative_screen.search.focused)
            || game.recipe_search_focused()
            || game.station_typing();
        if game_typing && !s.recipes.is_open() {
            return false;
        }
        let hit = hit_at(s, p);
        let over = match hit {
            Some(Hit::List(id) | Hit::Bookmark(id) | Hit::Slot(id) | Hit::Catalyst(id)) => Some(id),
            _ => None,
        };
        let covered = s.recipes.is_open();
        let lookup_key = if is("show_recipe") {
            Some(SlotKey::Recipes)
        } else if is("show_uses") {
            Some(SlotKey::Uses)
        } else if is("bookmark") {
            Some(SlotKey::Bookmark)
        } else {
            None
        };
        if let Some(k) = lookup_key {
            match over {
                Some(id) => match k {
                    SlotKey::Recipes => open(s, Lookup::Recipes(id)),
                    SlotKey::Uses => open(s, Lookup::Uses(id)),
                    SlotKey::Bookmark => toggle_bookmark(game, s, id),
                },
                None if !covered && hit.is_none() => {
                    s.pending.push(Action::SlotKey { key: k, at: p });
                    // The key's own use (A, R or U on a slot) is none: let it go on.
                    return false;
                }
                None => {}
            }
            return true;
        }
        if covered {
            let inventory = game.settings.keys.get("key.inventory") == key;
            if key == "key.keyboard.escape" || inventory {
                s.recipes.close();
            } else if is("recipe_back") {
                if let Some(data) = s.data.as_ref()
                    && !s.recipes.back(&data.index)
                {
                    s.recipes.close();
                }
            } else if is("previous_page") || is("next_page") {
                let d = if is("previous_page") { -1 } else { 1 };
                if shift {
                    s.recipes.turn_tab(d);
                } else {
                    let per = s.per_page.max(1);
                    s.recipes.turn_page(d, per);
                }
            }
            // Nothing under the recipes screen hears a key.
            return true;
        }
        if is("previous_page") || is("next_page") {
            let d = if is("previous_page") { -1 } else { 1 };
            let per = s.list_per;
            let len = s.filtered.as_ref().map_or(0, Vec::len);
            s.list_page = jei::turn(s.list_page, d, jei::pages(len, per.max(1)));
            return true;
        }
        false
    })
    .unwrap_or(false)
}

/// A character typed (`screen_char_typed_pre`): the search box's, when it
/// has focus.
pub(super) fn char_typed(game: &Game<'_>, screen: &NativeScreenView<'_>, c: char) -> bool {
    if !screen_shown(game, screen) {
        return false;
    }
    with(|s| {
        let covered = s.recipes.is_open();
        match s.search.as_mut() {
            Some(search) if search.focused => {
                search.insert_text(&c.to_string());
                search.reset_blink();
                true
            }
            _ => covered,
        }
    })
    .unwrap_or(false)
}

/// The container screen closed (`mod_browser_closed`).
pub(super) fn closed() {
    with(|s| {
        s.recipes.close();
        if let Some(search) = s.search.as_mut() {
            search.focused = false;
        }
    });
    COVERED.with(|c| c.set(false));
}

/// The item tooltip (`tooltip`): the mod's name at its foot, blue and
/// italic, as JEI adds it.
pub(super) fn tooltip(stack: &ItemStack, lines: &mut Vec<(horizon_client::text::Text, u32)>) {
    if !MOD_NAME.with(Cell::get) || !OWN_TOOLTIP.with(Cell::get) || lines.is_empty() {
        return;
    }
    let namespace = stack.item.split(':').next().unwrap_or("minecraft");
    let name = jei::mod_display_name(namespace);
    lines.push((horizon_client::text::Text::legacy(&format!("§9§o{name}")), u32::MAX));
}

/// The game's tooltips under the recipes screen stay hidden
/// (`native_tooltip_render_pre`); this mod's own show.
pub(super) fn hide_tooltip() -> bool {
    COVERED.with(Cell::get) && !OWN_TOOLTIP.with(Cell::get)
}

// ---- Options > Mods ------------------------------------------------------------

/// The row on Options > Mods, with the mod's keys.
pub(super) fn mods(game: &Game<'_>, mods: &mut Vec<super::native_hooks::NativeMod>) {
    let ru = jei::russian(&horizon_client::lang::code());
    let _ = game;
    mods.push(super::native_hooks::NativeMod {
        id: jei::ID.into(),
        title: format!("{} {}", jei::NAME, jei::VERSION),
        description: jei::tr(ru, "description").into(),
        keys: jei::KEYS
            .iter()
            .map(|(name, label, default)| {
                [name.to_string(), jei::tr(ru, label).to_string(), default.to_string()]
            })
            .collect(),
    });
}

/// The settings page's buttons: Mod Menu's "Configure..." first (JEI's config
/// screen), then the switches and the bookmarks.
pub(super) fn settings_buttons(game: &mut Game<'_>, mod_id: &str, buttons: &mut Vec<String>) {
    if mod_id != jei::ID {
        return;
    }
    let ru = jei::russian(&horizon_client::lang::code());
    buttons.push(jei::tr(ru, "modmenu_configure").to_owned());
    let game: &Game<'_> = game;
    with(|st| {
        let s = settings(game, st);
        for key in Settings::KEYS {
            let state = jei::tr(ru, if s.get(key) { "on" } else { "off" });
            buttons.push(format!("{}: {state}", jei::tr(ru, key)));
        }
        let count = bookmarks(st).keys.len();
        buttons.push(format!("{} ({count})", jei::tr(ru, "clear_bookmarks")));
    });
}

pub(super) fn settings_click(game: &mut Game<'_>, mod_id: &str, button: usize) {
    if mod_id != jei::ID {
        return;
    }
    // The first button and Mod Menu's Configure: JEI's config screen.
    if button == 0 || button == super::justenoughitems_options::MODMENU_CONFIGURE {
        let mode = if button == 0 { Opened::Page } else { Opened::ModMenu };
        super::justenoughitems_options::open(game, mode);
        return;
    }
    let button = button - 1;
    let game: &Game<'_> = game;
    with(|st| {
        if let Some(key) = Settings::KEYS.get(button) {
            let on = !settings(game, st).get(key);
            set_setting(game, st, key, on);
        } else if button == Settings::KEYS.len() {
            bookmarks(st).keys.clear();
            save_bookmarks(st);
        }
    });
}

/// The search box, made on first use.
pub(super) fn search_box(state: &mut State) -> &mut EditBox {
    state.search.get_or_insert_with(|| {
        let mut edit = EditBox::new(0.0, 0.0, 0.0, 0.0);
        edit.max_length = SEARCH_MAX;
        edit.focused = false;
        edit
    })
}
