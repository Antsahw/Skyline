//! The components an oracle item's declaration carries as they are.
use super::*;

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

/// `item`'s 26.3 components that are carried (as the bridge's exporter writes them), with
/// what is left out pushed to `differences`.
pub(super) fn carried(item: &Item, id: &str, differences: &mut Vec<String>) -> Map<String, Value> {
    let own = id.split_once(':').map_or("", |(ns, _)| ns);
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
    components
}
