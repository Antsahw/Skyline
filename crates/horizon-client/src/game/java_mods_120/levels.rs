//! The 1.20.1 mods' dimensions as levels of Horizon's server (phase 6f): every level stem in the
//! mods' world generation pack (`content120::worldgen_pack`: Deeper and Darker's Otherside) is
//! registered as a `chunk::ExtraLevel` when the mods' content is ready, before a world opens, as
//! the 26.3 bridge registers its mods' (`java_mods/bridge.rs`): so its dimension type is in the
//! registries a joining player is sent, `/execute in`, `/tp` and the mods' teleports reach it,
//! and it's saved under `dimensions/<ns>/<path>`. Horizon's generator makes it from its stem in
//! the pack; the mods' own features in it are placed by the runtime, as in vanilla's levels.

use std::path::Path;

/// A level of a mod's dimension type, as `chunk::ExtraLevel` takes it; `None` when its build
/// range can't be one.
fn level(v: &serde_json::Value) -> Option<horizon_client::chunk::ExtraLevel> {
    let int = |k: &str| v[k].as_i64().and_then(|n| i32::try_from(n).ok());
    let flag = |k: &str| v[k].as_bool().unwrap_or(false);
    let id = v["id"].as_str()?;
    let shape = match horizon_client::chunk::LevelShape::new(
        int("min_y")?,
        int("height")?,
        int("logical_height")?,
        flag("has_skylight"),
        flag("has_ceiling"),
    ) {
        Ok(shape) => shape,
        Err(e) => {
            log::warn!("1.20.1 mods: the dimension {id} can't be a level: {e}");
            return None;
        }
    };
    // Drawn as the vanilla level its type is most like: a ceiling or no skylight as the
    // Nether, else the Overworld (the 26.3 bridge's rule).
    let looks_like = if flag("has_ceiling") || !flag("has_skylight") {
        "minecraft:the_nether"
    } else {
        "minecraft:overworld"
    };
    Some(horizon_client::chunk::ExtraLevel {
        id: Box::leak(id.to_owned().into_boxed_str()),
        dimension_type: Box::leak(v["type"].as_str().unwrap_or(id).to_owned().into_boxed_str()),
        shape,
        coordinate_scale: v["coordinate_scale"].as_f64().unwrap_or(1.0),
        looks_like,
    })
}

/// The level stems in the pack at `pack` (`data/<ns>/dimension/<path>.json`), each with its
/// dimension type (by id, from the pack's `dimension_type/`, or inline) as `level` reads it.
fn stems(pack: &Path) -> Vec<serde_json::Value> {
    let read = |p: &Path| -> Option<serde_json::Value> {
        serde_json::from_slice(&std::fs::read(p).ok()?).ok()
    };
    let mut out = Vec::new();
    let mut namespaces: Vec<_> = std::fs::read_dir(pack.join("data"))
        .into_iter()
        .flatten()
        .flatten()
        .collect();
    namespaces.sort_by_key(|e| e.file_name());
    for ns in namespaces {
        let ns_name = ns.file_name().to_string_lossy().into_owned();
        let dir = ns.path().join("dimension");
        let mut files: Vec<_> = walk(&dir);
        files.sort();
        for file in files {
            let Some(rel) = file
                .strip_prefix(&dir)
                .ok()
                .and_then(|r| r.to_str())
                .and_then(|r| r.strip_suffix(".json"))
            else {
                continue;
            };
            let id = format!("{ns_name}:{}", rel.replace('\\', "/"));
            if matches!(
                id.as_str(),
                "minecraft:overworld" | "minecraft:the_nether" | "minecraft:the_end"
            ) {
                continue;
            }
            let Some(stem) = read(&file) else { continue };
            let (type_id, ty) = match &stem["type"] {
                serde_json::Value::String(t) => {
                    let (tns, tpath) = t.split_once(':').unwrap_or(("minecraft", t));
                    let path = pack
                        .join("data")
                        .join(tns)
                        .join("dimension_type")
                        .join(format!("{tpath}.json"));
                    (t.clone(), read(&path))
                }
                inline => (id.clone(), Some(inline.clone())),
            };
            let Some(ty) = ty else {
                log::warn!("1.20.1 mods: the dimension {id}'s type {type_id} isn't in their data");
                continue;
            };
            let mut level = serde_json::json!({"id": id, "type": type_id});
            for k in [
                "min_y",
                "height",
                "logical_height",
                "has_skylight",
                "has_ceiling",
                "coordinate_scale",
            ] {
                level[k] = ty[k].clone();
            }
            out.push(level);
        }
    }
    out
}

fn walk(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let p = e.path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else if p.extension().is_some_and(|x| x == "json") {
            out.push(p);
        }
    }
    out
}

/// Registers the dimensions of the mods' world generation pack at `pack`.
pub(super) fn register(pack: &Path) {
    for v in &stems(pack) {
        let Some(level) = level(v) else {
            continue;
        };
        if horizon_client::chunk::extra_level_named(level.id) == Some(level) {
            continue;
        }
        match horizon_client::chunk::register_extra_level(level) {
            Some(_) => log::info!(
                "1.20.1 mods: the dimension {} is a level (type {}, y {}..{})",
                level.id,
                level.dimension_type,
                level.shape.min_y,
                level.shape.min_y + level.shape.height
            ),
            None => log::warn!(
                "1.20.1 mods: too many dimensions: {} has no level",
                level.id
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dimension_without_skylight_looks_like_the_nether() {
        let otherside = serde_json::json!({"id": "deeperdarker:otherside",
            "type": "deeperdarker:otherside", "min_y": 0, "height": 128, "logical_height": 128,
            "has_skylight": false, "has_ceiling": true, "coordinate_scale": 8.0});
        let l = level(&otherside).expect("a level");
        assert_eq!(l.id, "deeperdarker:otherside");
        assert_eq!((l.shape.min_y, l.shape.height), (0, 128));
        assert_eq!(l.looks_like, "minecraft:the_nether");
        assert_eq!(l.coordinate_scale, 8.0);
        let mut bad = otherside.clone();
        bad["height"] = serde_json::json!(7);
        assert!(level(&bad).is_none(), "a build range no level can have");
    }

    #[test]
    fn the_packs_level_stems_and_their_types_are_read() {
        let dir = std::env::temp_dir().join(format!("hz-levels120-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let write = |p: &str, v: serde_json::Value| {
            let f = dir.join(p);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(f, v.to_string()).unwrap();
        };
        write(
            "data/dd/dimension/otherside.json",
            serde_json::json!({"type": "dd:otherside", "generator": {}}),
        );
        write(
            "data/dd/dimension_type/otherside.json",
            serde_json::json!({"min_y": 0, "height": 128, "logical_height": 128,
                "has_skylight": false, "has_ceiling": true, "coordinate_scale": 8.0}),
        );
        // Vanilla's own stem (Terralith's overworld) is no extra level.
        write(
            "data/minecraft/dimension/overworld.json",
            serde_json::json!({"type": "minecraft:overworld"}),
        );
        let stems = stems(&dir);
        assert_eq!(stems.len(), 1, "{stems:?}");
        let l = level(&stems[0]).expect("a level");
        assert_eq!((l.id, l.dimension_type), ("dd:otherside", "dd:otherside"));
        assert_eq!(
            (l.shape.min_y, l.shape.height, l.coordinate_scale),
            (0, 128, 8.0)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
