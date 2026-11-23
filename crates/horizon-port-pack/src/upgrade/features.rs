//! Configured and placed features.
use super::{Report, kind, states, vanilla};
use serde_json::{Map, Value, json};

/// Feature types 26.3 dropped for compositions of other features; a pack
/// using them needs vanilla's composition, which the upgrade does not
/// write.
const REPLACED_FEATURES: [&str; 10] = [
    "basalt_columns",
    "basalt_pillar",
    "coral_mushroom",
    "desert_well",
    "glowstone_blob",
    "kelp",
    "sea_pickle",
    "seagrass",
    "twisting_vines",
    "weeping_vines",
];

/// 1.21.1's patch features (`RandomPatchConfiguration`): `flower` and
/// `no_bonemeal_flower` place exactly like `random_patch`.
const PATCHES: [&str; 3] = ["random_patch", "flower", "no_bonemeal_flower"];

/// A patch in 26.3's form, as vanilla's own patches were rewritten: its
/// placed feature tried `tries` times at a trapezoid offset of the spreads
/// (`nextInt(s + 1) - nextInt(s + 1)`), inside a one-entry selector so the
/// configured feature stays one file.
fn patch(config: &Value, report: &mut Report) -> Value {
    let int = |k: &str, d: i64| config.get(k).and_then(Value::as_i64).unwrap_or(d);
    let (tries, xz, y) = (int("tries", 128), int("xz_spread", 7), int("y_spread", 3));
    let trap = |n: i64| json!({"type": "minecraft:trapezoid", "min": -n, "max": n, "plateau": 0});
    let mut placement = vec![
        json!({"type": "minecraft:count", "count": tries}),
        json!({"type": "minecraft:offset", "x": trap(xz), "y": trap(y), "z": trap(xz)}),
    ];
    let inner = config.get("feature").cloned().unwrap_or_default();
    let feature = if inner.is_object() {
        let p = placed(&inner, report);
        placement.extend(p["placement"].as_array().into_iter().flatten().cloned());
        p["feature"].clone()
    } else {
        json!({"type": "minecraft:simple_random_selector", "features": [inner]})
    };
    json!({"type": "minecraft:simple_random_selector",
        "features": [{"feature": feature, "placement": placement}]})
}

/// 1.21.1's `nether_forest_vegetation` as 26.3 wrote vanilla's crimson and
/// warped forest vegetation: on nylium, its block tried `spread_width²` times
/// at `nextInt(w) - nextInt(w)` (y from `spread_height`) where there is air.
fn nether_forest_vegetation(config: &Value, report: &mut Report) -> Value {
    let int = |k: &str| config.get(k).and_then(Value::as_i64).unwrap_or(1).max(1);
    let (w, h) = (int("spread_width"), int("spread_height"));
    let trap = |n: i64| json!({"type": "minecraft:trapezoid", "min": -n, "max": n, "plateau": 0});
    let on = |tag: &str| {
        json!({"type": "minecraft:block_predicate_filter",
        "predicate": {"type": "minecraft:matching_block_tag", "tag": tag}})
    };
    let mut nylium = on("minecraft:nylium");
    nylium["predicate"]["offset"] = json!([0, -1, 0]);
    let block = feature(
        &json!({"type": "minecraft:simple_block",
            "config": {"to_place": config.get("state_provider").cloned().unwrap_or_default()}}),
        report,
    );
    json!({"type": "minecraft:simple_random_selector", "features": [{"feature": block,
        "placement": [nylium, {"type": "minecraft:count", "count": w * w},
            {"type": "minecraft:offset", "x": trap(w - 1), "y": trap(h - 1), "z": trap(w - 1)},
            on("minecraft:air")]}]})
}

/// 26.2 block state provider types and their 26.3 names.
fn provider(t: &str) -> Option<&'static str> {
    Some(match t {
        "simple_state_provider" => "simple_state_provider",
        "weighted_state_provider" => "weighted",
        "rotated_block_provider" => "rotated",
        "randomized_int_state_provider" => "randomized_int",
        "rule_based_state_provider" => "rule_based",
        "noise_provider" => "noise",
        "dual_noise_provider" => "dual_noise",
        "noise_threshold_provider" => "noise_threshold",
        _ => return None,
    })
}

/// One placement modifier: `random_offset` became `offset` with one
/// provider per axis (x and z both from `xz_spread`, drawn in that order).
fn placement(m: &mut Value) {
    if kind(m) == Some("random_offset") {
        let xz = m.get("xz_spread").cloned().unwrap_or(json!(0));
        let y = m.get("y_spread").cloned().unwrap_or(json!(0));
        *m = json!({"type": "minecraft:offset", "x": xz.clone(), "y": y, "z": xz});
    }
}

