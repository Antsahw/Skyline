//! The 1.20.1 mods' blocks, items and assets as Horizon content (docs/java-bridge-120.md,
//! phase 2), as horizon-jvm's `content.rs` makes 26.3 mods': the running 1.20.1 runtime writes
//! the oracle facts of what the mods register (platform120's `Content120`), and
//! `horizon-port-pack`'s carry-over turns each jar into a folder content mod with them: block
//! declarations from the vanilla template of the same class and shapes, the mods' own numbers,
//! models, textures, recipes, loot and tags, upgraded from 1.20.1's formats to 26.3's. The game
//! loads those as it loads any content mod.
//!
//! The result is kept in a cache folder under a stamp of the jars (names, sizes, times) and of
//! this crate's version, so a start with the same mods reads it back without starting Java.

use crate::boot120::Runtime120;
use crate::wire::Value;
use std::path::{Path, PathBuf};

/// The version of what this module writes: bump it when the facts or the carry-over change.
/// 3: nested mods are inventoried whether or not they ship textures ([`nested_jars`]), so a
/// cache built before this holds none of a library mod's blocks.
const FORMAT: u32 = 4;

/// The stamp of a set of jars: their names, sizes and modification times, and [`FORMAT`].
pub fn stamp(jars: &[PathBuf]) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    FORMAT.hash(&mut hasher);
    "1.20.1".hash(&mut hasher);
    env!("CARGO_PKG_VERSION").hash(&mut hasher);
    let mut sorted = jars.to_vec();
    sorted.sort();
    for jar in &sorted {
        jar.file_name().hash(&mut hasher);
        if let Ok(meta) = std::fs::metadata(jar) {
            meta.len().hash(&mut hasher);
            if let Ok(t) = meta.modified() {
                t.hash(&mut hasher);
            }
        }
    }
    format!("{:016x}", hasher.finish())
}

/// The content mod folders built for `jars` under `cache` before, when their stamp matches.
pub fn cached(jars: &[PathBuf], cache: &Path) -> Option<Vec<PathBuf>> {
    if std::fs::read_to_string(cache.join("stamp")).ok()?.trim() != stamp(jars) {
        return None;
    }
    Some(carried(&cache.join("work")))
}

