//! Data files of mods made for 1.20.x (Fabric 1.20.1, the bridge's 1.20.1 runtime:
//! docs/java-bridge-120.md) in 1.21.1's shape, before the carry-over upgrades 1.21.1's to 26.3's.
//!
//! 1.21 renamed the data pack folders to the singular (`recipes/` is `recipe/`, `tags/blocks/`
//! is `tags/block/`), and 1.20.5 changed what is in them: a recipe's result is an item stack
//! (`{"id": ...}`, where 1.20.1 wrote `{"item": ...}` or, for smelting and stonecutting, a bare
//! id), and a loot table's tool match names its items as one holder set and its enchantments as a
//! sub-predicate. A jar's files in the old folders are read as 1.20.x's: 1.21 and later never
//! read those folders, so the old name is what says which version wrote the file.

use serde_json::{Map, Value, json};

/// 1.20.x's data folders and the names 1.21 gave them.
const FOLDERS: [(&str, &str); 7] = [
    ("recipes/", "recipe/"),
    ("loot_tables/", "loot_table/"),
    ("advancements/", "advancement/"),
    ("functions/", "function/"),
    ("structures/", "structure/"),
    ("predicates/", "predicate/"),
    ("item_modifiers/", "item_modifier/"),
];

/// 1.20.x's tag folders and 1.21's.
const TAG_FOLDERS: [(&str, &str); 5] = [
    ("tags/blocks/", "tags/block/"),
    ("tags/items/", "tags/item/"),
    ("tags/entity_types/", "tags/entity_type/"),
    ("tags/fluids/", "tags/fluid/"),
    ("tags/game_events/", "tags/game_event/"),
];

/// A jar entry's path in 1.21's folders when it is in one of 1.20.x's: `None` when it isn't.
pub fn data_path(name: &str) -> Option<String> {
    let rest = name.strip_prefix("data/")?;
    let (ns, path) = rest.split_once('/')?;
    FOLDERS
        .iter()
        .chain(TAG_FOLDERS.iter())
        .find_map(|(old, new)| {
            path.strip_prefix(old)
                .map(|p| format!("data/{ns}/{new}{p}"))
        })
}

/// A 1.20.x recipe's result in 1.21.1's shape, in place: `{"item": id, "count": n}` is
/// `{"id": id, "count": n}`; smelting's bare id is `{"id": id}`; stonecutting's id with a
/// `count` beside it is `{"id": id, "count": n}`. Whether anything changed.
pub fn recipe(v: &mut Value) -> bool {
    let Some(o) = v.as_object_mut() else {
        return false;
    };
    let count = o.get("count").and_then(Value::as_i64);
    let changed = match o.get_mut("result") {
        Some(Value::String(id)) => {
            let mut stack = Map::new();
            stack.insert("id".into(), json!(id));
            if let Some(n) = count.filter(|&n| n != 1) {
                stack.insert("count".into(), json!(n));
            }
            o.insert("result".into(), Value::Object(stack));
            true
        }
        Some(Value::Object(stack)) => match stack.remove("item") {
            Some(id) => {
                stack.insert("id".into(), id);
                true
            }
            None => false,
        },
        _ => false,
    };
    // Stonecutting's count went into the result.
    if changed && o.get("type").and_then(Value::as_str) == Some("minecraft:stonecutting") {
        o.remove("count");
    }
    changed
}

/// A 1.20.x loot table (or any part of one) in 1.21.1's shape, in place: every `match_tool`
/// predicate (`items` list or `tag` as one `items`; `enchantments` as the
/// `minecraft:enchantments` sub-predicate) and the renamed NBT functions.
pub fn loot(v: &mut Value) {
    match v {
        Value::Array(a) => a.iter_mut().for_each(loot),
        Value::Object(o) => {
            let condition = o
                .get("condition")
                .and_then(Value::as_str)
                .map(str::to_owned);
            if matches!(
                condition.as_deref(),
                Some("minecraft:match_tool" | "match_tool")
            ) && let Some(p) = o.get_mut("predicate")
            {
                item_predicate(p);
            }
            // 1.20.5: a loot_table entry names its table as `value`.
            let table_entry = o
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| matches!(t, "minecraft:loot_table" | "loot_table"));
            if table_entry
                && !o.contains_key("value")
                && let Some(name) = o.remove("name")
            {
                o.insert("value".into(), name);
            }
            if let Some(Value::String(f)) = o.get_mut("function") {
                match f.as_str() {
                    // 1.20.5: an item's custom NBT is its custom_data component.
                    "minecraft:set_nbt" | "set_nbt" => *f = "minecraft:set_custom_data".into(),
                    "minecraft:copy_nbt" | "copy_nbt" => *f = "minecraft:copy_custom_data".into(),
                    _ => {}
                }
            }
            o.values_mut().for_each(loot);
        }
        _ => {}
    }
}

