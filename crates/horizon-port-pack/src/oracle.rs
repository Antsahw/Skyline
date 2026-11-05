//! The NeoForge oracle's facts (`tools/neoforge_oracle.py export`, in
//! `<work>/oracle/facts.json`): every block, item, recipe and tag the pack's
//! mods register, read from the running game, and the vanilla blocks beside
//! them. Carry-over uses them in place of guesses: a block's template is the
//! vanilla block of the same class and states whose shapes and sounds match
//! best, and the declared properties are the mod's own numbers.
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};

#[derive(Debug, Deserialize)]
pub struct Facts {
    pub blocks: BTreeMap<String, Block>,
    pub items: BTreeMap<String, Item>,
    #[serde(default)]
    pub tags: Tags,
    /// The game's items' default components (`items.json`), so item ports leave out what their template has.
    #[serde(skip)]
    pub vanilla_items: BTreeMap<String, Value>,
    /// Every vanilla state's shapes, for [`Facts::own_states`] (made once).
    #[serde(skip)]
    shapes: std::sync::OnceLock<ShapeIndex>,
}

/// Vanilla states by their (outline, collision), and every one with its shapes.
#[derive(Debug, Default)]
struct ShapeIndex {
    exact: BTreeMap<(String, String), String>,
    /// Each state's outline, collision and occlusion (JSON text), by its full name.
    by_name: std::collections::HashMap<String, [String; 3]>,
    /// Each state, its outline's bounds and its collision's.
    all: Vec<(String, Option<[f64; 6]>, Option<[f64; 6]>)>,
    /// `all` with one entry per (outline bounds, collision bounds): the first name of each
    /// (the nearest search ties by name), a few hundred instead of every state.
    distinct: Vec<(String, Option<[f64; 6]>, Option<[f64; 6]>)>,
    /// The nearest vanilla state found for each (outline bounds, collision bounds) asked:
    /// a mod's blocks share a few shapes over thousands of states.
    nearest: std::sync::Mutex<std::collections::HashMap<BoundsKey, String>>,
}

/// A pair of shapes' bounds, bit for bit, as a map key.
type BoundsKey = (Option<[u64; 6]>, Option<[u64; 6]>);

fn bounds_key(outline: Option<[f64; 6]>, collision: Option<[f64; 6]>) -> BoundsKey {
    (
        outline.map(|b| b.map(f64::to_bits)),
        collision.map(|b| b.map(f64::to_bits)),
    )
}

#[derive(Debug, Default, Deserialize)]
pub struct Tags {
    #[serde(default)]
    pub block: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub item: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct Block {
    /// The class and its superclasses, most derived first.
    pub class: Vec<String>,
    pub properties: BTreeMap<String, Vec<String>>,
    pub default: String,
    pub item: String,
    pub explosion_resistance: f32,
    pub friction: f32,
    pub speed_factor: f32,
    pub jump_factor: f32,
    #[serde(default)]
    pub block_set_type: Option<SetType>,
    #[serde(default)]
    pub wood_type: Option<String>,
    pub states: BTreeMap<String, State>,
}

#[derive(Debug, Deserialize)]
pub struct SetType {
    pub name: String,
    pub can_open_by_hand: bool,
}

#[derive(Debug, Deserialize)]
pub struct State {
    pub hardness: f32,
    pub emission: u8,
    pub requires_tool: bool,
    /// `BlockState.isRandomlyTicking` (exporters that write it).
    #[serde(default)]
    pub random_ticks: bool,
    /// `RenderShape.INVISIBLE`: its block entity's renderer draws it, not its model.
    #[serde(default)]
    pub invisible: bool,
    pub map_color_id: u8,
    pub sound: Value,
    pub outline: Value,
    pub collision: Value,
    /// The faces it hides beside it (empty when it can't occlude), exporters that write it.
    #[serde(default)]
    pub occlusion: Option<Value>,
    /// What breaking the state drops, per tool (`hand`, `diamond_pickaxe`,
    /// `shears`, `silk_touch_pickaxe`...): item id → count.
    #[serde(default)]
    pub drops: BTreeMap<String, BTreeMap<String, u32>>,
}

#[derive(Debug, Deserialize)]
pub struct Item {
    pub class: String,
    pub max_stack_size: u8,
    pub rarity: String,
    #[serde(default)]
    pub block: Option<String>,
    #[serde(default)]
    pub food: Option<Value>,
    /// The default data components in 1.21.1's form; empty in facts from before they were exported.
    #[serde(default)]
    pub components: Map<String, Value>,
    #[serde(default)]
    pub attribute_modifiers: Option<Value>,
    #[serde(default)]
    pub enchantment_value: Option<u32>,
    #[serde(default)]
    pub repair_items: Option<Vec<String>>,
    #[serde(default)]
    pub armor: Option<Armor>,
}

#[derive(Debug, Deserialize)]
pub struct Armor {
    /// `head`, `chest`, `legs` or `feet`.
    pub slot: String,
    /// The armour material's id: the 26.3 equipment asset it becomes.
    #[serde(default)]
    pub material: String,
    #[serde(default)]
    pub equip_sound: Option<String>,
    #[serde(default)]
    pub layers: Vec<ArmorLayer>,
}

/// A 1.21.1 armour layer's textures: `<ns>:textures/models/armor/<name>_layer_1.png` and `_layer_2`.
#[derive(Debug, Deserialize)]
pub struct ArmorLayer {
    pub texture: String,
    pub texture_legs: String,
    #[serde(default)]
    pub dyeable: bool,
}

impl Armor {
    /// Whether carry writes the material's equipment asset: in the mod's own namespaces, from layers it can place.
    pub fn asset_carried(&self, owned: &[&str]) -> bool {
        self.material
            .split_once(':')
            .is_some_and(|(ns, _)| owned.contains(&ns))
            && self.equipment().is_some()
    }

