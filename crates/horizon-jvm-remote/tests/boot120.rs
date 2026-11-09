//! Fabric 1.20.1 booted from Rust through the socket transport, in a child JVM, with the
//! pinned Fabric API and any mods in `HORIZON_120_MODS`. Needs fetch120.py's folder
//! (`HORIZON_120_CACHE`, default `~/.cache/horizon-120`) and a JDK; passes without them.
//!   python3 -I tools/java_bridge/fetch120.py
//!   cargo test -p horizon-jvm-remote --test boot120 -- --nocapture

mod support;

use horizon_jvm_remote::boot120::{self, Fabric120, Side};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// The bridge's `remote` and `r120` classes, compiled into the target folder.
fn bridge_classes() -> Option<PathBuf> {
    static CLASSES: std::sync::OnceLock<Option<PathBuf>> = std::sync::OnceLock::new();
    CLASSES.get_or_init(compile_bridge).clone()
}

fn compile_bridge() -> Option<PathBuf> {
    let home = support::java_home()?;
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("bridge120-classes");
    let _ = std::fs::remove_dir_all(&out);
    let mut sources = Vec::new();
    for dir in ["remote", "r120"] {
        let dir = manifest
            .join("../horizon-jvm/java/dev/horizonlab/bridge")
            .join(dir);
        for entry in std::fs::read_dir(dir).ok()? {
            let path = entry.ok()?.path();
            if path.extension().is_some_and(|e| e == "java") {
                sources.push(path);
            }
        }
    }
    let status = std::process::Command::new(home.join("bin/javac"))
        .args(["--release", "17", "-proc:none", "-nowarn", "-d"])
        .arg(&out)
        .args(&sources)
        .status()
        .ok()?;
    assert!(status.success(), "javac failed");
    Some(out)
}

/// fetch120.py's folder, or `None` (and a note) without one.
fn cache() -> Option<PathBuf> {
    let cache = std::env::var_os("HORIZON_120_CACHE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache/horizon-120")))?
        .join("1.20.1");
    if !cache.join("launch.json").exists() {
        eprintln!(
            "no {}: run tools/java_bridge/fetch120.py (skipping)",
            cache.display()
        );
        return None;
    }
    Some(cache)
}

type Lines = Arc<std::sync::Mutex<Vec<String>>>;

fn config(cache: PathBuf, bridge: PathBuf, game: PathBuf, lines: Lines) -> Fabric120 {
    Fabric120 {
        // The child's Java: HORIZON_120_JAVA (a `java` executable), else the test's JDK.
        java: std::env::var_os("HORIZON_120_JAVA")
            .map(PathBuf::from)
            .unwrap_or_else(|| support::java_home().unwrap().join("bin/java")),
        install: cache,
        bridge,
        game_dir: game,
        mods: None,
        side: Side::Client,
        max_heap: "3G".into(),
        jvm_args: vec![],
        timeout: std::time::Duration::from_secs(600),
        log: Arc::new(move |l: &str| {
            println!("{l}");
            lines.lock().unwrap().push(l.to_string());
        }),
    }
}

/// The `stuckmod` fixture (`crates/horizon-jvm/java/fixtures/stuckmod`: declares `fabricloader <=0.16.4`, like mods stuck on that loader),
/// built into `dir` against the runtime's Fabric Loader.
fn stuck_mod(cache: &Path, dir: &Path) {
    let home = support::java_home().unwrap();
    let launch: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(cache.join("launch.json")).unwrap()).unwrap();
    let loader: Vec<&str> = launch["class_path"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|p| p.as_str())
        .filter(|p| p.contains("fabric-loader-"))
        .collect();
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../horizon-jvm/java/fixtures/stuckmod");
    let classes = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("stuckmod-classes");
    let _ = std::fs::remove_dir_all(&classes);
    let ok = std::process::Command::new(home.join("bin/javac"))
        .args([
            "--release",
            "17",
            "-proc:none",
            "-cp",
            &loader.join(":"),
            "-d",
        ])
        .arg(&classes)
        .arg(src.join("dev/horizonlab/stuckmod/StuckMod.java"))
        .status()
        .unwrap()
        .success();
    assert!(ok, "javac stuckmod");
    std::fs::copy(src.join("fabric.mod.json"), classes.join("fabric.mod.json")).unwrap();
    std::fs::create_dir_all(dir).unwrap();
    let ok = std::process::Command::new(home.join("bin/jar"))
        .arg("cf")
        .arg(dir.join("stuckmod-1.0.0.jar"))
        .arg("-C")
        .arg(&classes)
        .arg(".")
        .status()
        .unwrap()
        .success();
    assert!(ok, "jar stuckmod");
}

