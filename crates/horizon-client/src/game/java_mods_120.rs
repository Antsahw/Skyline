//! Fabric 1.20.1 mods (docs/java-bridge-120.md, phase 2): when `mods-java-1.20.1/` beside
//! `options.txt` holds jars, their blocks, items and assets become content mods the game loads
//! like any other. The jars run unmodified in a JVM of their own (horizon-jvm-remote's
//! `boot120`), which writes what they register; `content120` turns each jar into a content mod.
//! The result is cached under the jars' stamp, so a start with the same mods starts no Java.
//!
//! Needs, on this machine, what Mojang's files make (never shipped): `fetch120.py`'s folder
//! (`HORIZON_120_CACHE`, default `~/.cache/horizon-120`) and `build120.sh`'s platform jar in it.
//! Without them the game says which command to run and starts without the 1.20.1 mods.
//! Behaviour (block entities, menus, machines) is later phases: the blocks place, draw, drop
//! and save; what a mod's code does with them doesn't run yet.

use horizon_jvm_remote::boot120::{self, Fabric120, Side};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

/// The folder beside `options.txt` that holds the 1.20.1 Fabric mods.
pub(in crate::game) const DIR: &str = "mods-java-1.20.1";

/// The jars in `dir`, sorted.
fn jars(dir: &Path) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "jar"))
        .collect();
    out.sort();
    out
}

/// fetch120.py's folder for 1.20.1.
fn install() -> PathBuf {
    std::env::var_os("HORIZON_120_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache/horizon-120")))
        .unwrap_or_default()
        .join("1.20.1")
}

/// The content mods made from the jars in `mods-java-1.20.1/`, for the game's content root over
/// `base` (the prepared tables): read back from the cache when the jars are the ones they were
/// made from, else made now with the 1.20.1 runtime (stopped again after).
pub(in crate::game) fn content(base: &Path) -> Vec<PathBuf> {
    let dir = super::Game::options_path().with_file_name(DIR);
    let jars = jars(&dir);
    if jars.is_empty() {
        return Vec::new();
    }
    let home = super::mods::horizon_cache().join("java-120");
    let cache = home.join("content");
    if let Some(folders) = horizon_jvm_remote::content120::cached(&jars, &cache) {
        log::info!(
            "1.20.1 mods: {} content mod(s) from the cache",
            folders.len()
        );
        return folders;
    }
    match make(&dir, &jars, base, &home, &cache) {
        Ok(folders) => folders,
        Err(e) => {
            log::warn!("1.20.1 mods: their content was not made: {e}");
            Vec::new()
        }
    }
}

fn make(
    dir: &Path,
    jars: &[PathBuf],
    base: &Path,
    home: &Path,
    cache: &Path,
) -> Result<Vec<PathBuf>, String> {
    let install = install();
    if !install.join("launch.json").exists() {
        return Err(format!(
            "no {} (run tools/java_bridge/fetch120.py)",
            install.display()
        ));
    }
    let platform = install.join("horizon/platform120.jar");
    if !platform.exists() {
        return Err(format!(
            "no {} (run tools/java_bridge/build120.sh)",
            platform.display()
        ));
    }
    let bridge = home.join("bridge120.jar");
    horizon_jvm_remote::write_bridge120(&bridge)?;
    let started = std::time::Instant::now();
    log::info!(
        "1.20.1 mods: making the content of {} jar(s) (once for this set of mods)",
        jars.len()
    );
    let config = Fabric120 {
        java: boot120::find_java(),
        install,
        bridge,
        game_dir: home.join("game"),
        mods: Some(dir.to_path_buf()),
        platform: Some(platform),
        side: Side::Client,
        max_heap: "3G".into(),
        jvm_args: Vec::new(),
        timeout: Duration::from_secs(900),
        log: Arc::new(|line: &str| log::info!("{line}")),
    };
    let (mut runtime, report) = boot120::start(&config)?;
    log::info!(
        "1.20.1 mods: {} mods loaded",
        boot120::report_mods(&report).len()
    );
    let built = horizon_jvm_remote::content120::build(&mut runtime, jars, base, cache, &|l| {
        log::info!("{l}")
    });
    runtime.stop();
    let folders = built?;
    log::info!(
        "1.20.1 mods: {} content mod(s) made in {} ms",
        folders.len(),
        started.elapsed().as_millis()
    );
    Ok(folders)
}