/// The folder content mods under a work folder (`mods/<key>/carry`), in key order. One with
/// nothing in it but its manifest is left out: every nested jar is inventoried now, and Kotlin's
/// standard library carries no blocks, items or textures to give the game.
fn carried(work: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(work.join("mods"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join("carry"))
        .filter(|p| p.join("horizon-mod.toml").is_file() && has_content(p))
        .collect();
    out.sort();
    out
}

/// Whether a carried folder holds anything beside its manifest.
fn has_content(carry: &Path) -> bool {
    for dir in ["assets", "data"] {
        let mut stack = vec![carry.join(dir)];
        while let Some(at) = stack.pop() {
            for entry in std::fs::read_dir(&at).into_iter().flatten().flatten() {
                if entry.path().is_dir() {
                    stack.push(entry.path());
                } else {
                    return true;
                }
            }
        }
    }
    false
}

/// The jars a mod's `fabric.mod.json` carries inside it (`"jars": [{"file": ...}]`), and any under
/// `META-INF/jars/` it does not name. Fabric puts them where it likes: LibMultiPart ships a
/// dependency container whose four jars sit at the jar's root, not under `META-INF/jars/`.
fn carried_jars(zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>) -> Vec<String> {
    let mut names: Vec<String> = mod_json(zip)
        .and_then(|j| j["jars"].as_array().cloned())
        .unwrap_or_default()
        .iter()
        .filter_map(|j| j["file"].as_str().map(str::to_owned))
        .collect();
    for name in zip.file_names() {
        if name.starts_with("META-INF/jars/") && name.ends_with(".jar") {
            names.push(name.to_owned());
        }
    }
    names.sort();
    names.dedup();
    names
}

/// A jar's `fabric.mod.json`, when it has one that reads.
fn mod_json(zip: &mut zip::ZipArchive<std::io::Cursor<&[u8]>>) -> Option<serde_json::Value> {
    use std::io::Read;
    let mut text = String::new();
    zip.by_name("fabric.mod.json")
        .ok()?
        .read_to_string(&mut text)
        .ok()?;
    serde_json::from_str(&text).ok()
}

/// The mods a jar carries inside it (Fabric's jar-in-jar), written into `mods` as mods of their
/// own so the porter inventories them: a nested mod's blocks are only carried over when it has a
/// record there, and a mod that registers blocks in code alone (LibMultiPart's `container`, which
/// every Wired Redstone cable is a part of) has no assets to recognise it by. So every nested jar
/// that names itself a mod is written out, and every one is looked inside whether or not it is --
/// a dependency container is not a mod with content, but it holds four that are.
/// (horizon-jvm's `content.rs` does the same for 26.3's mods.)
fn nested_jars(bytes: &[u8], mods: &Path, depth: u32) {
    use std::io::Read;
    if depth == 0 {
        return;
    }
    let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(bytes)) else {
        return;
    };
    for name in carried_jars(&mut zip) {
        let mut inner = Vec::new();
        let Ok(mut entry) = zip.by_name(&name) else {
            continue;
        };
        if entry.read_to_end(&mut inner).is_err() {
            continue;
        }
        drop(entry);
        let mut nested = match zip::ZipArchive::new(std::io::Cursor::new(&inner[..])) {
            Ok(nested) => nested,
            Err(_) => continue,
        };
        let named = mod_json(&mut nested)
            .is_some_and(|j| j["id"].as_str().is_some_and(|id| !id.is_empty()));
        let file_name = name.rsplit('/').next().unwrap_or(&name);
        let out = mods.join(file_name);
        if named && !out.exists() {
            let _ = std::fs::write(&out, &inner);
        }
        nested_jars(&inner, mods, depth - 1);
    }
}

/// The mods' own creative tabs (the facts' `creative_tabs`, as their own code fills them) as
/// content tabs (`data/<ns>/horizon/creative_tab/<path>.json`: a title, an icon, the items),
/// each in the carried mod that owns its namespace, so the creative menu has a tab per mod
/// beside the 26.3 mods' and vanilla's. The title in English, from the mod's own language file
/// when it is a translation key. How many were written.
fn creative_tabs(facts: &Path, carried: &[PathBuf]) -> usize {
    let Ok(text) = std::fs::read_to_string(facts) else {
        return 0;
    };
    let Ok(facts) = serde_json::from_str::<serde_json::Value>(&text) else {
        return 0;
    };
    let mut written = 0;
    for tab in facts["creative_tabs"].as_array().into_iter().flatten() {
        let Some((ns, path)) = tab["id"].as_str().and_then(|id| id.split_once(':')) else {
            continue;
        };
        let items: Vec<&str> = tab["items"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|i| i.as_str())
            .collect();
        let (Some(icon), false) = (tab["icon"].as_str(), items.is_empty()) else {
            continue;
        };
        // The carried mod with that namespace's files.
        let Some(carry) = carried
            .iter()
            .find(|c| c.join("assets").join(ns).is_dir() || c.join("data").join(ns).is_dir())
        else {
            continue;
        };
        let english = tab["title_key"].as_str().and_then(|key| {
            let lang = std::fs::read(carry.join("assets").join(ns).join("lang/en_us.json")).ok()?;
            let lang: serde_json::Value = serde_json::from_slice(&lang).ok()?;
            lang[key].as_str().map(str::to_owned)
        });
        let title = english
            .or_else(|| tab["title"].as_str().map(str::to_owned))
            .unwrap_or_else(|| path.to_owned());
        let dir = carry.join("data").join(ns).join("horizon/creative_tab");
        let file = serde_json::json!({ "title": title, "icon": icon, "items": items });
        if std::fs::create_dir_all(&dir).is_ok()
            && std::fs::write(
                dir.join(format!("{}.json", path.replace('/', "_"))),
                file.to_string(),
            )
            .is_ok()
        {
            written += 1;
        }
    }
    written
}