    /// The 26.3 equipment asset of the material (`layers` by kind), and the 1.21.1 textures to copy, as (from, to) paths in the mod.
    pub fn equipment(&self) -> Option<(Value, Vec<(String, String)>)> {
        self.material.split_once(':')?;
        let mut copies = Vec::new();
        let mut kinds = Map::new();
        for (kind, legs) in [("humanoid", false), ("humanoid_leggings", true)] {
            let mut list = Vec::new();
            for layer in &self.layers {
                let texture = if legs {
                    &layer.texture_legs
                } else {
                    &layer.texture
                };
                let (ns, path) = texture.split_once(':')?;
                let file = path.strip_prefix("textures/models/armor/")?;
                let name = file.strip_suffix(if legs { "_layer_2.png" } else { "_layer_1.png" })?;
                copies.push((
                    format!("assets/{ns}/{path}"),
                    format!("assets/{ns}/textures/entity/equipment/{kind}/{name}.png"),
                ));
                let mut entry = json!({"texture": format!("{ns}:{name}")});
                if layer.dyeable {
                    entry["dyeable"] = json!({});
                }
                list.push(entry);
            }
            kinds.insert(kind.into(), Value::Array(list));
        }
        Some((json!({"layers": kinds}), copies))
    }
}

/// The vanilla item standing in for an item of a vanilla class, for the behaviour the class gives.
fn item_template(item: &Item) -> &'static str {
    let class = item.class.strip_prefix("net.minecraft.world.item.");
    match (class, item.armor.as_ref().map(|a| a.slot.as_str())) {
        (Some("SwordItem"), _) => "minecraft:iron_sword",
        (Some("PickaxeItem"), _) => "minecraft:iron_pickaxe",
        (Some("AxeItem"), _) => "minecraft:iron_axe",
        (Some("ShovelItem"), _) => "minecraft:iron_shovel",
        (Some("HoeItem"), _) => "minecraft:iron_hoe",
        (Some("ArmorItem"), Some("head")) => "minecraft:iron_helmet",
        (Some("ArmorItem"), Some("chest")) => "minecraft:iron_chestplate",
        (Some("ArmorItem"), Some("legs")) => "minecraft:iron_leggings",
        (Some("ArmorItem"), Some("feet")) => "minecraft:iron_boots",
        _ => "minecraft:stick",
    }
}

/// 1.21.1's `{"modifiers": [...]}` as 26.3's list, with attribute ids' `generic.` and `player.` dropped.
fn modifiers_26_3(v: &Value) -> Value {
    let list = v["modifiers"].as_array().cloned().unwrap_or_default();
    Value::Array(
        list.into_iter()
            .map(|mut m| {
                if let Some(t) = m["type"].as_str() {
                    let t = t.replacen(":generic.", ":", 1).replacen(":player.", ":", 1);
                    m["type"] = json!(t);
                }
                m
            })
            .collect(),
    )
}

/// The components carried onto an item's template: what it does when used, worn or held.
/// (Its name, model and tooltip come with the content mod's own id and assets.)
const CARRIED_COMPONENTS: &[&str] = &[
    "minecraft:food",
    "minecraft:consumable",
    "minecraft:use_remainder",
    "minecraft:use_cooldown",
    "minecraft:max_damage",
    "minecraft:tool",
    "minecraft:weapon",
    "minecraft:attribute_modifiers",
    "minecraft:equippable",
    "minecraft:enchantable",
    "minecraft:repairable",
    "minecraft:damage_resistant",
    "minecraft:death_protection",
    "minecraft:blocks_attacks",
    "minecraft:glider",
    "minecraft:fireworks",
    "minecraft:jukebox_playable",
    "minecraft:instrument",
];

/// Whether a JSON value names only ids Horizon has: vanilla's (and tags of them), or the
/// mod's own items (`own`), not its effects, sounds or other registries' entries.
fn known_ids(v: &Value, own: &str) -> bool {
    match v {
        Value::String(s) => {
            let id = s.strip_prefix('#').unwrap_or(s);
            match id.split_once(':') {
                Some((ns, path))
                    if !ns.is_empty()
                        && ns
                            .chars()
                            .all(|c| matches!(c, 'a'..='z' | '0'..='9' | '_' | '-' | '.'))
                        && !path.is_empty()
                        && !path.contains(' ') =>
                {
                    ns == "minecraft" || ns == own
                }
                _ => true,
            }
        }
        Value::Array(a) => a.iter().all(|v| known_ids(v, own)),
        Value::Object(o) => o.values().all(|v| known_ids(v, own)),
        _ => true,
    }
}

const VANILLA_PACKAGE: &str = "net.minecraft.";

/// A shape's bounds (a box list: `[x0 y0 z0 x1 y1 z1]...`); `None` when empty.
fn bounds(v: &Value) -> Option<[f64; 6]> {
    let boxes = v.as_array()?;
    let mut out = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
    for b in boxes {
        let c: Vec<f64> = b.as_array()?.iter().filter_map(Value::as_f64).collect();
        if c.len() != 6 {
            return None;
        }
        for i in 0..3 {
            out[i] = out[i].min(c[i]);
            out[i + 3] = out[i + 3].max(c[i + 3]);
        }
    }
    (!boxes.is_empty()).then_some(out)
}

/// How far apart two shapes' bounds are (a box list each: `[x0 y0 z0 x1 y1 z1]...`);
/// an empty shape is far from any other.
fn bounds_distance(a: Option<[f64; 6]>, b: Option<[f64; 6]>) -> f64 {
    match (a, b) {
        (Some(x), Some(y)) => x.iter().zip(y).map(|(p, q)| (p - q).abs()).sum(),
        (None, None) => 0.0,
        _ => 6.0,
    }
}

/// A state key with its properties sorted, so keys compare across blocks.
/// Which states of a block drop the block itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelfDrop {
    /// Every state: `dropSelf`.
    Always,
    /// Only the states where this property has this value.
    When(String, String),
}

