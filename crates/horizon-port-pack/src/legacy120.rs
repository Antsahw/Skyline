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
}