/// `TrunkPlacer.setDirtAt` in 1.21.1 becomes `placeBelowTrunkBlock` with an
/// optional provider in 26.3. Use the target version's protected soil tag.
fn tree_soil(tree: &mut Value) {
    let Value::Object(config) = tree else {
        return;
    };
    let Some(dirt_provider) = config.remove("dirt_provider") else {
        return;
    };
    let force_dirt = config.remove("force_dirt") == Some(Value::Bool(true));
    config.entry("below_trunk_provider").or_insert_with(|| {
        if force_dirt {
            dirt_provider
        } else if dirt_provider == json!({"id": "minecraft:dirt"})
            || dirt_provider == json!({"id": "minecraft:dirt", "properties": {}})
        {
            vanilla("soil_beneath_tree")
        } else {
            json!({
                "type": "minecraft:rule_based",
                "rules": [{
                    "if_true": {"type": "minecraft:not", "predicate": {
                        "type": "minecraft:matching_block_tag",
                        "tag": "minecraft:cannot_replace_below_tree_trunk"
                    }},
                    "then": dirt_provider
                }]
            })
        }
    });
}

/// A configured feature in 26.3's form: `config` unwrapped, block state
/// providers renamed, nested features and placements upgraded.
pub(crate) fn feature(v: &Value, report: &mut Report) -> Value {
    let Value::Object(o) = v else {
        return v.clone();
    };
    let Some(t) = kind(v) else {
        return v.clone();
    };
    if PATCHES.contains(&t) {
        return patch(o.get("config").unwrap_or(&Value::Null), report);
    }
    if t == "nether_forest_vegetation" {
        return nether_forest_vegetation(o.get("config").unwrap_or(&Value::Null), report);
    }
    if REPLACED_FEATURES.contains(&t) {
        report.unknown(format!("feature {t} (written as data in 26.3)"));
    }
    let mut n = Map::new();
    n.insert("type".into(), o["type"].clone());
    if let Some(Value::Object(config)) = o.get("config") {
        for (k, x) in config {
            n.insert(k.clone(), x.clone());
        }
    }
    renamed_feature(t, &mut n);
    let mut v = Value::Object(n);
    states(&mut v);
    nested(&mut v, report);
    if t == "tree" {
        tree_soil(&mut v);
    }
    crate::lithostitched::weighted_selector(&mut v);
    v
}

/// Features 26.3 renamed and generalised, as its vanilla data writes them: a dripstone cluster
/// is a speleothem cluster of dripstone, a forest rock a block blob on the tag that names what
/// 26.2 checked in code.
fn renamed_feature(t: &str, n: &mut Map<String, Value>) {
    match t {
        "dripstone_cluster" => {
            n.insert("type".into(), vanilla("speleothem_cluster"));
            for (old, new) in [
                (
                    "dripstone_block_layer_thickness",
                    "speleothem_block_layer_thickness",
                ),
                (
                    "chance_of_dripstone_column_at_max_distance_from_center",
                    "chance_of_speleothem_at_max_distance_from_center",
                ),
                (
                    "max_distance_from_edge_affecting_chance_of_dripstone_column",
                    "max_distance_from_edge_affecting_chance_of_speleothem",
                ),
            ] {
                if let Some(x) = n.remove(old) {
                    n.insert(new.into(), x);
                }
            }
            n.insert("base_block".into(), vanilla("dripstone_block"));
            n.insert("pointed_block".into(), vanilla("pointed_dripstone"));
            n.insert(
                "replaceable_blocks".into(),
                json!("#minecraft:dripstone_replaceable_blocks"),
            );
        }
        // 26.2's disk held a rule-based provider by its fields alone; 26.3's names its type.
        "disk" => {
            if let Some(Value::Object(provider)) = n.get_mut("state_provider")
                && !provider.contains_key("type")
                && provider.contains_key("fallback")
            {
                provider.insert("type".into(), vanilla("rule_based"));
            }
        }
        // 26.2's tree set the block under its trunk to `dirt_provider`'s unless it was dirt
        // already (or always, with `force_dirt`); 26.3's names one provider for that block,
        // vanilla's a rule-based dirt over all but the blocks a trunk can't replace.
        "tree" => {
            let force = n.remove("force_dirt").and_then(|f| f.as_bool()) == Some(true);
            if let Some(dirt) = n.remove("dirt_provider")
                && !n.contains_key("below_trunk_provider")
            {
                let below = if force {
                    dirt
                } else {
                    json!({"type": "minecraft:rule_based", "rules": [{"if_true": {
                        "type": "minecraft:not", "predicate": {
                            "type": "minecraft:matching_block_tag",
                            "tag": "minecraft:cannot_replace_below_tree_trunk"}},
                        "then": dirt}]})
                };
                n.insert("below_trunk_provider".into(), below);
            }
        }
        // 26.2's multiface growth grew glow lichen when it named no block; 26.3's names it.
        "multiface_growth" => {
            n.entry("block").or_insert_with(|| vanilla("glow_lichen"));
        }
        "forest_rock" => {
            n.insert("type".into(), vanilla("block_blob"));
            n.insert(
                "can_place_on".into(),
                json!({"type": "minecraft:matching_block_tag",
                    "tag": "minecraft:forest_rock_can_place_on"}),
            );
        }
        _ => {}
    }
}