/// A 1.20.x item predicate in 1.21.1's shape, in place.
fn item_predicate(p: &mut Value) {
    let Some(o) = p.as_object_mut() else {
        return;
    };
    // 1.20.x: `items` a list of ids, `tag` apart; 1.21: one holder set, an id, a list or a tag.
    if let Some(tag) = o.remove("tag").and_then(|t| t.as_str().map(str::to_owned)) {
        o.insert("items".into(), json!(format!("#{tag}")));
    } else if let Some(Value::Array(items)) = o.get("items")
        && items.len() == 1
    {
        let one = items[0].clone();
        o.insert("items".into(), one);
    }
    for (old, component) in [
        ("enchantments", "minecraft:enchantments"),
        ("stored_enchantments", "minecraft:stored_enchantments"),
    ] {
        let Some(Value::Array(list)) = o.remove(old) else {
            continue;
        };
        let upgraded: Vec<Value> = list
            .into_iter()
            .map(|mut e| {
                if let Some(eo) = e.as_object_mut()
                    && let Some(id) = eo.remove("enchantment")
                {
                    eo.insert("enchantments".into(), id);
                }
                e
            })
            .collect();
        let predicates = o
            .entry("predicates")
            .or_insert_with(|| json!({}))
            .as_object_mut();
        if let Some(predicates) = predicates {
            predicates.insert(component.into(), Value::Array(upgraded));
        }
    }
}

/// 1.20.x's int and float providers in 1.20.5's shape, everywhere in a worldgen file, in place:
/// 1.20.x put a provider's bounds in a `value` object (`{"type": "uniform", "value":
/// {"min_inclusive": 1, "max_inclusive": 3}}`), 1.20.5 beside its type. A constant's `value` is
/// a number and stays. Whether anything changed.
pub fn providers(v: &mut Value) -> bool {
    const NESTED: [&str; 5] = [
        "uniform",
        "biased_to_bottom",
        "clamped",
        "clamped_normal",
        "trapezoid",
    ];
    match v {
        Value::Array(a) => a.iter_mut().fold(false, |c, x| providers(x) | c),
        Value::Object(o) => {
            let mut changed = false;
            let nested = o
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| NESTED.contains(&t.strip_prefix("minecraft:").unwrap_or(t)));
            if nested
                && o.len() == 2
                && let Some(Value::Object(inner)) = o.remove("value")
            {
                o.extend(inner);
                changed = true;
            }
            o.values_mut().fold(changed, |c, x| providers(x) | c)
        }
        _ => false,
    }
}