/// A state key's `property=value` pairs.
fn pairs(key: &str) -> BTreeSet<(String, String)> {
    key.split(',')
        .filter_map(|p| p.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// From this many own states, a block's declaration lists them as a `state_table` (each
/// property's values and the distinct shapes once, a row of indices per state) instead of a
/// `states` array: ALA Industrial's pipes (76,832 states each) took 18.8 MB as an array, over
/// the content fence (`horizon_mods::content::MAX_FILE`), and the whole mod was dropped.
pub const STATE_TABLE_FROM: usize = 1024;

/// `states` (the declaration's array) as it goes in the file: the array itself when short,
/// else an object for `state_table`, rows in the same order.
pub fn compact_states(states: Vec<Value>) -> Value {
    if states.len() < STATE_TABLE_FROM {
        return Value::Array(states);
    }
    let mut props: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in &states {
        for (k, v) in s["properties"].as_object().into_iter().flatten() {
            let vals = props.entry(k.clone()).or_default();
            let v = v.as_str().unwrap_or_default();
            if !vals.iter().any(|x| x == v) {
                vals.push(v.to_owned());
            }
        }
    }
    let mut shapes: Vec<Value> = Vec::new();
    let mut shape_at: BTreeMap<String, usize> = BTreeMap::new();
    let (mut rows, mut light, mut ticks, mut default) = (Vec::new(), Map::new(), Vec::new(), None);
    for (i, s) in states.iter().enumerate() {
        let shape = match &s["shape"] {
            Value::String(n) => n.clone(),
            _ => String::new(),
        };
        // A state's own boxes make its shape entry an object: `like` the vanilla state, then
        // the boxes of its own.
        let mut own = Map::new();
        for k in ["outline", "collision", "occlusion"] {
            if let Some(b) = s.get(k) {
                own.insert(k.to_owned(), b.clone());
            }
        }
        let entry = if own.is_empty() {
            if shape.is_empty() {
                Value::Null
            } else {
                json!(shape)
            }
        } else {
            if !shape.is_empty() {
                own.insert("like".to_owned(), json!(shape));
            }
            Value::Object(own)
        };
        let at = *shape_at.entry(entry.to_string()).or_insert_with(|| {
            shapes.push(entry);
            shapes.len() - 1
        });
        let mut row = vec![at];
        for (k, vals) in &props {
            let v = s["properties"][k].as_str().unwrap_or_default();
            row.push(vals.iter().position(|x| x == v).unwrap_or(0));
        }
        rows.push(row);
        if let Some(l) = s.get("light") {
            light.insert(i.to_string(), l.clone());
        }
        if s["random_ticks"] == json!(true) {
            ticks.push(i);
        }
        if s["default"] == json!(true) {
            default = Some(i);
        }
    }
    let props: Vec<Value> = props.into_iter().map(|(k, v)| json!([k, v])).collect();
    let mut t = json!({"properties": props, "shapes": shapes, "rows": rows});
    if !light.is_empty() {
        t["light"] = Value::Object(light);
    }
    if !ticks.is_empty() {
        t["random_ticks"] = json!(ticks);
    }
    if let Some(d) = default {
        t["default"] = json!(d);
    }
    t
}

pub fn state_key(key: &str) -> String {
    let mut parts: Vec<&str> = key.split(',').filter(|p| !p.is_empty()).collect();
    parts.sort_unstable();
    parts.join(",")
}

impl Block {
    /// The first vanilla class in the chain.
    pub fn vanilla_class(&self) -> &str {
        self.class
            .iter()
            .find(|c| c.starts_with(VANILLA_PACKAGE))
            .map_or("", String::as_str)
    }
    /// Its own class is a mod class: behaviour code the template may not have.
    pub fn custom_class(&self) -> bool {
        self.class
            .first()
            .is_some_and(|c| !c.starts_with(VANILLA_PACKAGE))
    }
    /// Every state drops one `item` (the block's own) with every tool, except
    /// where the state needs a tool and gets none: vanilla's `dropSelf`. Or
    /// only the states where one property has one value do, and the others
    /// drop nothing with any tool: a door's lower half (`createDoorTable`).
    /// Mods often register these tables in code, so their jar has no file.
    pub fn drops_itself(&self, item: &str) -> Option<SelfDrop> {
        let one = BTreeMap::from([(item.to_owned(), 1)]);
        let (mut dropping, mut nothing) = (Vec::new(), Vec::new());
        for (key, state) in &self.states {
            if state.drops.is_empty() {
                continue;
            }
            let mut drops = false;
            let mut short = false;
            for got in state.drops.values() {
                if *got == one {
                    drops = true;
                } else if got.is_empty() {
                    short = true;
                } else {
                    return None;
                }
            }
            match (drops, short) {
                (true, true) if !state.requires_tool => return None,
                (true, _) => dropping.push(pairs(key)),
                (false, _) => nothing.push(pairs(key)),
            }
        }
        let (first, rest) = dropping.split_first()?;
        if nothing.is_empty() {
            return Some(SelfDrop::Always);
        }
        first
            .iter()
            .filter(|pair| rest.iter().all(|s| s.contains(*pair)))
            .find(|pair| nothing.iter().all(|s| !s.contains(*pair)))
            .map(|(p, v)| SelfDrop::When(p.clone(), v.clone()))
    }
    fn default_state(&self) -> Option<&State> {
        self.states
            .get(&self.default)
            .or_else(|| self.states.values().next())
    }
    fn same_states(&self, other: &Block) -> bool {
        self.properties.len() == other.properties.len()
            && self.properties.iter().all(|(k, v)| {
                other.properties.get(k).is_some_and(|w| {
                    v.iter().collect::<BTreeSet<_>>() == w.iter().collect::<BTreeSet<_>>()
                })
            })
    }
    fn by_key(&self) -> BTreeMap<String, &State> {
        self.states.iter().map(|(k, s)| (state_key(k), s)).collect()
    }
}

/// The template carry-over gives a mod block.
#[derive(Debug, Clone, PartialEq)]
pub struct Choice {
    pub template: String,
    /// `data/<mod>/horizon/block/<name>.json`'s `properties`.
    pub properties: Map<String, Value>,
    /// What the template does not reproduce, for the report.
    pub differences: Vec<String>,
}

/// Equal JSON, numbers compared as f64 (the oracle writes `6.0` where the game's table has `6`).
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

impl Facts {
    pub fn read(path: &Path) -> Result<Facts, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The vanilla block that best stands in for `id`: the same vanilla
    /// class and states, then the most states with the same outline and
    /// collision, then the same sounds, set type, wood type and hardness.
    /// `None` when no vanilla block has its class and states.
    pub fn template(&self, id: &str, allowed: &dyn Fn(&str) -> bool) -> Option<Choice> {
        let block = self.blocks.get(id)?;
        let class = block.vanilla_class();
        let own = block.by_key();
        let ours = block.default_state()?;
        let mut best: Option<(i64, &str, &Block)> = None;
        for (name, v) in &self.blocks {
            if !name.starts_with("minecraft:")
                || v.vanilla_class() != class
                || v.custom_class()
                || !block.same_states(v)
                || !allowed(name)
            {
                continue;
            }
            let theirs = v.by_key();
            let shapes = own
                .iter()
                .filter(|(k, s)| {
                    theirs
                        .get(*k)
                        .is_some_and(|t| t.outline == s.outline && t.collision == s.collision)
                })
                .count() as i64;
            let Some(td) = v.default_state() else {
                continue;
            };
            let same_set = match (&block.block_set_type, &v.block_set_type) {
                (Some(a), Some(b)) => {
                    (a.name == b.name) as i64 * 2
                        + (a.can_open_by_hand == b.can_open_by_hand) as i64 * 8
                }
                _ => 0,
            };
            let score = shapes * 1000
                + (td.sound == ours.sound) as i64 * 100
                + same_set * 10
                + (block.wood_type.is_some() && block.wood_type == v.wood_type) as i64 * 20
                + (td.hardness == ours.hardness) as i64 * 5
                + (td.map_color_id == ours.map_color_id) as i64;
            if best.is_none_or(|(s, n, _)| score > s || (score == s && name.as_str() < n)) {
                best = Some((score, name, v));
            }
        }
        let (_, template, t) = best?;
        Some(self.choice(block, template, t, allowed))
    }

    /// For a block no vanilla block stands in for (its properties are its own): a template
    /// without properties or behaviour (stone), and its own states, each with the vanilla
    /// state whose collision and outline it has (else the nearest), for the declaration's
    /// `states`.
    pub fn own_states(&self, id: &str, allowed: &dyn Fn(&str) -> bool) -> Option<(Choice, Value)> {
        let block = self.blocks.get(id)?;
        let template = [
            "minecraft:stone",
            "minecraft:smooth_stone",
            "minecraft:dirt",
        ]
        .into_iter()
        .find(|t| allowed(t) && self.blocks.get(*t).is_some_and(|b| b.properties.is_empty()))?;
        let mut choice = self.choice(block, template, &self.blocks[template], allowed);
        self.shapes.get_or_init(|| {
            let mut index = ShapeIndex::default();
            for (name, v) in &self.blocks {
                if !name.starts_with("minecraft:") || !allowed(name) {
                    continue;
                }
                for (key, st) in &v.states {
                    let full = format!("{name}[{}]", state_key(key));
                    index.by_name.insert(
                        full.clone(),
                        [
                            st.outline.to_string(),
                            st.collision.to_string(),
                            st.occlusion
                                .as_ref()
                                .map(Value::to_string)
                                .unwrap_or_default(),
                        ],
                    );
                    index
                        .exact
                        .entry((st.outline.to_string(), st.collision.to_string()))
                        .or_insert_with(|| full.clone());
                    index
                        .all
                        .push((full, bounds(&st.outline), bounds(&st.collision)));
                }
            }
            let mut first: BTreeMap<BoundsKey, usize> = BTreeMap::new();
            for (i, (name, o, c)) in index.all.iter().enumerate() {
                let at = first.entry(bounds_key(*o, *c)).or_insert(i);
                if name < &index.all[*at].0 {
                    *at = i;
                }
            }
            let mut picked: Vec<usize> = first.into_values().collect();
            picked.sort_unstable();
            index.distinct = picked.into_iter().map(|i| index.all[i].clone()).collect();
            index
        });
        let ShapeIndex {
            exact,
            distinct,
            nearest,
            by_name,
            ..
        } = self.shapes.get().expect("built above");
        let default = state_key(&block.default);
        let default_light = block.default_state().map_or(0, |s| s.emission);
        let mut near = 0;
        let mut states = Vec::new();
        for (key, st) in &block.states {
            let key = state_key(key);
            let shape = match exact.get(&(st.outline.to_string(), st.collision.to_string())) {
                Some(s) => s.clone(),
                None => {
                    near += 1;
                    let (outline, collision) = (bounds(&st.outline), bounds(&st.collision));
                    let key = bounds_key(outline, collision);
                    let known = nearest
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(&key)
                        .cloned();
                    match known {
                        Some(n) => n,
                        None => {
                            let d = |(_, o, c): &(String, Option<[f64; 6]>, Option<[f64; 6]>)| {
                                bounds_distance(*c, collision) + bounds_distance(*o, outline) * 0.5
                            };
                            // Over one state per pair of bounds: the same distances, and the
                            // tie on the name goes to the same state (each entry's name is
                            // the least of its pair's).
                            let n = distinct
                                .iter()
                                .min_by(|a, b| d(a).total_cmp(&d(b)).then(a.0.cmp(&b.0)))
                                .map(|(n, _, _)| n.clone())?;
                            nearest
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .insert(key, n.clone());
                            n
                        }
                    }
                }
            };
            let properties: Map<String, Value> = pairs(&key)
                .into_iter()
                .map(|(k, v)| (k, Value::String(v)))
                .collect();
            let mut v = json!({"properties": properties, "shape": shape});
            // Its own boxes where the vanilla state's differ: the nearest state's outline and
            // collision are a guess, and a state's occlusion (the faces it hides) need not be
            // its stand-in's (a basin on a full cube's hid the floor under it).
            let like = by_name.get(&shape);
            let differs = |i: usize, own: &Value| {
                let own = own.to_string();
                like.is_none_or(|l| l[i] != own)
            };
            if differs(0, &st.outline) {
                v["outline"] = st.outline.clone();
            }
            if differs(1, &st.collision) {
                v["collision"] = st.collision.clone();
            }
            if let Some(occlusion) = &st.occlusion
                && differs(2, occlusion)
            {
                v["occlusion"] = occlusion.clone();
            }
            if st.emission != default_light {
                v["light"] = json!(st.emission);
            }
            if st.random_ticks {
                v["random_ticks"] = json!(true);
            }
            if key == default {
                v["default"] = json!(true);
            }
            states.push(v);
        }
        if near > 0 {
            choice.differences.push(format!(
                "shapes: {near} of {} states take the nearest vanilla state's",
                states.len()
            ));
        }
        choice.differences.push(format!(
            "its own states: behaviour beyond a plain block ({})",
            block.class.first().map_or("", String::as_str)
        ));
        Some((choice, compact_states(states)))
    }

    fn choice(
        &self,
        block: &Block,
        template: &str,
        t: &Block,
        allowed: &dyn Fn(&str) -> bool,
    ) -> Choice {
        let (ours, theirs) = (block.default_state(), t.default_state());
        let mut properties = Map::new();
        let mut differences = Vec::new();
        if let Some(s) = ours {
            // Any negative destroy time is unbreakable to vanilla (Better End's eternal pedestal
            // has -11); Horizon's content format says it as -1.
            let hardness = if s.hardness < 0.0 { -1.0 } else { s.hardness };
            properties.insert("hardness".into(), json!(hardness));
            properties.insert(
                "explosion_resistance".into(),
                json!(block.explosion_resistance),
            );
            properties.insert("light".into(), json!(s.emission));
            properties.insert("requires_tool".into(), json!(s.requires_tool));
            properties.insert("friction".into(), json!(block.friction));
            properties.insert("map_color".into(), json!(s.map_color_id));
            if theirs.is_none_or(|t| t.sound != s.sound) {
                match self.sound_block(&s.sound, allowed) {
                    Some(b) => {
                        properties.insert("sounds".into(), json!(b));
                    }
                    None => differences.push("sounds: no vanilla block has them".into()),
                }
            }
        }
        if (block.speed_factor, block.jump_factor) != (t.speed_factor, t.jump_factor) {
            differences.push(format!(
                "speed and jump factors {} {} (template {} {})",
                block.speed_factor, block.jump_factor, t.speed_factor, t.jump_factor
            ));
        }
        let theirs = t.by_key();
        let mut shapes = 0;
        let mut varying = BTreeSet::new();
        let first = ours.map(|s| (s.hardness, s.emission, s.requires_tool, s.map_color_id));
        for (k, s) in block.by_key() {
            let Some(v) = theirs.get(&k) else {
                continue;
            };
            if v.outline != s.outline || v.collision != s.collision {
                shapes += 1;
            }
            if Some((s.hardness, s.emission, s.requires_tool, s.map_color_id)) != first {
                varying.insert("per-state hardness, light or map colour");
            }
        }
        if shapes > 0 {
            differences.push(format!(
                "shapes differ from {template} in {shapes} of {} states",
                block.states.len()
            ));
        }
        differences.extend(varying.into_iter().map(str::to_owned));
        if block.custom_class() {
            differences.push(format!(
                "class {} (behaviour beyond {})",
                block.class[0],
                block.vanilla_class()
            ));
        }
        Choice {
            template: template.to_owned(),
            properties,
            differences,
        }
    }

    /// A vanilla block whose default state has these sounds.
    fn sound_block(&self, sound: &Value, allowed: &dyn Fn(&str) -> bool) -> Option<&str> {
        self.blocks
            .iter()
            .filter(|(n, _)| n.starts_with("minecraft:") && allowed(n))
            .find(|(_, b)| b.default_state().is_some_and(|s| &s.sound == sound))
            .map(|(n, _)| n.as_str())
    }

    /// An item declaration for a mod item that is not a declared block's:
    /// the vanilla item of its class with its stack size, rarity and numbers,
    /// and what it does not reproduce.
    pub fn item(&self, id: &str, owned: &[&str]) -> Result<(Value, Vec<String>), String> {
        let item = self.items.get(id).ok_or("not in the oracle")?;
        let mut differences = Vec::new();
        if let Some(block) = &item.block {
            // A seed's crop, a block of another name: the Java mod places it when it runs.
            differences.push(format!("places {block}: only where its Java mod runs"));
        }
        if !item.class.starts_with(VANILLA_PACKAGE) {
            differences.push(format!("class {} (behaviour beyond Item)", item.class));
        }
        let food = item.food.is_some() || item.components.contains_key("minecraft:food");
        // Food on bread; else the vanilla item of its class (a sword on an iron sword).
        let template = if food {
            "minecraft:bread"
        } else {
            item_template(item)
        };
        let mut decl = json!({"template": template});
        let own = id.split_once(':').map_or("", |(ns, _)| ns);
        // 26.3 components as the bridge's exporter writes them, carried as they are.
        let mut components = Map::new();
        for (name, value) in &item.components {
            if !CARRIED_COMPONENTS.contains(&name.as_str()) {
                continue;
            }
            let mut value = value.clone();
            // Effects of the mod's own (Farmer's Delight's Comfort...) are left out, the rest kept.
            if let Some(Value::Array(effects)) = value.get_mut("on_consume_effects") {
                let before = effects.len();
                effects.retain(|e| known_ids(e, ""));
                if effects.len() < before {
                    differences.push(format!(
                        "{name}: {} effect(s) of the mod's own",
                        before - effects.len()
                    ));
                }
            }
            if known_ids(&value, own) {
                components.insert(name.clone(), value);
            } else {
                differences.push(format!("{name}: names what Horizon does not have"));
            }
        }
        if food && !components.contains_key("minecraft:food") {
            return Err("food without its components (an older exporter)".into());
        }
        if item.max_stack_size != 64 {
            decl["max_stack_size"] = json!(item.max_stack_size);
        }
        if item.rarity != "common" {
            decl["rarity"] = json!(item.rarity);
        }
        if let Some(armor) = item.armor.as_ref().filter(|a| !a.material.is_empty())
            && !self.armor_asset_exists(armor, owned)
        {
            differences.push(format!(
                "armour material {} has no equipment asset: it draws the template's",
                armor.material
            ));
        }
        // The component patch over the template's, from the numbers 1.21.1's exporter keeps
        // beside its components: over the carried ones where both say.
        components.extend(self.item_components(item, template, owned));
        if !components.is_empty() {
            decl["components"] = Value::Object(components);
        }
        Ok((decl, differences))
    }

    /// The item's numbers as a 26.3 component patch over `template`'s, leaving out what already matches.
    fn armor_asset_exists(&self, armor: &Armor, owned: &[&str]) -> bool {
        armor.asset_carried(owned)
            || self.vanilla_items.values().any(|i| {
                i["minecraft:equippable"]["asset_id"].as_str() == Some(armor.material.as_str())
            })
    }

    fn item_components(&self, item: &Item, template: &str, owned: &[&str]) -> Map<String, Value> {
        if item.components.is_empty() {
            return Map::new();
        }
        let base = self.vanilla_items.get(template);
        let mut want = Map::new();
        want.insert(
            "minecraft:max_damage".into(),
            item.components
                .get("minecraft:max_damage")
                .cloned()
                .unwrap_or(Value::Null),
        );
        if let Some(m) = &item.attribute_modifiers {
            want.insert("minecraft:attribute_modifiers".into(), modifiers_26_3(m));
        }
        // The mod's rules as they are; what 1.21.1 kept in the class (can_destroy_blocks_in_creative) stays the template's.
        let tool = item.components.get("minecraft:tool").map(|t| {
            let mut tool = base
                .and_then(|b| b.get("minecraft:tool"))
                .cloned()
                .unwrap_or_else(|| json!({}));
            for key in ["rules", "default_mining_speed", "damage_per_block"] {
                match t.get(key) {
                    Some(v) => tool[key] = v.clone(),
                    None => _ = tool.as_object_mut().map(|o| o.remove(key)),
                }
            }
            tool
        });
        want.insert("minecraft:tool".into(), tool.unwrap_or(Value::Null));
        if let Some(value) = item.enchantment_value {
            let enchantable = (value > 0).then(|| json!({"value": value}));
            want.insert(
                "minecraft:enchantable".into(),
                enchantable.unwrap_or(Value::Null),
            );
        }
        if let Some(repair) = &item.repair_items {
            let items = match repair.as_slice() {
                [] => Value::Null,
                [one] => json!({"items": one}),
                many => json!({"items": many}),
            };
            want.insert("minecraft:repairable".into(), items);
        }
        if let Some(armor) = item.armor.as_ref().filter(|a| !a.material.is_empty()) {
            let mut equippable = base.and_then(|b| b.get("minecraft:equippable")).cloned();
            if self.armor_asset_exists(armor, owned) {
                equippable.get_or_insert_with(|| json!({"slot": armor.slot}))["asset_id"] =
                    json!(armor.material);
            }
            if let Some(mut equippable) = equippable {
                if let Some(sound) = &armor.equip_sound {
                    equippable["equip_sound"] = json!(sound);
                }
                want.insert("minecraft:equippable".into(), equippable);
            }
        }
        let mut patch = Map::new();
        for (key, value) in want {
            let had = base.and_then(|b| b.get(&key));
            match (value, had) {
                (Value::Null, None) => {}
                (Value::Null, Some(_)) => _ = patch.insert(format!("!{key}"), json!({})),
                (v, Some(h)) if same(&v, h) => {}
                (v, _) => _ = patch.insert(key, v),
            }
        }
        patch
    }

    /// A tag's members the content mod may name: vanilla's and its own.
    pub fn tag_members(&self, kind: &str, tag: &str, own: &str) -> Option<Vec<String>> {
        let map = if kind == "block" {
            &self.tags.block
        } else {
            &self.tags.item
        };
        let members = map.get(tag)?;
        Some(
            members
                .iter()
                .filter(|m| {
                    m.split_once(':')
                        .is_some_and(|(ns, _)| ns == "minecraft" || ns == own)
                })
                .cloned()
                .collect(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(hardness: f32, sound: &str, outline: Value) -> Value {
        json!({"hardness": hardness, "emission": 0, "requires_tool": false, "map_color_id": 13,
            "sound": {"break": sound}, "outline": outline, "collision": outline})
    }
    fn block(class: &[&str], props: Value, states: Value) -> Value {
        let default = states
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap()
            .to_owned();
        json!({"class": class, "properties": props, "default": default, "item": "x",
            "explosion_resistance": 3.0, "friction": 0.6, "speed_factor": 1.0,
            "jump_factor": 1.0, "states": states})
    }

    #[test]
    fn templates_follow_class_shapes_and_sounds() {
        let cube = json!([[0, 0, 0, 1, 1, 1]]);
        let low = json!([[0, 0, 0, 1, 0.9375, 1]]);
        let block_class = ["net.minecraft.world.level.block.Block"];
        let path = ["net.minecraft.world.level.block.DirtPathBlock"];
        let facts: Facts = serde_json::from_value(json!({
            "blocks": {
                "minecraft:stone": block(&block_class, json!({}), json!({"": state(1.5, "stone", cube.clone())})),
                "minecraft:oak_planks": block(&block_class, json!({}), json!({"": state(2.0, "wood", cube.clone())})),
                "minecraft:dirt_path": block(&path, json!({}), json!({"": state(0.65, "grass", low.clone())})),
                "m:wood_tile": block(&["m.Tile", block_class[0]], json!({}), json!({"": state(2.5, "wood", cube.clone())})),
                "m:path": block(&["m.Path", block_class[0]], json!({}), json!({"": state(1.0, "stone", low.clone())})),
                "m:gadget": block(&["m.Gadget", block_class[0]], json!({"mode": ["a", "b"]}),
                    json!({"mode=a": state(1.0, "stone", cube.clone()), "mode=b": state(1.0, "stone", cube)})),
            },
            "items": {}
        }))
        .unwrap();
        let any = |_: &str| true;
        let tile = facts.template("m:wood_tile", &any).unwrap();
        assert_eq!(tile.template, "minecraft:oak_planks");
        assert_eq!(tile.properties["hardness"], json!(2.5));
        assert!(!tile.properties.contains_key("sounds"));
        assert_eq!(
            tile.differences,
            ["class m.Tile (behaviour beyond net.minecraft.world.level.block.Block)"]
        );
        // A plain Block is never templated on a DirtPathBlock: shapes differ, and say so.
        let path = facts.template("m:path", &any).unwrap();
        assert_eq!(path.template, "minecraft:stone");
        assert!(
            path.differences[0].starts_with("shapes differ"),
            "{:?}",
            path.differences
        );
        assert_eq!(facts.template("m:gadget", &any), None);
        let tile = facts
            .template("m:wood_tile", &|n: &str| n != "minecraft:oak_planks")
            .unwrap();
        assert_eq!(tile.template, "minecraft:stone");
        assert_eq!(tile.differences[0], "sounds: no vanilla block has them");
    }

    #[test]
    fn own_states_carry_the_boxes_their_stand_in_lacks() {
        // A basin: open on top, its floor short of the block's edges, so it hides only the
        // middle of the floor's face under it. Its nearest vanilla state is a full cube, whose
        // occlusion hid the whole face (the sky showed through the gap round the basin's feet).
        let cube = json!([[0, 0, 0, 1, 1, 1]]);
        let basin = json!([
            [0.125, 0, 0.125, 0.875, 0.125, 0.875],
            [0, 0.125, 0, 1, 1, 1]
        ]);
        let with_occlusion = |mut s: Value, occlusion: &Value| {
            s["occlusion"] = occlusion.clone();
            s
        };
        let class = ["net.minecraft.world.level.block.Block"];
        let facts: Facts = serde_json::from_value(json!({
            "blocks": {
                "minecraft:stone": block(&class, json!({}), json!({"": with_occlusion(state(1.5, "stone", cube.clone()), &cube)})),
                "m:basin": block(&["m.Basin", class[0]], json!({"facing": ["down", "north"]}), json!({
                    "facing=down": with_occlusion(state(1.5, "stone", basin.clone()), &basin),
                    "facing=north": with_occlusion(state(1.5, "stone", cube.clone()), &json!([]))})),
            },
            "items": {}
        }))
        .unwrap();
        let (_, states) = facts.own_states("m:basin", &|_: &str| true).unwrap();
        let by = |facing: &str| {
            states
                .as_array()
                .unwrap()
                .iter()
                .find(|s| s["properties"]["facing"] == json!(facing))
                .unwrap()
                .clone()
        };
        let down = by("down");
        assert_eq!(down["shape"], json!("minecraft:stone[]"));
        assert_eq!(down["outline"], basin);
        assert_eq!(down["collision"], basin);
        assert_eq!(down["occlusion"], basin);
        // Stone's shapes exactly, but it occludes nothing (a noOcclusion block): only that.
        let north = by("north");
        assert!(north.get("outline").is_none() && north.get("collision").is_none());
        assert_eq!(north["occlusion"], json!([]));
        // Many states: the table's shape entries carry the boxes.
        let table = compact_states(vec![down.clone(); STATE_TABLE_FROM]);
        assert_eq!(table["shapes"][0]["like"], json!("minecraft:stone[]"));
        assert_eq!(table["shapes"][0]["occlusion"], basin);
    }

    #[test]
    fn drops_itself_is_vanillas_drop_self() {
        let cube = json!([[0, 0, 0, 1, 1, 1]]);
        let with = |requires_tool: bool, drops: Value| {
            let mut s = state(1.5, "stone", cube.clone());
            s["requires_tool"] = json!(requires_tool);
            s["drops"] = drops;
            let b: Block = serde_json::from_value(block(&[], json!({}), json!({"": s}))).unwrap();
            b.drops_itself("m:tile")
        };
        let me = json!({"m:tile": 1});
        let always = Some(SelfDrop::Always);
        assert_eq!(with(false, json!({"hand": me, "shears": me})), always);
        // A state that needs a tool drops nothing by hand, as dropSelf does.
        assert_eq!(
            with(true, json!({"hand": {}, "diamond_pickaxe": me})),
            always
        );
        assert_eq!(
            with(false, json!({"hand": {}, "diamond_pickaxe": me})),
            None
        );
        // Two of itself (a double slab), or something else, is not dropSelf.
        assert_eq!(with(false, json!({"hand": {"m:tile": 2}})), None);
        assert_eq!(with(false, json!({"hand": {"m:other": 1}})), None);
        assert_eq!(with(false, json!({})), None);
    }

    #[test]
    fn a_door_drops_from_its_lower_half() {
        let cube = json!([[0, 0, 0, 1, 1, 1]]);
        let me = json!({"hand": {"m:door": 1}, "shears": {"m:door": 1}});
        let none = json!({"hand": {}, "shears": {}});
        let door = |upper: &Value| {
            let mut states = serde_json::Map::new();
            for (key, drops) in [
                ("half=lower,open=false", &me),
                ("half=lower,open=true", &me),
                ("half=upper,open=false", upper),
                ("half=upper,open=true", upper),
            ] {
                let mut s = state(3.0, "wood", cube.clone());
                s["drops"] = drops.clone();
                states.insert(key.to_owned(), s);
            }
            let props = json!({"half": ["upper", "lower"], "open": ["true", "false"]});
            let b: Block =
                serde_json::from_value(block(&[], props, Value::Object(states))).unwrap();
            b.drops_itself("m:door")
        };
        assert_eq!(
            door(&none),
            Some(SelfDrop::When("half".to_owned(), "lower".to_owned()))
        );
        assert_eq!(door(&me), Some(SelfDrop::Always));
        // Something else from the upper half: not a door table.
        assert_eq!(door(&json!({"hand": {"m:other": 1}})), None);
    }

    #[test]
    fn items_of_vanilla_classes_take_their_template_and_numbers() {
        let mut facts: Facts = serde_json::from_value(json!({
            "blocks": {},
            "items": {
                "m:sword": {"class": "net.minecraft.world.item.SwordItem", "max_stack_size": 1, "rarity": "common",
                    "components": {"minecraft:max_damage": 905, "minecraft:tool": {"rules": [
                        {"blocks": "minecraft:cobweb", "speed": 15.0, "correct_for_drops": true}], "damage_per_block": 2}},
                    "attribute_modifiers": {"modifiers": [{"type": "minecraft:generic.attack_damage",
                        "id": "minecraft:base_attack_damage", "amount": 5.5, "operation": "add_value", "slot": "mainhand"}]},
                    "enchantment_value": 14, "repair_items": ["minecraft:emerald"]},
                "m:boots": {"class": "net.minecraft.world.item.ArmorItem", "max_stack_size": 1, "rarity": "common",
                    "components": {"minecraft:max_damage": 1}, "enchantment_value": 0, "repair_items": [],
                    "armor": {"slot": "feet"}},
                "m:gem": {"class": "net.minecraft.world.item.Item", "max_stack_size": 64, "rarity": "common"},
            }
        }))
        .unwrap();
        facts.vanilla_items = serde_json::from_value(json!({
            "minecraft:iron_sword": {"minecraft:max_damage": 250, "minecraft:enchantable": {"value": 14},
                "minecraft:repairable": {"items": "#minecraft:iron_tool_materials"},
                "minecraft:tool": {"can_destroy_blocks_in_creative": false, "damage_per_block": 2,
                    "rules": [{"blocks": "#minecraft:sword_efficient", "speed": 1.5}]}},
            "minecraft:iron_boots": {"minecraft:max_damage": 195, "minecraft:enchantable": {"value": 9},
                "minecraft:repairable": {"items": "#minecraft:repairs_iron_armor"}},
        }))
        .unwrap();
        let (sword, _) = facts.item("m:sword", &["m"]).unwrap();
        assert_eq!(sword["template"], "minecraft:iron_sword");
        let c = &sword["components"];
        assert_eq!(c["minecraft:max_damage"], 905);
        assert_eq!(
            c["minecraft:attribute_modifiers"][0]["type"],
            "minecraft:attack_damage"
        );
        assert_eq!(
            c["minecraft:repairable"],
            json!({"items": "minecraft:emerald"})
        );
        // The mod's rules replace the template's; the creative flag the class gave stays.
        assert_eq!(
            c["minecraft:tool"],
            json!({"can_destroy_blocks_in_creative": false, "damage_per_block": 2,
                "rules": [{"blocks": "minecraft:cobweb", "speed": 15.0, "correct_for_drops": true}]})
        );
        // Equal to the template's: left out.
        assert!(c.get("minecraft:enchantable").is_none());
        let (boots, _) = facts.item("m:boots", &["m"]).unwrap();
        assert_eq!(boots["template"], "minecraft:iron_boots");
        assert_eq!(boots["components"]["!minecraft:enchantable"], json!({}));
        assert_eq!(boots["components"]["!minecraft:repairable"], json!({}));
        // Facts without components (exported before they were) keep the plain template.
        let (gem, _) = facts.item("m:gem", &["m"]).unwrap();
        assert_eq!(gem, json!({"template": "minecraft:stick"}));
    }

    #[test]
    fn armour_materials_become_equipment_assets() {
        let armor: Armor = serde_json::from_value(json!({"slot": "legs", "material": "m:ruby",
            "equip_sound": "minecraft:item.armor.equip_diamond", "layers": [
                {"texture": "m:textures/models/armor/ruby_layer_1.png",
                 "texture_legs": "m:textures/models/armor/ruby_layer_2.png", "dyeable": false},
                {"texture": "m:textures/models/armor/ruby_overlay_layer_1.png",
                 "texture_legs": "m:textures/models/armor/ruby_overlay_layer_2.png", "dyeable": true}]}))
        .unwrap();
        let (asset, copies) = armor.equipment().unwrap();
        assert_eq!(
            asset["layers"]["humanoid_leggings"],
            json!([{"texture": "m:ruby"}, {"texture": "m:ruby_overlay", "dyeable": {}}])
        );
        assert_eq!(
            copies[2],
            (
                "assets/m/textures/models/armor/ruby_layer_2.png".to_owned(),
                "assets/m/textures/entity/equipment/humanoid_leggings/ruby.png".to_owned()
            )
        );
        let mut facts: Facts =
            serde_json::from_value(json!({"blocks": {}, "items": {"m:ruby_leggings": {
            "class": "net.minecraft.world.item.ArmorItem", "max_stack_size": 1, "rarity": "common",
            "components": {"minecraft:max_damage": 300}, "armor": armor_json()}}}))
            .unwrap();
        facts.vanilla_items = serde_json::from_value(json!({"minecraft:iron_leggings": {
            "minecraft:equippable": {"asset_id": "minecraft:iron", "equip_sound": "minecraft:item.armor.equip_iron", "slot": "legs"}}}))
        .unwrap();
        let (decl, _) = facts.item("m:ruby_leggings", &["m"]).unwrap();
        assert_eq!(
            decl["components"]["minecraft:equippable"],
            json!({"asset_id": "m:ruby", "equip_sound": "minecraft:item.armor.equip_diamond", "slot": "legs"})
        );
        // Layers outside models/armor/ have no 26.3 place to go.
        let odd: Armor =
            serde_json::from_value(json!({"slot": "head", "material": "m:odd", "layers": [
            {"texture": "m:textures/odd.png", "texture_legs": "m:textures/odd.png"}]}))
            .unwrap();
        assert!(odd.equipment().is_none());
    }

    #[test]
    fn armour_only_names_an_equipment_asset_that_exists() {
        let leggings = |material: &str, layers: Value| {
            json!({"class": "net.minecraft.world.item.ArmorItem", "max_stack_size": 1, "rarity": "common",
                "components": {"minecraft:max_damage": 300},
                "armor": {"slot": "legs", "material": material,
                    "equip_sound": "minecraft:item.armor.equip_diamond", "layers": layers}})
        };
        let odd = json!([{"texture": "m:textures/odd.png", "texture_legs": "m:textures/odd.png"}]);
        let mut facts: Facts = serde_json::from_value(json!({"blocks": {}, "items": {
            "m:ruby": leggings("m:ruby", json!([])),
            "m:odd": leggings("m:odd", odd),
            "m:other": leggings("other:steel", json!([])),
            "m:unknown": leggings("minecraft:steel", json!([])),
            "m:diamond": leggings("minecraft:diamond", json!([]))}}))
        .unwrap();
        facts.vanilla_items = serde_json::from_value(json!({
            "minecraft:iron_leggings": {"minecraft:equippable": {"asset_id": "minecraft:iron", "slot": "legs"}},
            "minecraft:diamond_leggings": {"minecraft:equippable": {"asset_id": "minecraft:diamond", "slot": "legs"}}}))
        .unwrap();
        let asset = |id: &str| {
            let (decl, differences) = facts.item(id, &["m"]).unwrap();
            let asset = decl["components"]["minecraft:equippable"]["asset_id"].clone();
            (asset, !differences.is_empty())
        };
        assert_eq!(asset("m:ruby"), (json!("m:ruby"), false));
        assert_eq!(asset("m:diamond"), (json!("minecraft:diamond"), false));
        for id in ["m:odd", "m:other", "m:unknown"] {
            assert_eq!(asset(id), (json!("minecraft:iron"), true), "{id}");
        }
        // Without the game's tables the template's own component stays whole.
        facts.vanilla_items.clear();
        let (decl, _) = facts.item("m:odd", &["m"]).unwrap();
        assert!(decl["components"].get("minecraft:equippable").is_none());
    }

    fn armor_json() -> Value {
        json!({"slot": "legs", "material": "m:ruby", "equip_sound": "minecraft:item.armor.equip_diamond"})
    }

    #[test]
    fn many_own_states_fit_the_content_fence_and_read_back() {
        // A pipe: six sides of four kinds and a light level, 4^6 * 4 = 16,384 states, with
        // a handful of shapes (ALA Industrial's have 76,832, 49 shapes).
        let kinds = ["none", "pipe", "machine", "blocked"];
        let sides = ["down", "east", "north", "south", "up", "west"];
        let mut states = Vec::new();
        for n in 0..4usize.pow(7) {
            let mut properties = Map::new();
            for (i, side) in sides.iter().enumerate() {
                properties.insert((*side).into(), json!(kinds[n >> (2 * i) & 3]));
            }
            properties.insert("power".into(), json!((n >> 12).to_string()));
            let mut v = json!({"properties": properties, "shape": format!("minecraft:cake[bites={}]", n % 7)});
            if n >> 12 == 3 {
                v["light"] = json!(9);
            }
            if n == 5 {
                v["default"] = json!(true);
            }
            states.push(v);
        }
        let array = serde_json::to_vec(&Value::Array(states.clone())).unwrap();
        let table = compact_states(states.clone());
        assert!(table.is_object());
        let file =
            serde_json::to_vec(&json!({"template": "minecraft:stone", "state_table": table}))
                .unwrap();
        assert!(
            file.len() * 4 < array.len(),
            "{} vs {}",
            file.len(),
            array.len()
        );
        let manifest = "id = \"pipes\"\nname = \"Pipes\"\nversion = \"0.1.0\"\napi = \"0.2\"\nside = \"both\"\ncapabilities = [\"registries\"]\n";
        let dir = std::env::temp_dir().join(format!("horizon-state-table-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("m.hmod");
        let files: &[(&str, &[u8])] = &[("data/pipes/horizon/block/pipe.json", &file)];
        std::fs::write(
            &path,
            horizon_mods::Package::write_files(manifest, None, files).unwrap(),
        )
        .unwrap();
        let package = horizon_mods::Package::read(&path).unwrap();
        let content = horizon_mods::Content::read(&path, &package.manifest).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let read = &content.blocks[0].states;
        assert_eq!(read.len(), states.len());
        for (r, s) in read.iter().zip(&states) {
            let props: BTreeMap<String, String> =
                serde_json::from_value(s["properties"].clone()).unwrap();
            assert_eq!(r.properties, props);
            assert_eq!(r.shape.as_deref(), s["shape"].as_str());
            assert_eq!(r.light, s.get("light").map(|l| l.as_u64().unwrap() as u8));
            assert_eq!(r.default, s.get("default").is_some());
        }
        // Few states stay an array.
        assert!(compact_states(states[..10].to_vec()).is_array());
    }

    #[test]
    fn state_keys_sort() {
        assert_eq!(
            state_key("half=lower,facing=north"),
            "facing=north,half=lower"
        );
        assert_eq!(state_key(""), "");
    }
}