/// Vanilla's blocks in the facts as 26.3 has them. The facts are 1.20.1's, and the carry-over
/// borrows vanilla states for mods' shapes (`"shape": "minecraft:piglin_head[rotation=0]"`): a
/// state 26.3 doesn't have (its heads gained `powered`) makes the game refuse every content mod.
/// So a vanilla block 26.3 lacks, or whose property names changed, leaves the facts, and of the
/// others only the states 26.3 has stay. 26.3's states come from the prepared `facts.json`.
fn vanilla_as_in_26_3(facts: &Path, resources: &Path) -> Result<(), String> {
    use serde_json::Value;
    use std::collections::{BTreeMap, BTreeSet};
    let read = |p: &Path| -> Result<Value, String> {
        let text = std::fs::read_to_string(p).map_err(|e| format!("{}: {e}", p.display()))?;
        serde_json::from_str(&text).map_err(|e| format!("{}: {e}", p.display()))
    };
    let prepared = read(&resources.join("facts.json"))?;
    // 26.3: block -> its states as sorted "k=v,..." keys.
    let mut states: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for s in prepared["states"].as_array().into_iter().flatten() {
        let Some(id) = s.get(0).and_then(Value::as_str) else {
            continue;
        };
        let key = state_key(s.get(4).and_then(|x| x.get("properties")));
        states.entry(id.to_owned()).or_default().insert(key);
    }
    let mut ours = read(facts)?;
    let Some(blocks) = ours["blocks"].as_object_mut() else {
        return Ok(());
    };
    let vanilla: Vec<String> = blocks
        .keys()
        .filter(|k| k.starts_with("minecraft:"))
        .cloned()
        .collect();
    for id in vanilla {
        let Some(theirs) = states.get(&id) else {
            blocks.remove(&id);
            continue;
        };
        let block = &mut blocks[&id];
        let Some(own) = block["states"].as_object_mut() else {
            continue;
        };
        own.retain(|key, _| theirs.contains(&sorted(key)));
        if own.is_empty() {
            blocks.remove(&id);
        }
    }
    let text = serde_json::to_string(&ours).map_err(|e| e.to_string())?;
    std::fs::write(facts, text).map_err(|e| format!("{}: {e}", facts.display()))
}

/// A state's properties as a key with the names sorted: "a=1,b=x".
fn state_key(properties: Option<&serde_json::Value>) -> String {
    let mut parts: Vec<String> = properties
        .and_then(|p| p.as_object())
        .map(|o| {
            o.iter()
                .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or_default()))
                .collect()
        })
        .unwrap_or_default();
    parts.sort();
    parts.join(",")
}

/// A facts state key ("rotation=0,powered=true", in the block's property order) sorted.
fn sorted(key: &str) -> String {
    let mut parts: Vec<&str> = key.split(',').filter(|p| !p.is_empty()).collect();
    parts.sort_unstable();
    parts.join(",")
}