/// Inside a feature: block state providers, placed features and
/// configured features held inline.
fn nested(v: &mut Value, report: &mut Report) {
    match v {
        Value::Object(o) => {
            if let Some(t) = kind(&Value::Object(o.clone()))
                && let Some(to) = provider(t)
            {
                if t == "simple_state_provider" {
                    // A direct provider is a state object: a bare string
                    // would name a provider in its registry instead.
                    *v = match o.get("state").cloned().unwrap_or_default() {
                        id @ Value::String(_) => json!({"id": id}),
                        state => state,
                    };
                    return;
                }
                o.insert("type".into(), vanilla(to));
                // A noise provider's inline noise parameters, as the noise registry's.
                for key in ["noise", "slow_noise"] {
                    if let Some(n) = o.get(key).filter(|n| n.is_object()) {
                        let n = super::noise::noise(n, report);
                        o.insert(key.into(), n);
                    }
                }
            }
            // An inline placed feature: {feature, placement}.
            if o.contains_key("placement") && o.contains_key("feature") {
                let placed = placed(&Value::Object(o.clone()), report);
                *v = placed;
                return;
            }
            for x in o.values_mut() {
                if x.get("config").is_some() && x.get("type").is_some() {
                    *x = feature(x, report);
                } else {
                    nested(x, report);
                }
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|x| nested(x, report)),
        _ => {}
    }
}

/// A placed feature: its feature (an id or inline) and its placement
/// modifiers.
pub(crate) fn placed(v: &Value, report: &mut Report) -> Value {
    let mut v = v.clone();
    states(&mut v);
    if let Some(f) = v.get("feature").cloned()
        && f.is_object()
    {
        v["feature"] = feature(&f, report);
    }
    if let Some(patch) = v
        .get("feature")
        .and_then(Value::as_str)
        .and_then(removed_patch)
    {
        v["feature"] = patch;
    }
    if let Some(Value::Array(ms)) = v.get_mut("placement") {
        for m in ms.iter_mut() {
            placement(m);
        }
    }
    v
}

/// A vanilla patch feature 26.3 left out (`minecraft:patch_grass`): 26.3 keeps only the block
/// (`minecraft:grass`), the patch being its placed features' placement. As 26.2's random patch
/// of it, with 1.20's tries and spread: on a free block, and on grass for the crops that need it.
fn removed_patch(id: &str) -> Option<Value> {
    let name = id.strip_prefix("minecraft:")?.strip_prefix("patch_")?;
    let (block, tries, xz, y) = match name {
        "grass" | "grass_jungle" | "taiga_grass" => (name, 32, 7, 3),
        "dead_bush" => (name, 4, 7, 3),
        "cactus" | "waterlily" => (name, 10, 7, 3),
        "sugar_cane" => (name, 20, 4, 0),
        "melon" => (name, 64, 7, 3),
        "large_fern" | "tall_grass" | "sunflower" | "berry_bush" | "pumpkin" | "red_mushroom"
        | "brown_mushroom" | "bush" | "firefly_bush" | "leaf_litter" | "dry_grass" => {
            (name, 96, 7, 3)
        }
        "berry_common" | "berry_rare" => ("berry_bush", 96, 7, 3),
        _ => return None,
    };
    let mut filter = json!({"type": "minecraft:matching_block_tag", "tag": "minecraft:air"});
    if matches!(block, "pumpkin" | "melon" | "berry_bush") {
        filter = json!({"type": "minecraft:all_of", "predicates": [filter, {
            "type": "minecraft:matching_blocks", "offset": [0, -1, 0],
            "blocks": "minecraft:grass_block"}]});
    }
    let spread = |s: i64| json!({"type": "minecraft:trapezoid", "min": -s, "max": s, "plateau": 0});
    Some(
        json!({"type": "minecraft:random_selector", "features": [], "default": {
        "feature": format!("minecraft:{block}"),
        "placement": [
            {"type": "minecraft:count", "count": tries},
            {"type": "minecraft:offset", "x": spread(xz), "y": spread(y), "z": spread(xz)},
            {"type": "minecraft:block_predicate_filter", "predicate": filter}]}}),
    )
}