#[test]
fn a_mod_held_back_only_by_its_loader_range_runs_and_the_player_can_keep_the_range() {
    let Some(cache) = cache() else { return };
    let Some(bridge) = bridge_classes() else {
        return;
    };
    let game = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("game120-stuck");
    let _ = std::fs::remove_dir_all(&game);
    stuck_mod(&cache, &game.join("mods"));
    let lines: Lines = Arc::default();

    // Automatic: the declared range is dropped, the mod's code runs on the newer loader.
    let cfg = config(cache.clone(), bridge.clone(), game.clone(), lines.clone());
    let (runtime, report) = boot120::start(&cfg).expect("boots with the override");
    runtime.stop();
    let r: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(
        r["dependency_overrides"]["stuckmod"],
        serde_json::json!({"fabricloader": "<=0.16.4", "by": "auto"})
    );
    assert_eq!(r["entrypoints"]["stuckmod"]["main"], "ok");
    assert!(
        lines
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("[stuckmod] initialized on Fabric Loader 0.20.1"))
    );
    // Only the mods that need it: Fabric API's modules declare ranges the loader meets.
    assert_eq!(r["dependency_overrides"].as_object().unwrap().len(), 1);
    assert_eq!(r["problems"], serde_json::json!([]));

    // The player keeps the range: Fabric Loader refuses the mod, and says which and why.
    std::fs::write(
        game.join("horizon-dependency-overrides.json"),
        r#"{"version": 1, "overrides": {"stuckmod": {}}}"#,
    )
    .unwrap();
    let cfg = config(cache, bridge, game.clone(), lines);
    let error = boot120::start(&cfg).err().expect("refused");
    assert!(error.contains("stuckmod"), "{error}");
    assert!(!game.join("config/fabric_loader_dependencies.json").exists());
}

#[test]
fn fabric_1_20_1_boots_in_its_own_jvm_and_reports() {
    let Some(cache) = cache() else { return };
    let Some(bridge) = bridge_classes() else {
        eprintln!("no JDK (skipping)");
        return;
    };
    let game = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("game120");
    let _ = std::fs::remove_dir_all(game.join("mods"));
    std::fs::create_dir_all(game.join("mods")).unwrap();
    let extra = std::env::var_os("HORIZON_120_MODS").map(PathBuf::from);
    if let Some(dir) = &extra {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            std::fs::copy(&path, game.join("mods").join(path.file_name().unwrap())).unwrap();
        }
    }
    let lines: Lines = Arc::default();
    let config = config(cache, bridge, game, lines.clone());
    let (mut runtime, report) = boot120::start(&config).expect("1.20.1 boots");
    let r: serde_json::Value = serde_json::from_str(&report).unwrap();

    // Facts of vanilla 1.20.1, known independently of this code.
    assert_eq!(r["items"]["minecraft:diamond_sword"]["damage"], 1561);
    assert_eq!(r["blocks"]["minecraft:oak_stairs"]["states"], 80);
    assert_eq!(r["blocks"]["minecraft:glowstone"]["light"], 15);
    assert!(
        r["blocks"]["minecraft:sniffer_egg"].is_object(),
        "1.20 added the sniffer egg"
    );
    assert!(
        r["blocks"]["minecraft:crafter"].is_null(),
        "the crafter came in 1.20.3"
    );
    assert_eq!(r["biomes"]["vanilla"].as_array().unwrap().len(), 64);
    assert_eq!(r["entity_types"]["minecraft:zombie"]["category"], "monster");
    assert_eq!(r["mods"]["fabric-api"]["version"], "0.92.12+1.20.1");
    assert_eq!(r["mods"]["minecraft"]["version"], "1.20.1");

    // Every entrypoint ran; nothing went wrong.
    let failed: Vec<String> = r["entrypoints"]
        .as_object()
        .unwrap()
        .iter()
        .flat_map(|(id, e)| {
            e.as_object()
                .unwrap()
                .iter()
                .filter(|(_, v)| *v != "ok")
                .map(move |(k, v)| format!("{id} {k}: {v}"))
        })
        .collect();
    assert!(failed.is_empty(), "{failed:#?}");
    assert_eq!(r["problems"], serde_json::json!([]));
    // The runtime's own log lines came over the socket as events, the game's through stdout.
    let lines = lines.lock().unwrap();
    assert!(
        lines
            .iter()
            .any(|l| l.starts_with("[fabric-1.20.1] Fabric 1.20.1 ready in"))
    );
    assert!(
        lines
            .iter()
            .any(|l| l.contains("Loading Minecraft 1.20.1 with Fabric Loader"))
    );
    drop(lines);

    // The child keeps answering after the boot.
    assert_eq!(runtime.call("ping").unwrap(), "pong");
    if extra.is_some() {
        println!(
            "{} mods: {:?}",
            boot120::report_mods(&report).len(),
            boot120::report_mods(&report)
        );
    }
    runtime.stop();
}
