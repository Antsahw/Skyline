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
const FORMAT: u32 = 1;

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

/// The folder content mods under a work folder (`mods/<key>/carry`), in key order.
fn carried(work: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(work.join("mods"))
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path().join("carry"))
        .filter(|p| p.join("horizon-mod.toml").is_file())
        .collect();
    out.sort();
    out
}

/// The mods a jar carries inside it (Fabric's jar-in-jar, `META-INF/jars/`) that have textures,
/// written into `mods` as mods of their own so their assets are carried too. Libraries and
/// Fabric API's modules (no textures) stay inside. (horizon-jvm's `content.rs` does the same
/// for 26.3's mods.)
fn nested_jars(jar: &Path, mods: &Path, depth: u32) {
    use std::io::Read;
    let Ok(file) = std::fs::File::open(jar) else {
        return;
    };
    let Ok(mut zip) = zip::ZipArchive::new(file) else {
        return;
    };
    let inner: Vec<String> = zip
        .file_names()
        .filter(|n| n.starts_with("META-INF/jars/") && n.ends_with(".jar"))
        .map(str::to_owned)
        .collect();
    for name in inner {
        let mut bytes = Vec::new();
        let Ok(mut entry) = zip.by_name(&name) else {
            continue;
        };
        if entry.read_to_end(&mut bytes).is_err() {
            continue;
        }
        let Ok(nested) = zip::ZipArchive::new(std::io::Cursor::new(&bytes)) else {
            continue;
        };
        let textures = nested
            .file_names()
            .any(|n| n.starts_with("assets/") && n.contains("/textures/") && n.ends_with(".png"));
        let file_name = name.rsplit('/').next().unwrap_or(&name);
        let out = mods.join(file_name);
        if !textures || out.exists() {
            continue;
        }
        if std::fs::write(&out, &bytes).is_ok() && depth > 1 {
            nested_jars(&out, mods, depth - 1);
        }
    }
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
        nested_jars(jar, &mods, 2);
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
    // Every jar is carried: its assets and data under its own namespaces.
    let all: Vec<String> = records.iter().map(|r| r.key.clone()).collect();
    horizon_port_pack::pipeline::carry(&work, Some(resources), &all, &log_line)?;
    let _ = std::fs::remove_dir_all(&instance);
    std::fs::write(cache.join("stamp"), stamp(jars)).map_err(|e| format!("stamp: {e}"))?;
    Ok(carried(&work))
}
