//! Boots Forge 1.12.2 (Cleanroom) hidden in the bridge's JVM with the fixture mod hello112 and
//! checks the platform's report (docs/java-bridge-112.md, phase 1). One test: one JVM a process.
//!
//! Needs Java 25 (`HORIZON_JAVA_HOME`, else `JAVA_HOME`), and the 1.12 files in the cache:
//! `python3 -I tools/java_bridge/fetch112.py && tools/java_bridge/build112.sh`
//! (`HORIZON_112_INSTALL` names another folder). Without them it says so and passes.

use std::path::PathBuf;
use std::sync::Mutex;

static LINES: Mutex<Vec<String>> = Mutex::new(Vec::new());

fn record(line: &str) {
    LINES.lock().unwrap().push(line.to_string());
}

fn install() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("HORIZON_112_INSTALL") {
        return Some(PathBuf::from(dir));
    }
    let lock: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../research-lock.json"
        ))
        .ok()?,
    )
    .ok()?;
    let version = lock["cleanroom"]["version"].as_str()?;
    Some(
        PathBuf::from(std::env::var_os("HOME")?)
            .join(".cache/horizon-112")
            .join(version),
    )
}

#[test]
fn cleanroom_boots_hidden_and_reports_the_fixture() {
    let home = [horizon_jvm::JAVA_HOME_VAR, "JAVA_HOME"]
        .iter()
        .find_map(|v| std::env::var_os(v).filter(|h| !h.is_empty()))
        .map(PathBuf::from);
    let (Some(home), Some(install)) = (home, install()) else {
        eprintln!("no Java 25 or no 1.12 install: see this file's header");
        return;
    };
    let fixture = install.join("horizon/hello112.jar");
    if !install.join("launch.json").is_file() || !fixture.is_file() {
        eprintln!(
            "{} has no launch.json or fixture: run fetch112.py and build112.sh",
            install.display()
        );
        return;
    }
    let dir = std::env::temp_dir().join(format!("horizon-112-test-{}", std::process::id()));
    let game_dir = dir.join("game");
    std::fs::create_dir_all(game_dir.join("mods")).unwrap();
    std::fs::copy(&fixture, game_dir.join("mods/hello112.jar")).unwrap();

    horizon_jvm::set_log_sink(Some(record));
    let config = horizon_jvm::boot112::Cleanroom {
        home,
        work: dir.join("work"),
        install,
        game_dir: game_dir.clone(),
    };
    let (bridge, report) = horizon_jvm::boot112::start(&config).expect("Cleanroom boots");
    assert!(!bridge.fabric());
    assert!(
        bridge.mods().iter().any(|m| m == "hello112 1.0"),
        "{:?}",
        bridge.mods()
    );
    assert!(
        bridge.mods().iter().any(|m| m.starts_with("forge 14.23.5")),
        "{:?}",
        bridge.mods()
    );
    // The platform mod stays out of the player's mods folder.
    assert!(!game_dir.join("mods/horizon112-platform.jar").exists());

    let report: serde_json::Value = serde_json::from_str(&report).unwrap();
    let find = |list: &str, id: &str| {
        report[list]
            .as_array()
            .unwrap()
            .iter()
            .find(|b| b["id"] == id)
            .unwrap_or_else(|| panic!("no {id} in {list}"))
            .clone()
    };
    // Stored states: the distinct getStateFromMeta(0..15), with their meta.
    let marble = find("blocks", "hello112:marble");
    let states: Vec<(i64, &str)> = marble["states"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| (s["meta"].as_i64().unwrap(), s["state"].as_str().unwrap()))
        .collect();
    assert_eq!(
        states,
        [
            (0, "hello112:marble[variant=0]"),
            (1, "hello112:marble[variant=1]"),
            (2, "hello112:marble[variant=2]")
        ]
    );
    assert_eq!(find("blocks", "hello112:lamp")["states"][0]["light"], 15);
    // Vanilla's own: wool's 16 colours by meta.
    let wool = find("blocks", "minecraft:wool");
    assert_eq!(wool["states"].as_array().unwrap().len(), 16);
    assert_eq!(wool["states"][14]["state"], "minecraft:wool[color=red]");
    assert_eq!(wool["num"], 35);
    // Items: subtypes by meta with the client's names; a damageable tool.
    let gem = find("items", "hello112:gem");
    assert_eq!(gem["subtypes"], true);
    let names: Vec<&str> = gem["variants"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Red Gem", "Blue Gem"]);
    assert_eq!(
        find("items", "hello112:marble")["variants"][2]["name"],
        "Black Marble"
    );
    let wand = find("items", "hello112:wand");
    assert_eq!(
        (wand["max_damage"].as_i64(), wand["stack"].as_i64()),
        (Some(64), Some(1))
    );
    assert_eq!(
        find("items", "minecraft:dye")["variants"][4]["name"],
        "Lapis Lazuli"
    );
    assert_eq!(find("fluids", "hello112_goo")["temperature"], 320);
    assert_eq!(report["dimensions"].as_array().unwrap().len(), 3);

    // The client thread stays Horizon's: commands run there.
    let pong = horizon_jvm::boot112::call(bridge, "ping").unwrap();
    assert_eq!(pong, "pong from Client thread");
    assert!(
        LINES
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("Cleanroom ready in")),
        "Java's log reached Rust"
    );
    eprintln!(
        "Cleanroom ready in {} ms, {} MB heap; {} blocks, {} items",
        report["start_ms"],
        report["heap_mb"],
        report["blocks"].as_array().unwrap().len(),
        report["items"].as_array().unwrap().len()
    );
}
