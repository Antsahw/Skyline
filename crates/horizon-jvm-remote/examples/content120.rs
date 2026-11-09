//! Boots the 1.20.1 runtime with a mods folder and builds their Horizon content mods:
//!   cargo run -p horizon-jvm-remote --example content120 -- MODS_DIR RESOURCES OUT
//! RESOURCES: Horizon's prepared 26.3 folder (.local/horizon-client/26.3). Needs fetch120.py
//! and build120.sh (HORIZON_120_CACHE, default ~/.cache/horizon-120); the bridge's classes are
//! compiled into OUT.

use horizon_jvm_remote::boot120::{self, Fabric120, Side};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn main() -> Result<(), String> {
    let args: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let [mods, resources, out] = args.as_slice() else {
        return Err("usage: content120 MODS_DIR RESOURCES OUT".into());
    };
    let cache = std::env::var_os("HORIZON_120_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap()).join(".cache/horizon-120")
        })
        .join("1.20.1");
    let java = boot120::find_java();
    let javac = java.with_file_name("javac");
    let bridge = out.join("bridge-classes");
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../horizon-jvm/java/dev/horizonlab/bridge");
    let mut sources = Vec::new();
    for dir in ["remote", "r120"] {
        for e in std::fs::read_dir(root.join(dir)).map_err(|e| e.to_string())? {
            let p = e.map_err(|e| e.to_string())?.path();
            if p.extension().is_some_and(|x| x == "java") {
                sources.push(p);
            }
        }
    }
    let ok = std::process::Command::new(&javac)
        .args(["--release", "17", "-proc:none", "-nowarn", "-d"])
        .arg(&bridge)
        .args(&sources)
        .status()
        .map_err(|e| format!("{}: {e}", javac.display()))?
        .success();
    if !ok {
        return Err("javac failed".into());
    }
    let jars: Vec<PathBuf> = std::fs::read_dir(mods)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "jar"))
        .collect();
    let config = Fabric120 {
        java,
        install: cache.clone(),
        bridge,
        game_dir: out.join("game"),
        mods: Some(mods.clone()),
        platform: Some(cache.join("horizon/platform120.jar")),
        side: Side::Client,
        max_heap: "3G".into(),
        jvm_args: vec![],
        timeout: Duration::from_secs(900),
        log: Arc::new(|l: &str| eprintln!("{l}")),
    };
    let started = std::time::Instant::now();
    let (mut runtime, _report) = boot120::start(&config)?;
    let folders = horizon_jvm_remote::content120::build(
        &mut runtime,
        &jars,
        resources,
        &out.join("content"),
        &|l| eprintln!("{l}"),
    )?;
    runtime.stop();
    println!(
        "{} content mods in {:.0?}:",
        folders.len(),
        started.elapsed()
    );
    for f in folders {
        println!("  {}", f.display());
    }
    Ok(())
}