/// Builds the content mods of `jars` into `cache` with the running 1.20.1 runtime (booted with
/// these jars and the platform jar), for the prepared game at `resources`: the folders to load.
pub fn build(
    runtime: &mut Runtime120,
    jars: &[PathBuf],
    resources: &Path,
    cache: &Path,
    log: &dyn Fn(String),
) -> Result<Vec<PathBuf>, String> {
    let _ = std::fs::remove_dir_all(cache);
    let work = cache.join("work");
    // An instance folder as the porter imports one: the jars in mods/, the game and loader named.
    let instance = cache.join("instance");
    let mods = instance.join("mods");
    std::fs::create_dir_all(&mods).map_err(|e| format!("{}: {e}", mods.display()))?;
    for jar in jars {
        let name = jar.file_name().ok_or("a jar without a name")?;
        std::fs::copy(jar, mods.join(name)).map_err(|e| format!("{}: {e}", jar.display()))?;
        let bytes = std::fs::read(jar).map_err(|e| format!("{}: {e}", jar.display()))?;
        nested_jars(&bytes, &mods, 4);
    }
    let pack = serde_json::json!({
        "components": [
            {"uid": "net.minecraft", "version": "1.20.1"},
            {"uid": "net.fabricmc.fabric-loader", "version": "0.20.1"},
        ]
    });
    std::fs::write(instance.join("mmc-pack.json"), pack.to_string())
        .map_err(|e| format!("mmc-pack.json: {e}"))?;
    let log_line = |line: String| log(format!("1.20.1 mods' content: {line}"));
    horizon_port_pack::pipeline::import(&instance, &work, &log_line)?;
    let records = horizon_port_pack::pipeline::inventory(&work, true, &log_line)?;
    let facts = work.join("oracle/facts.json");
    let namespaces =
        runtime.call_with("facts", &[Value::Str(facts.to_string_lossy().into_owned())])?;
    log(format!(
        "1.20.1 mods' content: the mods register in {}",
        if namespaces.is_empty() {
            "no namespace of their own"
        } else {
            &namespaces
        }
    ));
    vanilla_as_in_26_3(&facts, resources)?;
    // Every jar is carried: its assets and data under its own namespaces.
    let all: Vec<String> = records.iter().map(|r| r.key.clone()).collect();
    horizon_port_pack::pipeline::carry(&work, Some(resources), &all, &log_line)?;
    let tabs = creative_tabs(&facts, &carried(&work));
    if tabs > 0 {
        log(format!(
            "1.20.1 mods' content: {tabs} creative tab(s) of their own"
        ));
    }
    let _ = std::fs::remove_dir_all(&instance);
    std::fs::write(cache.join("stamp"), stamp(jars)).map_err(|e| format!("stamp: {e}"))?;
    Ok(carried(&work))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_mods_own_creative_tab_becomes_its_content_tab_in_english() {
        let dir = std::env::temp_dir().join(format!("content120-tabs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let carry = dir.join("mods/fd/carry");
        std::fs::create_dir_all(carry.join("assets/farmersdelight/lang")).unwrap();
        std::fs::write(
            carry.join("assets/farmersdelight/lang/en_us.json"),
            r#"{"itemGroup.farmersdelight": "Farmer's Delight"}"#,
        )
        .unwrap();
        let facts = dir.join("facts.json");
        let tab = serde_json::json!({"creative_tabs": [{
            "id": "farmersdelight:farmersdelight", "title_key": "itemGroup.farmersdelight",
            "title": "itemGroup.farmersdelight", "icon": "farmersdelight:stove",
            "items": ["farmersdelight:stove", "farmersdelight:cooking_pot"]}]});
        std::fs::write(&facts, tab.to_string()).unwrap();
        assert_eq!(creative_tabs(&facts, &[carry.clone()]), 1);
        let written: serde_json::Value = serde_json::from_slice(
            &std::fs::read(
                carry.join("data/farmersdelight/horizon/creative_tab/farmersdelight.json"),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            written["title"], "Farmer's Delight",
            "its English name, not the key"
        );
        assert_eq!(written["icon"], "farmersdelight:stove");
        assert_eq!(written["items"].as_array().unwrap().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    use super::*;

    #[test]
    fn vanilla_states_26_3_lacks_leave_the_facts() {
        let dir = std::env::temp_dir().join(format!("content120-vanilla-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 26.3's prepared facts: heads gained `powered`; stone as ever; no `minecraft:grass`.
        let prepared = serde_json::json!({"states": [
            ["minecraft:stone", false, false, "", {"id": "minecraft:stone"}],
            ["minecraft:piglin_head", false, false, "", {"id": "minecraft:piglin_head", "properties": {"rotation": "0", "powered": "false"}}],
            ["minecraft:oak_slab", false, false, "", {"id": "minecraft:oak_slab", "properties": {"type": "top", "waterlogged": "false"}}],
        ], "defaults": {}});
        std::fs::write(dir.join("facts.json"), prepared.to_string()).unwrap();
        let facts = dir.join("oracle-facts.json");
        let ours = serde_json::json!({"blocks": {
            "minecraft:stone": {"states": {"": {}}},
            "minecraft:piglin_head": {"states": {"rotation=0": {}}},
            "minecraft:grass": {"states": {"": {}}},
            "minecraft:oak_slab": {"states": {"waterlogged=false,type=top": {}, "waterlogged=true,type=top": {}}},
            "farmersdelight:stove": {"states": {"facing=north": {}}},
        }});
        std::fs::write(&facts, ours.to_string()).unwrap();
        vanilla_as_in_26_3(&facts, &dir).unwrap();
        let got: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&facts).unwrap()).unwrap();
        let blocks = got["blocks"].as_object().unwrap();
        let ids: Vec<&String> = blocks.keys().collect();
        assert_eq!(
            ids,
            [
                "farmersdelight:stove",
                "minecraft:oak_slab",
                "minecraft:stone"
            ]
        );
        let slab: Vec<&String> = blocks["minecraft:oak_slab"]["states"]
            .as_object()
            .unwrap()
            .keys()
            .collect();
        assert_eq!(slab, ["waterlogged=false,type=top"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// A jar with the given entries.
    fn jar(entries: &[(&str, &[u8])]) -> Vec<u8> {
        use std::io::Write;
        let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let options: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Stored);
        for (name, body) in entries {
            out.start_file(*name, options).unwrap();
            out.write_all(body).unwrap();
        }
        out.finish().unwrap().into_inner()
    }

    /// LibMultiPart's shape: Wired Redstone carries a dependency container, which is not a mod
    /// with content of its own, and the four mods it holds sit at its root rather than under
    /// `META-INF/jars/`. One of them registers `libmultipart:container` in code and ships no
    /// assets at all, so it has to be written out for the porter to inventory it.
    #[test]
    fn a_dependency_container_does_not_hide_the_mods_inside_it() {
        let base = jar(&[(
            "fabric.mod.json",
            br#"{"schemaVersion":1,"id":"libmultipart","version":"0.11.0"}"#,
        )]);
        let container = jar(&[
            (
                "fabric.mod.json",
                br#"{"schemaVersion":1,"id":"libmultipart_deps_container","version":"0.11.0",
                     "jars":[{"file":"libmultipart-base-0.11.0.jar"}]}"#,
            ),
            ("libmultipart-base-0.11.0.jar", &base),
            ("assets/libmultipart/icon.png", b"not a png"),
        ]);
        let top = jar(&[
            (
                "fabric.mod.json",
                br#"{"schemaVersion":1,"id":"wiredredstone","version":"0.7.8"}"#,
            ),
            ("META-INF/jars/libmultipart-all-0.11.0.jar", &container),
        ]);

        let dir = std::env::temp_dir().join(format!("content120-nested-{}", std::process::id()));
        let mods = dir.join("mods");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&mods).unwrap();
        nested_jars(&top, &mods, 4);
        let mut found: Vec<String> = std::fs::read_dir(&mods)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        found.sort();
        assert_eq!(
            found,
            [
                "libmultipart-all-0.11.0.jar",
                "libmultipart-base-0.11.0.jar"
            ]
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Every nested jar is inventoried, so the ones that carry nothing (Kotlin's standard
    /// library) must not reach the game as content mods of their own.
    #[test]
    fn a_carried_folder_with_nothing_in_it_is_not_a_content_mod() {
        let dir = std::env::temp_dir().join(format!("content120-carried-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mods = dir.join("mods");
        for (key, files) in [
            (
                "libmultipart",
                &["data/libmultipart/horizon/block/container.json"][..],
            ),
            ("kotlin-stdlib", &[][..]),
        ] {
            let carry = mods.join(key).join("carry");
            std::fs::create_dir_all(&carry).unwrap();
            std::fs::write(carry.join("horizon-mod.toml"), "id = \"x\"\n").unwrap();
            for f in files {
                let at = carry.join(f);
                std::fs::create_dir_all(at.parent().unwrap()).unwrap();
                std::fs::write(at, "{}").unwrap();
            }
        }
        let got: Vec<String> = carried(&dir)
            .iter()
            .map(|p| {
                p.parent()
                    .unwrap()
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert_eq!(got, ["libmultipart"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
