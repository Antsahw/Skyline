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

/// A configured feature in 26.3's form: `config` unwrapped, block state
/// providers renamed, nested features and placements upgraded.
pub(super) fn feature(v: &Value, report: &mut Report) -> Value {
    let Value::Object(o) = v else {
        return v.clone();
    };
    let Some(t) = kind(v) else {
        return v.clone();
    };
    if REPLACED_FEATURES.contains(&t) {
        report.unknown(format!("feature {t} (written as data in 26.3)"));
    }
    if t == "nether_forest_vegetation" {
        let config = o.get("config").cloned().unwrap_or_default();
        return nether_forest_vegetation(&config, report);
    }
    // 26.2's flower features were random patches, as 1.20's.
    if matches!(t, "random_patch" | "flower" | "no_bonemeal_flower") {
        let config = o.get("config").cloned().unwrap_or_default();
        return random_patch(&config, report);
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

/// A `random_patch` as 26.3 writes vanilla's: the patch's tries and spread are placement
/// modifiers (a count, then an offset per axis, triangular as `nextInt(s + 1) - nextInt(s + 1)`
/// was) in front of the placed feature's own, under a selector with no choice but its default.
/// A placed feature named by id keeps its own placement, one selector further in.
fn random_patch(config: &Value, report: &mut Report) -> Value {
    let int = |k: &str, default: i64| config.get(k).and_then(Value::as_i64).unwrap_or(default);
    let spread = |s: i64| json!({"type": "minecraft:trapezoid", "min": -s, "max": s, "plateau": 0});
    let (xz, y) = (spread(int("xz_spread", 7)), spread(int("y_spread", 3)));
    let mut placement = vec![
        json!({"type": "minecraft:count", "count": int("tries", 128)}),
        json!({"type": "minecraft:offset", "x": xz.clone(), "y": y, "z": xz}),
    ];
    let inner = config.get("feature").cloned().unwrap_or_default();
    let feature = if inner.is_object() {
        let inner = placed(&inner, report);
        if let Some(Value::Array(own)) = inner.get("placement") {
            placement.extend(own.iter().cloned());
        }
        inner.get("feature").cloned().unwrap_or_default()
    } else {
        json!({"type": "minecraft:random_selector", "features": [], "default": inner})
    };
    json!({"type": "minecraft:random_selector", "features": [],
        "default": {"feature": feature, "placement": placement}})
}

/// Nether forest vegetation as 26.3's vanilla writes its warped forest's: a block on nylium,
/// `spread_width²` tries spread `nextInt(w) - nextInt(w)` (the block placed only in air, where
/// it survives, as `simple_block` checks).
fn nether_forest_vegetation(config: &Value, report: &mut Report) -> Value {
    let int = |k: &str| config.get(k).and_then(Value::as_i64).unwrap_or(1).max(1);
    let (w, h) = (int("spread_width"), int("spread_height"));
    let spread = |s: i64| json!({"type": "minecraft:trapezoid", "min": -s, "max": s, "plateau": 0});
    let mut block = json!({"type": "minecraft:simple_block",
        "to_place": config.get("state_provider").cloned().unwrap_or_default()});
    states(&mut block);
    nested(&mut block, report);
    let on_nylium = json!({"type": "minecraft:block_predicate_filter", "predicate": {
        "type": "minecraft:matching_block_tag", "offset": [0, -1, 0], "tag": "minecraft:nylium"}});
    let in_air = json!({"type": "minecraft:block_predicate_filter", "predicate": {
        "type": "minecraft:matching_block_tag", "tag": "minecraft:air"}});
    json!({"type": "minecraft:random_selector", "features": [], "default": {"feature": block,
        "placement": [on_nylium, {"type": "minecraft:count", "count": w * w},
            {"type": "minecraft:offset", "x": spread(w - 1), "y": spread(h - 1), "z": spread(w - 1)},
            in_air]}})
}

/// Inside a feature: block state providers, placed features and
/// configured features held inline.
fn nested(v: &mut Value, report: &mut Report) {
    match v {
        Value::Object(o) => {
            if let Some(t) = kind(&Value::Object(o.clone())) {
                if let Some(to) = provider(t) {
                    if t == "simple_state_provider" {
                        *v = o.get("state").cloned().unwrap_or_default();
                        return;
                    }
                    o.insert("type".into(), vanilla(to));
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
pub(super) fn placed(v: &Value, report: &mut Report) -> Value {
    let mut v = v.clone();
    states(&mut v);
    if let Some(f) = v.get("feature").cloned()
        && f.is_object()
    {
        v["feature"] = feature(&f, report);
    }
    if let Some(Value::Array(ms)) = v.get_mut("placement") {
        for m in ms.iter_mut() {
            placement(m);
        }
    }
    v
}