/// A 1.20.x biome's look and sound in 26.3's place, in place: 1.21.2 to 26.1 moved what
/// `effects` held but the water and plant colours into environment attributes (the sky, fog and
/// underwater fog colours as `#rrggbb`, the ambient particle, sounds and music). A colour equal to
/// the overworld's default is left out, as vanilla's own biomes leave it to the dimension.
/// 1.20.x's carvers by carving step are 1.21.2's one list, the air step's: the liquid step was
/// gone. Whether anything changed.
pub fn biome(v: &mut Value) -> bool {
    let mut carved = false;
    if let Some(Value::Object(steps)) = v.get("carvers") {
        let air = steps.get("air").cloned().unwrap_or(json!([]));
        v["carvers"] = match air {
            Value::String(one) => json!([one]),
            air => air,
        };
        carved = true;
    }
    let Some(effects) = v.get_mut("effects").and_then(Value::as_object_mut) else {
        return carved;
    };
    let mut attributes = Map::new();
    let hex = |c: &Value| c.as_i64().map(|c| json!(format!("#{:06x}", c & 0xFF_FFFF)));
    for (key, attribute, default) in [
        ("sky_color", "minecraft:visual/sky_color", None),
        ("fog_color", "minecraft:visual/fog_color", Some(0xC0D8FF)),
        (
            "water_fog_color",
            "minecraft:visual/water_fog_color",
            Some(0x050533),
        ),
    ] {
        if let Some(color) = effects.remove(key)
            && color.as_i64() != default
            && let Some(color) = hex(&color).or(Some(color).filter(Value::is_string))
        {
            attributes.insert(attribute.into(), color);
        }
    }
    if let Some(Value::Object(particle)) = effects.remove("particle") {
        let entry = json!({"particle": particle.get("options").cloned().unwrap_or_default(),
            "probability": particle.get("probability").cloned().unwrap_or(json!(0.0))});
        attributes.insert(
            "minecraft:visual/ambient_particles".into(),
            json!({"modifier": "append", "argument": [entry]}),
        );
    }
    let mut sounds = Map::new();
    if let Some(sound) = effects.remove("ambient_sound") {
        sounds.insert("loop".into(), sound);
    }
    if let Some(mood) = effects.remove("mood_sound") {
        sounds.insert("mood".into(), mood);
    }
    if let Some(additions) = effects.remove("additions_sound") {
        sounds.insert("additions".into(), additions);
    }
    if !sounds.is_empty() {
        attributes.insert(
            "minecraft:audio/ambient_sounds".into(),
            Value::Object(sounds),
        );
    }
    if let Some(Value::Object(mut music)) = effects.remove("music") {
        if music.get("replace_current_music") == Some(&json!(false)) {
            music.remove("replace_current_music");
        }
        attributes.insert(
            "minecraft:audio/background_music".into(),
            json!({"default": music}),
        );
    }
    if attributes.is_empty() {
        return carved;
    }
    let Some(o) = v.as_object_mut() else {
        return carved;
    };
    match o.entry("attributes").or_insert_with(|| json!({})) {
        Value::Object(existing) => {
            for (k, x) in attributes {
                existing.entry(k).or_insert(x);
            }
        }
        other => *other = Value::Object(attributes),
    }
    true
}

/// A surface rule's biome conditions with only the biomes that exist, in place: 1.20.x read
/// `biome_is` as biome keys and let one name a biome nothing registers (Terralith names some it
/// never made), where 26.3 reads a biome list and refuses the whole surface rule over it. An id
/// without a namespace is vanilla's. Whether anything changed.
pub fn biome_conditions(v: &mut Value, exists: &dyn Fn(&str) -> bool) -> bool {
    match v {
        Value::Array(a) => a
            .iter_mut()
            .fold(false, |c, x| biome_conditions(x, exists) | c),
        Value::Object(o) => {
            let mut changed = false;
            let biome = o
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| t.strip_prefix("minecraft:").unwrap_or(t) == "biome");
            if biome && let Some(Value::Array(names)) = o.get_mut("biome_is") {
                let before = names.clone();
                names.retain_mut(|n| match n.as_str() {
                    Some(id) => {
                        let full = if id.contains(':') {
                            id.to_owned()
                        } else {
                            format!("minecraft:{id}")
                        };
                        let keep = exists(&full);
                        *n = Value::String(full);
                        keep
                    }
                    None => false,
                });
                changed = *names != before;
            }
            o.values_mut()
                .fold(changed, |c, x| biome_conditions(x, exists) | c)
        }
        _ => false,
    }
}

/// Where each feature comes in its generation step, by the order `biomes` (biome files, in
/// order) give their features: the order vanilla's feature sorter settles on, for 26.3's own
/// biomes. A feature found in no biome has none. Empty when their lists contradict each other.
pub fn feature_order(biomes: &[Value]) -> std::collections::HashMap<(usize, String), usize> {
    use std::collections::{BTreeMap, BTreeSet, HashMap};
    type Node = (usize, String);
    let mut after: BTreeMap<Node, BTreeSet<Node>> = BTreeMap::new();
    let mut before: BTreeMap<Node, usize> = BTreeMap::new();
    for biome in biomes {
        let mut previous: Option<Node> = None;
        let steps = biome["features"].as_array().into_iter().flatten();
        for (step, features) in steps.enumerate() {
            for id in features
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
            {
                let node = (step, id.to_owned());
                before.entry(node.clone()).or_default();
                after.entry(node.clone()).or_default();
                if let Some(previous) = previous.take()
                    && previous != node
                    && after.entry(previous).or_default().insert(node.clone())
                {
                    *before.entry(node.clone()).or_default() += 1;
                }
                previous = Some(node);
            }
        }
    }
    // Kahn's, smallest first: the same order for the same lists.
    let mut ready: BTreeSet<Node> = before
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(k, _)| k.clone())
        .collect();
    let mut order = HashMap::new();
    while let Some(node) = ready.pop_first() {
        for next in after.get(&node).into_iter().flatten() {
            let n = before.get_mut(next).expect("counted");
            *n -= 1;
            if *n == 0 {
                ready.insert(next.clone());
            }
        }
        let rank = order.len();
        order.insert(node, rank);
    }
    if order.len() != before.len() {
        return HashMap::new();
    }
    order
}

/// A 1.20.x biome's features put in `order`'s order ([`feature_order`]: 26.3's vanilla biomes'),
/// in place: 26.3 reordered some of vanilla's (sugar cane after pumpkins), and a biome that keeps
/// 1.20.1's order beside 26.3's own biomes makes the feature sorter find a cycle and the world
/// fail to generate. The features `order` has keep their slots among the others, sorted. Whether
/// anything changed.
pub fn order_features(
    biome: &mut Value,
    order: &std::collections::HashMap<(usize, String), usize>,
) -> bool {
    let mut changed = false;
    let Some(steps) = biome.get_mut("features").and_then(Value::as_array_mut) else {
        return false;
    };
    for (step, features) in steps.iter_mut().enumerate() {
        let Some(features) = features.as_array_mut() else {
            continue;
        };
        let rank = |f: &Value| {
            f.as_str()
                .and_then(|id| order.get(&(step, id.to_owned())).copied())
        };
        let slots: Vec<usize> = (0..features.len())
            .filter(|&i| rank(&features[i]).is_some())
            .collect();
        let mut known: Vec<Value> = slots.iter().map(|&i| features[i].clone()).collect();
        known.sort_by_key(|f| rank(f));
        for (slot, feature) in slots.into_iter().zip(known) {
            if features[slot] != feature {
                features[slot] = feature;
                changed = true;
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_folders_move_and_new_ones_stay() {
        let cases = [
            (
                "data/farmersdelight/recipes/skillet.json",
                Some("data/farmersdelight/recipe/skillet.json"),
            ),
            (
                "data/fd/loot_tables/blocks/rice.json",
                Some("data/fd/loot_table/blocks/rice.json"),
            ),
            (
                "data/c/tags/items/iron_ingots.json",
                Some("data/c/tags/item/iron_ingots.json"),
            ),
            (
                "data/minecraft/tags/blocks/mineable/axe.json",
                Some("data/minecraft/tags/block/mineable/axe.json"),
            ),
            (
                "data/x/structures/house.nbt",
                Some("data/x/structure/house.nbt"),
            ),
            ("data/x/recipe/new.json", None),
            ("data/x/worldgen/biome/b.json", None),
            ("assets/x/recipes/not_data.json", None),
        ];
        for (from, to) in cases {
            assert_eq!(data_path(from).as_deref(), to, "{from}");
        }
    }

    #[test]
    fn recipe_results_become_item_stacks() {
        // Farmer's Delight 1.20.1's skillet and cooked salmon slice, Tech Reborn's slab cutting.
        let mut shaped = json!({"type": "minecraft:crafting_shaped", "key": {"#": {"tag": "c:iron_ingots"}},
            "pattern": ["##"], "result": {"item": "farmersdelight:skillet"}});
        assert!(recipe(&mut shaped));
        assert_eq!(shaped["result"], json!({"id": "farmersdelight:skillet"}));
        let mut smelting = json!({"type": "minecraft:smelting", "ingredient": {"item": "farmersdelight:salmon_slice"},
            "result": "farmersdelight:cooked_salmon_slice", "experience": 0.35, "cookingtime": 200});
        assert!(recipe(&mut smelting));
        assert_eq!(
            smelting["result"],
            json!({"id": "farmersdelight:cooked_salmon_slice"})
        );
        let mut cutting = json!({"type": "minecraft:stonecutting", "count": 2, "ingredient": {"tag": "c:aluminum_blocks"},
            "result": "techreborn:aluminum_storage_block_slab"});
        assert!(recipe(&mut cutting));
        assert_eq!(
            cutting["result"],
            json!({"id": "techreborn:aluminum_storage_block_slab", "count": 2})
        );
        assert!(cutting.get("count").is_none());
        // Already 1.21's: left as it is.
        let mut new =
            json!({"type": "minecraft:crafting_shapeless", "result": {"id": "x:y", "count": 3}});
        assert!(!recipe(&mut new));
        // And the carry-over's own upgrade takes it from there to 26.3's.
        assert!(crate::carry::upgrade_recipe(&mut shaped));
        assert_eq!(shaped["key"]["#"], json!("#c:iron_ingots"));
    }

    #[test]
    fn tool_matches_become_what_the_carry_over_turns_into_26_3_conditions() {
        // Supplementaries 1.20.1's raked gravel and Farmer's Delight's wild potatoes.
        let mut table = json!({"type": "minecraft:block", "pools": [{"rolls": 1, "entries": [
            {"type": "minecraft:alternatives", "children": [
                {"type": "minecraft:item", "name": "supplementaries:raked_gravel", "conditions": [
                    {"condition": "minecraft:match_tool", "predicate": {"enchantments": [
                        {"enchantment": "minecraft:silk_touch", "levels": {"min": 1}}]}}]},
                {"type": "minecraft:item", "name": "farmersdelight:wild_potatoes", "conditions": [
                    {"condition": "minecraft:match_tool", "predicate": {"tag": "c:shears"}}]},
                {"type": "minecraft:item", "name": "minecraft:gravel", "functions": [
                    {"function": "minecraft:set_nbt", "tag": "{x:1}"}],
                 "conditions": [{"condition": "minecraft:match_tool", "predicate": {"items": ["minecraft:shears"]}}]}]}]}]});
        loot(&mut table);
        let children = &table["pools"][0]["entries"][0]["children"];
        assert_eq!(
            children[0]["conditions"][0]["predicate"],
            json!({"predicates": {"minecraft:enchantments": [
                {"enchantments": "minecraft:silk_touch", "levels": {"min": 1}}]}})
        );
        assert_eq!(
            children[1]["conditions"][0]["predicate"],
            json!({"items": "#c:shears"})
        );
        assert_eq!(
            children[2]["conditions"][0]["predicate"],
            json!({"items": "minecraft:shears"})
        );
        assert_eq!(
            children[2]["functions"][0]["function"],
            "minecraft:set_custom_data"
        );
        // The carry-over's 26.3 step then names the silk touch and shears matches.
        crate::carry::upgrade_loot(&mut table);
        let children = &table["pools"][0]["entries"][0]["children"];
        assert_eq!(
            children[0]["condition"],
            json!("minecraft:tool/can_silk_touch")
        );
        assert_eq!(children[2]["condition"], json!("minecraft:tool/can_shear"));
    }

    #[test]
    fn a_providers_bounds_come_out_of_its_value() {
        let mut feature = json!({"type": "minecraft:tree", "config": {
            "foliage_placer": {"radius": {"type": "minecraft:uniform",
                "value": {"min_inclusive": 1, "max_inclusive": 3}}, "offset": 0},
            "count": {"type": "clamped", "value": {"min_inclusive": 0, "max_inclusive": 6,
                "source": {"type": "minecraft:biased_to_bottom",
                    "value": {"min_inclusive": 1, "max_inclusive": 1}}}},
            "height": {"type": "minecraft:constant", "value": 4}}});
        assert!(providers(&mut feature));
        let config = &feature["config"];
        assert_eq!(
            config["foliage_placer"]["radius"],
            json!({"type": "minecraft:uniform", "min_inclusive": 1, "max_inclusive": 3})
        );
        assert_eq!(
            config["count"],
            json!({"type": "clamped", "min_inclusive": 0, "max_inclusive": 6,
                "source": {"type": "minecraft:biased_to_bottom",
                    "min_inclusive": 1, "max_inclusive": 1}})
        );
        // A constant keeps its value, and 1.20.5's shape is left as it is.
        assert_eq!(
            config["height"],
            json!({"type": "minecraft:constant", "value": 4})
        );
        assert!(!providers(&mut feature));
    }

    /// As 26.3's vanilla writes its biomes: swamp's sky and underwater fog, basalt deltas'
    /// particles and sounds; the overworld's default fog left to the dimension.
    #[test]
    fn a_biomes_look_and_sound_become_attributes() {
        let mut swamp = json!({"temperature": 0.8, "carvers": {"air": ["minecraft:cave"]},
            "effects": {
            "sky_color": 7907327, "fog_color": 12638463, "water_color": 6388580,
            "water_fog_color": 2302743, "grass_color_modifier": "swamp",
            "music": {"sound": "minecraft:music.overworld.swamp", "min_delay": 12000,
                "max_delay": 24000, "replace_current_music": false}}});
        assert!(biome(&mut swamp));
        assert_eq!(swamp["carvers"], json!(["minecraft:cave"]));
        assert_eq!(
            swamp["effects"],
            json!({"water_color": 6388580, "grass_color_modifier": "swamp"})
        );
        assert_eq!(
            swamp["attributes"],
            json!({"minecraft:visual/sky_color": "#78a7ff",
                "minecraft:visual/water_fog_color": "#232317",
                "minecraft:audio/background_music": {"default": {
                    "sound": "minecraft:music.overworld.swamp", "min_delay": 12000,
                    "max_delay": 24000}}})
        );
        let mut deltas = json!({"effects": {"fog_color": 6840176,
            "particle": {"options": {"type": "minecraft:white_ash"}, "probability": 0.118093334},
            "ambient_sound": "minecraft:ambient.basalt_deltas.loop",
            "mood_sound": {"sound": "minecraft:ambient.basalt_deltas.mood", "tick_delay": 6000,
                "block_search_extent": 8, "offset": 2.0},
            "additions_sound": {"sound": "minecraft:ambient.basalt_deltas.additions",
                "tick_chance": 0.0111}}});
        assert!(biome(&mut deltas));
        let attributes = &deltas["attributes"];
        assert_eq!(attributes["minecraft:visual/fog_color"], json!("#685f70"));
        assert_eq!(
            attributes["minecraft:visual/ambient_particles"],
            json!({"modifier": "append", "argument": [{"particle": {"type": "minecraft:white_ash"},
                "probability": 0.118093334}]})
        );
        assert_eq!(
            attributes["minecraft:audio/ambient_sounds"],
            json!({"loop": "minecraft:ambient.basalt_deltas.loop",
                "mood": {"sound": "minecraft:ambient.basalt_deltas.mood", "tick_delay": 6000,
                    "block_search_extent": 8, "offset": 2.0},
                "additions": {"sound": "minecraft:ambient.basalt_deltas.additions",
                    "tick_chance": 0.0111}})
        );
        assert!(!biome(&mut deltas), "once");
    }

    #[test]
    fn a_surface_rule_names_only_biomes_that_exist() {
        let mut rule = json!({"type": "minecraft:condition", "if_true": {"type": "minecraft:biome",
            "biome_is": ["terralith:icy_spires", "deep_frozen_ocean", "terralith:yosemite_cliffs"]},
            "then_run": {"type": "minecraft:block", "result_state": {"Name": "minecraft:ice"}}});
        let exists = |id: &str| id != "terralith:icy_spires";
        assert!(biome_conditions(&mut rule, &exists));
        assert_eq!(
            rule["if_true"]["biome_is"],
            json!(["minecraft:deep_frozen_ocean", "terralith:yosemite_cliffs"])
        );
        assert!(!biome_conditions(&mut rule, &exists));
    }

    #[test]
    fn a_biome_takes_vanillas_feature_order_around_its_own() {
        // 26.3's own: pumpkins before sugar cane.
        let vanilla = [
            json!({"features": [[], ["minecraft:patch_pumpkin", "minecraft:patch_sugar_cane"]]}),
            json!({"features": [[], ["minecraft:trees", "minecraft:patch_pumpkin"]]}),
        ];
        let order = feature_order(&vanilla);
        let mut biome = json!({"features": [["mod:rock"], ["minecraft:patch_sugar_cane",
            "mod:flowers", "minecraft:patch_pumpkin", "minecraft:trees"]]});
        assert!(order_features(&mut biome, &order));
        assert_eq!(
            biome["features"],
            json!([
                ["mod:rock"],
                [
                    "minecraft:trees",
                    "mod:flowers",
                    "minecraft:patch_pumpkin",
                    "minecraft:patch_sugar_cane"
                ]
            ])
        );
        assert!(!order_features(&mut biome, &order));
        // Lists that contradict each other give no order.
        let cycle = [
            json!({"features": [["a", "b"]]}),
            json!({"features": [["b", "a"]]}),
        ];
        assert!(feature_order(&cycle).is_empty());
    }
}
