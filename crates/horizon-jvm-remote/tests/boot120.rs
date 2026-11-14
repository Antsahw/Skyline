//! Fabric 1.20.1 booted from Rust through the socket transport, in a child JVM, with the
//! pinned Fabric API and any mods in `HORIZON_120_MODS`. Needs fetch120.py's folder
//! (`HORIZON_120_CACHE`, default `~/.cache/horizon-120`) and a JDK; passes without them.
//!   python3 -I tools/java_bridge/fetch120.py
//!   cargo test -p horizon-jvm-remote --test boot120 -- --nocapture

mod support;

use horizon_jvm_remote::boot120::{self, Fabric120, Side};
use horizon_jvm_remote::mirror120::{Hopper, inventory_json};
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
        platform: None,
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

/// Grass at y 64 everywhere, and some blocks above it with their block entities.
struct Blocks(Vec<([i32; 3], String, String)>);

impl Blocks {
    fn new(blocks: Vec<([i32; 3], String, String)>) -> Self {
        Self(blocks)
    }
}

impl horizon_jvm_remote::mirror120::World120 for Blocks {
    fn block_state(&self, _dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        if let Some((_, state, _)) = self.0.iter().find(|(p, _, _)| *p == [x, y, z]) {
            return Some(state.clone());
        }
        Some(
            match y {
                64 => "minecraft:grass_block[snowy=false]",
                ..64 => "minecraft:dirt",
                _ => "minecraft:air",
            }
            .into(),
        )
    }
    fn block_entity(&self, _dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        self.0
            .iter()
            .find(|(p, _, _)| *p == [x, y, z])
            .map(|(_, _, e)| e.clone())
            .filter(|e| !e.is_empty())
    }
}

/// Another world, with what the mods told the players kept (`World120::tell`).
struct Told<'a> {
    world: &'a dyn horizon_jvm_remote::mirror120::World120,
    told: std::cell::RefCell<Vec<(String, String, bool)>>,
}

impl horizon_jvm_remote::mirror120::World120 for Told<'_> {
    fn block_state(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        self.world.block_state(dimension, x, y, z)
    }
    fn block_entity(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        self.world.block_entity(dimension, x, y, z)
    }
    fn tell(&self, player: &str, json: &str, overlay: bool) {
        self.told
            .borrow_mut()
            .push((player.to_owned(), json.to_owned(), overlay));
    }
}

/// A flat meadow for features: grass at y 63 over dirt, air above, one biome, and what was set.
struct Meadow {
    biome: String,
    set: Vec<([i32; 3], String)>,
}

impl Meadow {
    fn new(biome: &str) -> Self {
        Self {
            biome: biome.into(),
            set: Vec::new(),
        }
    }
}

impl horizon_jvm_remote::mirror120::Region120 for Meadow {
    fn block_state(&mut self, x: i32, y: i32, z: i32) -> Option<String> {
        if let Some((_, s)) = self.set.iter().rev().find(|(p, _)| *p == [x, y, z]) {
            return Some(s.clone());
        }
        Some(
            match y {
                63 => "minecraft:grass_block[snowy=false]",
                ..63 => "minecraft:dirt",
                _ => "minecraft:air",
            }
            .into(),
        )
    }
    fn height(&mut self, _heightmap: &str, x: i32, z: i32) -> Option<i32> {
        let top = self
            .set
            .iter()
            .filter(|(p, s)| p[0] == x && p[2] == z && !s.ends_with(":air"))
            .map(|(p, _)| p[1] + 1)
            .max()
            .unwrap_or(64);
        Some(top.max(64))
    }
    fn biome(&mut self, _x: i32, _y: i32, _z: i32) -> Option<String> {
        Some(self.biome.clone())
    }
    fn set_block(&mut self, pos: [i32; 3], state: &str) {
        self.set.push((pos, state.to_owned()));
    }
    fn set_block_entity(&mut self, _pos: [i32; 3], _snbt: &str) {}
}

/// A world of one block, stone below it, air elsewhere. At 0 64 0 unless `at` says otherwise:
/// a block entity of the runtime stays alive per position (as a chunk's would), so a case that
/// wants a fresh one takes a position of its own.
struct OneBlock(String, [i32; 3]);

impl OneBlock {
    fn at(state: &str, pos: [i32; 3]) -> Self {
        Self(state.into(), pos)
    }
}

impl horizon_jvm_remote::mirror120::World120 for OneBlock {
    fn block_state(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        assert_eq!(dimension, "minecraft:overworld");
        let [bx, by, bz] = self.1;
        Some(match (x, y, z) {
            p if p == (bx, by, bz) => self.0.clone(),
            p if p == (bx, by - 1, bz) => "minecraft:stone".into(),
            _ => "minecraft:air".into(),
        })
    }
}

/// [`OneBlock`] whose block has a block entity, as Horizon saves it (SNBT).
struct WithEntity {
    block: OneBlock,
    entity: String,
}

impl horizon_jvm_remote::mirror120::World120 for WithEntity {
    fn block_state(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        self.block.block_state(dimension, x, y, z)
    }
    fn block_entity(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        assert_eq!(dimension, "minecraft:overworld");
        let [bx, by, bz] = self.block.1;
        ((x, y, z) == (bx, by, bz)).then(|| self.entity.clone())
    }
}

/// [`flint_and_steel_on_top`] aimed at another block.
fn on_top_of(pos: [i32; 3]) -> horizon_jvm_remote::mirror120::UseBlock {
    let mut request = flint_and_steel_on_top();
    request.pos = pos;
    request.hit = [
        f64::from(pos[0]) + 0.5,
        f64::from(pos[1]) + 1.0,
        f64::from(pos[2]) + 0.5,
    ];
    request
}

fn flint_and_steel_on_top() -> horizon_jvm_remote::mirror120::UseBlock {
    horizon_jvm_remote::mirror120::UseBlock {
        uuid: "6b1f0b8e-2a63-4a2a-9a0c-6c1d2f3e4a5b".into(),
        name: "Tester".into(),
        dimension: "minecraft:overworld".into(),
        pos: [0, 64, 0],
        face: 1,
        hit: [0.5, 65.0, 0.5],
        hand: 0,
        item: "minecraft:flint_and_steel".into(),
        count: 1,
        damage: 0,
        creative: false,
        stance: None,
    }
}

/// Phase 3a: a right-click on a block runs the 1.20.1 code of the block and the item, in the
/// child, on Horizon's blocks; what it sets comes back. Vanilla's campfire, then Farmer's
/// Delight's stove (a mod's own `use`), both lit with flint and steel, which wears by one.
/// Phase 6d: a mod's own mob lives in the runtime. Summoned on Horizon's grass, it lands and
/// stands on it (its physics read Horizon's blocks), ticks its own AI, and keeps its health.
/// Needs build120.sh's platform jar and Friends & Foes among HORIZON_120_MODS.
#[test]
fn a_mods_mob_lives_on_horizons_blocks() {
    let Some(cache) = cache() else { return };
    let Some(bridge) = bridge_classes() else {
        return;
    };
    let platform = cache.join("horizon/platform120.jar");
    let Some(dir) = std::env::var_os("HORIZON_120_MODS") else {
        return;
    };
    let jars: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    if !platform.exists()
        || !jars
            .iter()
            .any(|p| p.to_string_lossy().contains("friendsandfoes"))
    {
        eprintln!("no Friends & Foes in HORIZON_120_MODS (or no platform jar): skipping");
        return;
    }
    let game = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("game120-mobs");
    let _ = std::fs::remove_dir_all(&game);
    let mods = game.join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    for jar in &jars {
        std::fs::copy(jar, mods.join(jar.file_name().unwrap())).unwrap();
    }
    let lines: Lines = Arc::default();
    let mut cfg = config(cache, bridge, game.clone(), lines);
    cfg.platform = Some(platform);
    let (mut runtime, _) = boot120::start(&cfg).expect("boots");
    let world = Blocks::new(Vec::new());
    let id = runtime
        .summon(
            &world,
            "minecraft:overworld",
            "friendsandfoes:moobloom",
            [0.5, 70.0, 0.5],
            None,
        )
        .expect("summon answers")
        .expect("a moobloom is made");
    assert!(id >= 0x7ffe_0000, "in the mods' own id range: {id}");
    let mut last = None;
    for _ in 0..60 {
        let ticked = runtime
            .tick_entities(&world, "minecraft:overworld", &[])
            .expect("its tick");
        last = ticked.entities.into_iter().find(|e| e.id == id);
    }
    let moobloom = last.expect("it is still there after 3 seconds");
    eprintln!("the moobloom after 60 ticks: {moobloom:?}");
    assert_eq!(moobloom.kind, "friendsandfoes:moobloom");
    // Fallen from 70 onto the grass (whose top is 65), and not through it.
    assert!(
        (moobloom.pos[1] - 65.0).abs() < 1e-6,
        "on the grass: {:?}",
        moobloom.pos
    );
    // Phase 6d.2: its own renderer draws it, with its own texture, within its box (the
    // quads are relative to where it stands).
    let drawn: serde_json::Value = serde_json::from_str(
        &runtime
            .render_entities(&world, "minecraft:overworld")
            .expect("its look"),
    )
    .unwrap();
    let meshes = drawn[id.to_string()]
        .as_array()
        .expect("a look for the moobloom");
    let textures: Vec<&str> = meshes.iter().filter_map(|m| m["tex"].as_str()).collect();
    eprintln!(
        "the moobloom draws {} vertices of {textures:?}",
        meshes
            .iter()
            .map(|m| m["v"].as_array().map_or(0, Vec::len) / 9)
            .sum::<usize>()
    );
    assert!(
        textures
            .iter()
            .any(|t| t.starts_with("friendsandfoes:textures/entity/moobloom")),
        "{textures:?}"
    );
    for mesh in meshes {
        for v in mesh["v"].as_array().unwrap().chunks(9) {
            let [x, y, z] = [0, 1, 2].map(|i| v[i].as_f64().unwrap());
            assert!(
                x.abs() < 1.5 && z.abs() < 1.5 && (-0.2..3.0).contains(&y),
                "{x} {y} {z}"
            );
        }
    }
    // Vanilla's fall damage for five blocks (fallDistance 5 - 3): 10 health down to 8.
    assert_eq!(
        moobloom.health,
        Some(8.0),
        "hurt by the fall, alive: {moobloom:?}"
    );

    // Phase 6d.3: a player of Horizon's, three blocks east of it and facing it, uses it and
    // hits it with the mods' code.
    // Which it is now: a world opened again gives it a new id.
    let current = std::cell::Cell::new(id);
    let action = |attack: bool, held: &str| horizon_jvm_remote::mirror120::EntityAction {
        uuid: "00000000-0000-0000-0000-00000000beef".into(),
        name: "Tester".into(),
        dimension: "minecraft:overworld".into(),
        id: current.get(),
        attack,
        off_hand: false,
        held: held.into(),
        creative: false,
        sneaking: false,
        ticks: 100,
        location: [0.0, 1.0, 0.0],
        stance: Some(horizon_jvm_remote::mirror120::Stance {
            feet: [moobloom.pos[0] + 2.0, 65.0, moobloom.pos[2]],
            yaw: 90.0,
            pitch: 20.0,
        }),
    };
    // Cow.mobInteract, which the moobloom keeps: a bucket comes back full of milk.
    let milked = runtime
        .entity_action(
            &world,
            &action(false, r#"{id:"minecraft:bucket",Count:1b}"#),
        )
        .expect("the use answers");
    eprintln!("milking: {milked:?}");
    assert_ne!(milked.result, 0, "the moobloom took the use");
    assert!(milked.held.contains("milk_bucket"), "{}", milked.held);
    // A bare hand's full blow is one point (Player.attack); its invulnerability then keeps
    // further blows away for ten ticks, so a blow lands every eleven.
    let health = |runtime: &mut horizon_jvm_remote::boot120::Runtime120| {
        runtime
            .tick_entities(&world, "minecraft:overworld", &[])
            .expect("its tick")
            .entities
            .into_iter()
            .find(|e| e.id == current.get())
            .and_then(|e| e.health)
    };
    runtime
        .entity_action(&world, &action(true, ""))
        .expect("the blow answers");
    assert_eq!(health(&mut runtime), Some(7.0), "one point off");
    // Phase 6d.4: kept with the world. Saved, gone with a closed world, and back as it was
    // when the world opens again.
    let saved = runtime
        .save_entities(&world, "minecraft:overworld")
        .expect("the save answers");
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert!(saved[0].contains("friendsandfoes:moobloom"), "{}", saved[0]);
    runtime.clear_entities(&world).expect("the clear answers");
    assert_eq!(health(&mut runtime), None, "gone with the world");
    assert_eq!(
        runtime
            .load_entities(&world, "minecraft:overworld", &saved)
            .expect("the load answers"),
        1
    );
    let back = runtime
        .tick_entities(&world, "minecraft:overworld", &[])
        .expect("its tick")
        .entities;
    assert_eq!(back.len(), 1);
    assert_eq!(back[0].kind, "friendsandfoes:moobloom");
    assert_eq!(back[0].health, Some(7.0), "as it was saved");
    current.set(back[0].id);
    // A chunk Horizon hasn't loaded reads as air: a mob in one waits rather than fall.
    struct Unloaded;
    impl horizon_jvm_remote::mirror120::World120 for Unloaded {
        fn block_state(&self, _: &str, _: i32, _: i32, _: i32) -> Option<String> {
            None
        }
    }
    let still = back[0].pos;
    for _ in 0..20 {
        runtime
            .tick_entities(&Unloaded, "minecraft:overworld", &[])
            .expect("its tick");
    }
    let after = runtime
        .save_entities(&world, "minecraft:overworld")
        .expect("the save answers");
    let waited = runtime
        .tick_entities(&world, "minecraft:overworld", &[])
        .expect("its tick")
        .entities;
    assert!(
        (waited[0].pos[1] - still[1]).abs() < 0.5,
        "it waited: {still:?} -> {:?} ({after:?})",
        waited[0].pos
    );
    // In creative, ItemUtils.createFilledResult keeps the empty bucket and adds the milk to the
    // inventory: Horizon is handed it, since its player's inventory isn't the runtime's.
    let milked = runtime
        .entity_action(
            &world,
            &horizon_jvm_remote::mirror120::EntityAction {
                creative: true,
                ..action(false, r#"{id:"minecraft:bucket",Count:1b}"#)
            },
        )
        .expect("the use answers");
    eprintln!("milking in creative: {milked:?}");
    assert!(
        milked.held.contains("minecraft:bucket\""),
        "{}",
        milked.held
    );
    assert!(
        milked.given.iter().any(|g| g.contains("milk_bucket")),
        "{:?}",
        milked.given
    );
    // Its invulnerability keeps further blows away for ten ticks; then a diamond sword's
    // full blow, seven points (1 + its 6), kills it, and it drops its loot.
    for _ in 0..11 {
        health(&mut runtime);
    }
    let hit = runtime
        .entity_action(
            &world,
            &action(
                true,
                r#"{id:"minecraft:diamond_sword",Count:1b,tag:{Damage:0}}"#,
            ),
        )
        .expect("the blow answers");
    eprintln!(
        "its drops: {:?}; the sword after: {}",
        hit.spawned, hit.held
    );
    assert!(
        hit.spawned.iter().any(|s| !s.snbt.is_empty()),
        "killed by a player, it drops its loot"
    );
    // Animal.getExperienceReward: 1 to 3, killed by a player.
    let xp: u32 = hit.spawned.iter().map(|s| s.experience).sum();
    assert!((1..=3).contains(&xp), "and its experience: {xp}");
    assert!(
        hit.held.contains("Damage:1"),
        "the sword wears: {}",
        hit.held
    );
    for _ in 0..25 {
        health(&mut runtime);
    }
    assert_eq!(health(&mut runtime), None, "gone once its death is played");

    // `/summon`: the mods' types are listed for Horizon's commands, and one made with NBT is
    // made from it (no finalizeSpawn), as SummonCommand does.
    let types = runtime.mod_entity_types().expect("the types answer");
    assert!(
        types.iter().any(|t| t == "friendsandfoes:moobloom"),
        "{types:?}"
    );
    assert!(types.iter().all(|t| !t.starts_with("minecraft:")));
    let made = runtime
        .summon(
            &world,
            "minecraft:overworld",
            "friendsandfoes:moobloom",
            [4.5, 65.0, 4.5],
            Some("{Health:4.0f,NoAI:1b}"),
        )
        .expect("summon answers")
        .expect("a moobloom is made");
    let made = runtime
        .tick_entities(&world, "minecraft:overworld", &[])
        .expect("its tick")
        .entities
        .into_iter()
        .find(|e| e.id == made)
        .expect("it's there");
    assert_eq!(made.health, Some(4.0), "as its NBT says");

    // Another of the mod's mobs (a copper golem) stays and ticks, as the moobloom does.
    let golem = runtime
        .summon(
            &world,
            "minecraft:overworld",
            "friendsandfoes:copper_golem",
            [-4.5, 65.0, 4.5],
            None,
        )
        .expect("summon answers")
        .expect("a golem is made");
    for tick in 0..100 {
        let ticked = runtime
            .tick_entities(&world, "minecraft:overworld", &[])
            .expect("its tick");
        assert!(
            ticked.entities.iter().any(|e| e.id == golem),
            "the golem is there at tick {tick}"
        );
    }

    // Phase 6d.6: a natural spawn of the mod's type where Horizon's spawner picked it. On
    // grass in daylight a moobloom spawns (Animal.checkAnimalSpawnRules); in the air it doesn't
    // (its ON_GROUND placement).
    let natural = |runtime: &mut horizon_jvm_remote::boot120::Runtime120, y: f64| {
        runtime
            .spawn_natural(
                &world,
                "minecraft:overworld",
                "friendsandfoes:moobloom",
                [8.5, y, 8.5],
                90.0,
                40.0 * 40.0,
            )
            .expect("the spawn answers")
    };
    let spawned = natural(&mut runtime, 65.0).expect("on grass, it spawns");
    assert!(
        spawned.size[0] > 0.5 && spawned.max_cluster >= 1,
        "{spawned:?}"
    );
    assert_eq!(natural(&mut runtime, 70.0), None, "not in the air");
    // Despawning: a monster (an iceologer) far from every player goes; one near a player stays.
    let near = horizon_jvm_remote::mirror120::PlayerHere {
        uuid: "00000000-0000-0000-0000-00000000beef".into(),
        name: "Tester".into(),
        pos: [-20.0, 65.0, -20.0],
    };
    let iceologer = runtime
        .summon(
            &world,
            "minecraft:overworld",
            "friendsandfoes:iceologer",
            [-20.5, 65.0, -12.5],
            None,
        )
        .expect("summon answers")
        .expect("an iceologer is made");
    let ticked = runtime
        .tick_entities(&world, "minecraft:overworld", std::slice::from_ref(&near))
        .expect("its tick");
    let it = ticked
        .entities
        .iter()
        .find(|e| e.id == iceologer)
        .expect("near a player, it stays");
    assert_eq!(it.category, "monster");
    assert!(it.is_mob && !it.persistent, "{it:?}");
    let far = horizon_jvm_remote::mirror120::PlayerHere {
        pos: [300.0, 65.0, 300.0],
        ..near
    };
    let ticked = runtime
        .tick_entities(&world, "minecraft:overworld", &[far])
        .expect("its tick");
    assert!(
        ticked.entities.iter().all(|e| e.id != iceologer),
        "more than 128 blocks from every player, it went"
    );
    assert!(
        ticked.entities.iter().any(|e| e.id == golem),
        "a golem (persistent as golems are) doesn't"
    );
    // Where it spawns: Friends & Foes adds its moobloom to vanilla's biomes in code
    // (BiomeModifications.addSpawn), reported with its category for Horizon's spawn lists.
    let changes: serde_json::Value = serde_json::from_str(
        &runtime
            .call_with("biomeChanges", &[])
            .expect("the mods' biome changes"),
    )
    .unwrap();
    let moobloom = changes["spawns"]
        .as_object()
        .expect("spawns")
        .iter()
        .flat_map(|(biome, list)| list.as_array().unwrap().iter().map(move |s| (biome, s)))
        .find(|(_, s)| s["type"] == "friendsandfoes:moobloom")
        .expect("the moobloom is added to a biome's spawns");
    eprintln!("the moobloom spawns in {}: {}", moobloom.0, moobloom.1);
    assert_eq!(moobloom.1["category"], "creature");
    assert!(moobloom.1["weight"].as_i64().unwrap() > 0);

    // Phase 5f: the sounds the mods' code plays reach Horizon, by name, where they are. A
    // moobloom milked plays the milking sound (the hurt sound is the client's, from 1.19.4's
    // damage event, so a blow plays none on the server).
    struct Hearing<'a> {
        world: &'a dyn horizon_jvm_remote::mirror120::World120,
        heard: std::cell::RefCell<Vec<horizon_jvm_remote::mirror120::PlayedSound>>,
        seen: std::cell::RefCell<Vec<horizon_jvm_remote::mirror120::SentParticles>>,
        added: std::cell::RefCell<Vec<String>>,
    }
    impl horizon_jvm_remote::mirror120::World120 for Hearing<'_> {
        fn block_state(&self, d: &str, x: i32, y: i32, z: i32) -> Option<String> {
            self.world.block_state(d, x, y, z)
        }
        fn play_sound(&self, sound: &horizon_jvm_remote::mirror120::PlayedSound) {
            self.heard.borrow_mut().push(sound.clone());
        }
        fn send_particles(&self, particles: &horizon_jvm_remote::mirror120::SentParticles) {
            self.seen.borrow_mut().push(particles.clone());
        }
        fn spawn_entity(&self, _: &str, snbt: &str) {
            self.added.borrow_mut().push(snbt.to_owned());
        }
    }
    let hearing = Hearing {
        world: &world,
        heard: Default::default(),
        seen: Default::default(),
        added: Default::default(),
    };
    let target = runtime
        .summon(
            &world,
            "minecraft:overworld",
            "friendsandfoes:moobloom",
            [-4.5, 65.0, -4.5],
            None,
        )
        .expect("summon answers")
        .expect("a moobloom is made");
    current.set(target);
    runtime
        .entity_action(
            &hearing,
            &action(false, r#"{id:"minecraft:bucket",Count:1b}"#),
        )
        .expect("the use answers");
    // And particles: shears on a moobloom turn it into a cow with a puff (MushroomCow.shear's
    // sendParticles of an explosion, which Friends & Foes' moobloom keeps).
    runtime
        .entity_action(
            &hearing,
            &action(false, r#"{id:"minecraft:shears",Count:1b,tag:{Damage:0}}"#),
        )
        .expect("the shears answer");
    let seen = hearing.seen.borrow().clone();
    eprintln!("shearing showed: {seen:?}");
    let puff = seen
        .iter()
        .find(|p| p.options.starts_with("minecraft:explosion"))
        .expect("the shearing's puff");
    assert!(puff.count >= 1, "{puff:?}");
    // What it turned into is a vanilla cow: Horizon's to keep, handed over as 1.20.1 saves it.
    let added = hearing.added.borrow().clone();
    assert!(
        added.iter().any(|e| e.contains("id:\"minecraft:cow\"")),
        "the cow it became: {added:?}"
    );
    let heard = hearing.heard.into_inner();
    eprintln!("milking sounded: {heard:?}");
    let milk = heard
        .iter()
        .find(|s| s.sound == "minecraft:entity.cow.milk")
        .expect("Cow.mobInteract's milking sound");
    assert_eq!(milk.dimension, "minecraft:overworld");
    // Played by the player, where the player stands.
    let stood = action(false, "").stance.expect("a stance").feet;
    assert_eq!(milk.pos, stood, "{milk:?}");
    assert_eq!(milk.source, 7, "SoundSource.PLAYERS");
}

/// Needs build120.sh's platform jar, and Farmer's Delight among HORIZON_120_MODS for the stove
/// (the other mods there are loaded too).
#[test]
fn right_clicking_a_block_runs_its_1_20_1_code_on_horizons_blocks() {
    use horizon_jvm_remote::mirror120::SetBlock;
    let Some(cache) = cache() else { return };
    let Some(bridge) = bridge_classes() else {
        return;
    };
    let platform = cache.join("horizon/platform120.jar");
    if !platform.exists() {
        eprintln!(
            "no {}: run tools/java_bridge/build120.sh (skipping)",
            platform.display()
        );
        return;
    }
    // By the file's name with case, dashes and underscores ignored: Farmer's Delight ships as
    // `farmers-delight-fabric-1.4.3.jar` and as `FarmersDelight-1.20.1-2.5.7+refabricated.jar`,
    // and matching only the first quietly skipped every case below on the second.
    let jar_named = |needle: &str| -> Option<PathBuf> {
        let plain = |s: &str| s.to_lowercase().replace(['-', '_'], "");
        std::fs::read_dir(std::env::var_os("HORIZON_120_MODS")?)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| {
                p.file_name()
                    .is_some_and(|n| plain(&n.to_string_lossy()).contains(&plain(needle)))
            })
    };
    let delight = jar_named("farmers-delight");
    let wired = jar_named("wired-redstone");
    let game = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("game120-use");
    let _ = std::fs::remove_dir_all(&game);
    let mods = game.join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    // All of HORIZON_120_MODS: the mirror has to hold with other mods' Mixins in the same
    // classes (Lithium's in Level, ...).
    // Whatever is in it, not only the jar a case below looks for: a set without Farmer's
    // Delight used to copy nothing at all, so every mod case quietly tested vanilla.
    if let Some(dir) = std::env::var_os("HORIZON_120_MODS") {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            std::fs::copy(&path, mods.join(path.file_name().unwrap())).unwrap();
        }
    }
    let lines: Lines = Arc::default();
    let seen = Arc::clone(&lines);
    let mut cfg = config(cache, bridge, game.clone(), lines);
    cfg.platform = Some(platform);
    let (mut runtime, _) = boot120::start(&cfg).expect("boots");

    // A mod's screen draws through `GuiGraphics`, whose calls the recorder takes and Horizon
    // then draws (`game/java_screen.rs`, shared with the 26.3 bridge). This is that recording
    // end to end: the mixins bound to the right methods of 1.20.1's `GuiGraphics`, and each
    // operation carries what Horizon reads. Without them the call reaches vanilla's drawing
    // and fails on an allocated object's null state instead of answering.
    {
        let json = runtime.screen_probe().expect("the recording answers");
        let ops: serde_json::Value = serde_json::from_str(&json).expect("operations as JSON");
        let ops = ops.as_array().expect("an array of operations");
        let kinds: Vec<&str> = ops
            .iter()
            .filter_map(|o| o.get("op").and_then(serde_json::Value::as_str))
            .collect();
        assert_eq!(
            kinds,
            ["fill", "gradient", "scissor", "text", "unscissor"],
            "every call recorded, in drawing order: {json}"
        );
        // The rectangle as Horizon's screen drawing reads one: its two corners.
        assert_eq!(
            (&ops[0]["x0"], &ops[0]["y0"], &ops[0]["x1"], &ops[0]["y1"]),
            (&10.into(), &20.into(), &110.into(), &70.into()),
            "{json}"
        );
        assert_eq!(ops[3]["text"], "Cooking Pot", "{json}");
        // A gradient's colours, top then bottom: 1.20.1 orders the RenderType overload's
        // arguments differently from fill's, and reading them fill's way shifts both by one.
        assert_eq!(
            (&ops[1]["top"], &ops[1]["bottom"]),
            (
                &(0xFFFF_FFFF_u32 as i32).into(),
                &(0xFF00_0000_u32 as i32).into()
            ),
            "{json}"
        );
    }

    let world = OneBlock(
        "minecraft:campfire[facing=north,lit=false,signal_fire=false,waterlogged=false]".into(),
        [0, 64, 0],
    );
    let used = runtime
        .use_block(&world, &flint_and_steel_on_top())
        .expect("campfire");
    // CONSUME: a server's answer (the swing is the client's).
    assert_eq!(used.result, 1, "{used:?}");
    assert_eq!(
        (used.item.as_str(), used.count, used.damage),
        ("minecraft:flint_and_steel", 1, 1)
    );
    assert_eq!(
        used.set,
        [SetBlock {
            dimension: "minecraft:overworld".into(),
            pos: [0, 64, 0],
            state: "minecraft:campfire[facing=north,lit=true,signal_fire=false,waterlogged=false]"
                .into(),
            flags: 11,
        }]
    );

    // Air under the hand: nothing to light, nothing set; flint and steel on air's top face
    // would make fire above, so use a face of stone instead: fire goes beside it.
    let world = OneBlock("minecraft:stone".into(), [0, 64, 0]);
    let used = runtime
        .use_block(&world, &flint_and_steel_on_top())
        .expect("stone");
    assert_eq!(used.set.len(), 1, "{used:?}");
    assert_eq!(used.set[0].pos, [0, 65, 0]);
    assert!(used.set[0].state.starts_with("minecraft:fire"), "{used:?}");

    if delight.is_some() {
        let world = OneBlock(
            "farmersdelight:stove[facing=north,lit=false]".into(),
            [0, 64, 0],
        );
        let used = runtime
            .use_block(&world, &flint_and_steel_on_top())
            .expect("stove");
        // Farmer's Delight's own StoveBlock.use: SUCCESS on both sides, unlike vanilla.
        assert_eq!(used.result, 2, "{used:?}");
        assert_eq!(used.damage, 1);
        assert_eq!(used.set.len(), 1, "{used:?}");
        assert_eq!(
            used.set[0].state,
            "farmersdelight:stove[facing=north,lit=true]"
        );
        // A mod's block placed on top of a mod's block: a stove held against a stove. The
        // stove's own click has nothing to do with a stove in the hand (it cooks food on top),
        // so it passes, and the held stove is placed above it.
        let stove = WithEntity {
            block: OneBlock(
                "farmersdelight:stove[facing=north,lit=false]".into(),
                [30, 64, 0],
            ),
            entity: "{id:\"farmersdelight:stove\",x:30,y:64,z:0}".into(),
        };
        let mut held_stove = on_top_of([30, 64, 0]);
        held_stove.item = "farmersdelight:stove".into();
        held_stove.count = 1;
        let stacked = runtime
            .use_block(&stove, &held_stove)
            .expect("a stove on a stove");
        assert!(
            stacked
                .set
                .iter()
                .any(|s| s.pos == [30, 65, 0] && s.state.starts_with("farmersdelight:stove")),
            "the held stove is placed on top: {stacked:?}"
        );
        // The mods' own creative tabs, as their own code fills them, go into the content
        // build's facts: Farmer's Delight's tab, with its cooking pot in it.
        let facts = game.join("facts-tabs.json");
        runtime
            .call_with(
                "facts",
                &[horizon_jvm_remote::wire::Value::Str(
                    facts.to_string_lossy().into_owned(),
                )],
            )
            .expect("the facts");
        let facts: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&facts).expect("facts written"))
                .expect("facts as JSON");
        let tabs = facts["creative_tabs"]
            .as_array()
            .expect("creative tabs in the facts");
        let delight_tab = tabs
            .iter()
            .find(|t| {
                t["id"]
                    .as_str()
                    .is_some_and(|id| id.starts_with("farmersdelight:"))
            })
            .unwrap_or_else(|| panic!("Farmer's Delight's own tab: {tabs:?}"));
        assert!(
            delight_tab["items"]
                .as_array()
                .is_some_and(|i| i.iter().any(|i| i == "farmersdelight:cooking_pot")),
            "with the cooking pot in it: {delight_tab}"
        );

        // A block placed faces the way the player looks: a stove faces the player who places
        // it (the opposite of where they look). The same click from the north and from the
        // south: two stoves facing opposite ways. (The mirror player used to always look
        // south, so every mod's block faced north whoever placed it.)
        let grass = |at: [i32; 3]| OneBlock("minecraft:grass_block[snowy=false]".into(), at);
        let facing_from = |runtime: &mut horizon_jvm_remote::boot120::Runtime120,
                           at: [i32; 3],
                           feet_z: f64,
                           yaw: f32| {
            let mut request = on_top_of(at);
            request.item = "farmersdelight:stove".into();
            request.stance = Some(horizon_jvm_remote::mirror120::Stance {
                feet: [f64::from(at[0]) + 0.5, f64::from(at[1]) + 1.0, feet_z],
                yaw,
                pitch: 45.0,
            });
            let placed = runtime
                .use_block(&grass(at), &request)
                .expect("a stove placed");
            placed
                .set
                .iter()
                .find(|s| s.pos == [at[0], at[1] + 1, at[2]])
                .map(|s| s.state.clone())
                .unwrap_or_else(|| panic!("a stove is placed: {placed:?}"))
        };
        // Standing south of it (z + 3) looking north (yaw 180), then north of it looking south.
        let from_south = facing_from(&mut runtime, [50, 64, 0], 3.5, 180.0);
        let from_north = facing_from(&mut runtime, [52, 64, 0], -2.5, 0.0);
        assert!(from_south.contains("facing=south"), "{from_south}");
        assert!(from_north.contains("facing=north"), "{from_north}");

        // A hopper over a basket pushes into it, and one under it pulls out: Fabric Transfer
        // API's ItemStorage.SIDED on the face it touches, as its patch of vanilla's hopper
        // asks. The basket's block entity is the mod's own, kept by Horizon as opaque NBT,
        // so the item goes in and out through the mods' code and the NBT comes back changed.
        // `basket` in Farmer's Delight 1.4.3; Refabricated renamed it `wooden_basket` and keeps
        // the old name as an alias, so the one id reaches the basket in both.
        let basket = WithEntity {
            block: OneBlock("farmersdelight:basket".into(), [44, 64, 0]),
            entity: "{id:\"farmersdelight:basket\",x:44,y:64,z:0,Items:[]}".into(),
        };
        // An empty first slot and three carrots in the second: one carrot of the second.
        let pushed = runtime
            .hopper_insert(
                &basket,
                "minecraft:overworld",
                [44, 64, 0],
                1,
                &[None, Some(("minecraft:carrot", 3, 0))],
            )
            .expect("a hopper's push");
        assert_eq!(pushed.hopper, Hopper::Moved(1), "{pushed:?}");
        let holding = pushed
            .block_entities
            .iter()
            .find(|b| b.pos == [44, 64, 0])
            .unwrap_or_else(|| panic!("the basket's NBT comes back: {pushed:?}"));
        assert!(holding.snbt.contains("carrot"), "{holding:?}");
        let basket = WithEntity {
            block: OneBlock("farmersdelight:basket".into(), [44, 64, 0]),
            entity: holding.snbt.clone(),
        };
        // A full hopper with no carrots in it takes nothing.
        let full = runtime
            .hopper_extract(
                &basket,
                "minecraft:overworld",
                [44, 64, 0],
                0,
                false,
                &[("minecraft:potato", 1, 0)],
            )
            .expect("a full hopper's pull");
        assert_eq!(full.hopper, Hopper::Nothing, "{full:?}");
        let pulled = runtime
            .hopper_extract(&basket, "minecraft:overworld", [44, 64, 0], 0, true, &[])
            .expect("a hopper's pull");
        match &pulled.hopper {
            Hopper::Moved(item) => assert!(item.contains("carrot"), "{pulled:?}"),
            other => panic!("the carrot comes out: {other:?}"),
        }
        // A block with no storage says so, and Horizon stops asking for a while.
        let stove_storage = runtime
            .hopper_insert(
                &stove,
                "minecraft:overworld",
                [30, 64, 0],
                1,
                &[Some(("minecraft:carrot", 1, 0))],
            )
            .expect("a push into a stove");
        assert_ne!(
            stove_storage.hopper,
            Hopper::Moved(0),
            "a stove takes no carrots from a hopper: {stove_storage:?}"
        );

        // Phase 7b: a block entity drawn by its mod's renderer. A carrot put on a cutting board
        // (its own `use`) is drawn by the board's renderer, through the client's item models.
        let board = OneBlock(
            "farmersdelight:cutting_board[facing=north,waterlogged=false]".into(),
            [0, 64, 0],
        );
        let empty = WithEntity {
            block: OneBlock(board.0.clone(), board.1),
            entity: "{id:\"farmersdelight:cutting_board\",x:0,y:64,z:0}".into(),
        };
        let mut carrot = flint_and_steel_on_top();
        carrot.item = "minecraft:carrot".into();
        carrot.damage = 0;
        let used = runtime
            .use_block(&empty, &carrot)
            .expect("the cutting board");
        let on_board = used
            .block_entities
            .iter()
            .find(|e| e.pos == [0, 64, 0])
            .unwrap_or_else(|| panic!("the board keeps the carrot: {used:?}"));
        let with_carrot = WithEntity {
            block: board,
            entity: on_board.snbt.clone(),
        };
        let drawn: serde_json::Value = serde_json::from_str(
            &runtime
                .render_block_entities(&with_carrot, "minecraft:overworld", &[0, 64, 0])
                .expect("the board draws"),
        )
        .unwrap();
        let meshes = drawn["0,64,0"].as_array().expect("the board is drawn");
        let textures: Vec<&str> = meshes.iter().filter_map(|m| m["tex"].as_str()).collect();
        eprintln!("the board draws {textures:?}");
        assert!(
            textures.iter().any(|t| t.contains("carrot")),
            "the carrot, with its own texture: {textures:?}"
        );
        // And an empty board (another one) draws nothing of its own.
        let bare = WithEntity {
            block: OneBlock(with_carrot.block.0.clone(), [8, 64, 8]),
            entity: "{id:\"farmersdelight:cutting_board\",x:8,y:64,z:8}".into(),
        };
        let nothing: serde_json::Value = serde_json::from_str(
            &runtime
                .render_block_entities(&bare, "minecraft:overworld", &[8, 64, 8])
                .expect("the board draws"),
        )
        .unwrap();
        assert!(nothing.get("8,64,8").is_none(), "{nothing}");

        // Phase 5g: a block's client code (animateTick), run as a client runs it on the
        // blocks around its player: a lit stove smokes, its particles to Horizon.
        struct Watching {
            block: OneBlock,
            seen: std::cell::RefCell<Vec<horizon_jvm_remote::mirror120::SentParticles>>,
        }
        impl horizon_jvm_remote::mirror120::World120 for Watching {
            fn block_state(&self, d: &str, x: i32, y: i32, z: i32) -> Option<String> {
                self.block.block_state(d, x, y, z)
            }
            fn send_particles(&self, p: &horizon_jvm_remote::mirror120::SentParticles) {
                self.seen.borrow_mut().push(p.clone());
            }
        }
        let stove = Watching {
            block: OneBlock(
                "farmersdelight:stove[facing=north,lit=true]".into(),
                [0, 64, 0],
            ),
            seen: Default::default(),
        };
        for _ in 0..40 {
            runtime
                .animate_ticks(&stove, "minecraft:overworld", &[0, 64, 0])
                .expect("the stove's animateTick");
        }
        let smoke = stove.seen.into_inner();
        eprintln!(
            "a lit stove showed {} particle(s): {:?}",
            smoke.len(),
            smoke.first()
        );
        assert!(!smoke.is_empty(), "a lit stove smokes");
        assert!(
            smoke
                .iter()
                .all(|p| (0.0..=1.0).contains(&(p.pos[0] - 0.0))),
            "on the stove: {smoke:?}"
        );

        // Phase 4: a cooking pot opened with an empty hand opens its menu. Vanilla's own
        // openMenu runs on the mirror player -- the menu made, its slots listened to -- and only
        // the packet telling a client to show it goes nowhere; the click reports the menu
        // instead, for Horizon to show its screen. An empty hand, so nothing is taken out.
        let pot = WithEntity {
            block: OneBlock(
                "farmersdelight:cooking_pot[facing=north,support=none,waterlogged=false]".into(),
                [0, 64, 0],
            ),
            entity: "{id:\"farmersdelight:cooking_pot\",x:0,y:64,z:0}".into(),
        };
        let mut empty_hand = flint_and_steel_on_top();
        empty_hand.item = "minecraft:air".into();
        empty_hand.count = 0;
        empty_hand.damage = 0;
        let used = runtime.use_block(&pot, &empty_hand).expect("cooking pot");
        let opened = used
            .opened
            .as_ref()
            .unwrap_or_else(|| panic!("the pot opens its menu: {used:?}"));
        assert_eq!(opened.kind, "farmersdelight:cooking_pot", "{used:?}");
        assert!(
            opened.title.contains("cooking_pot") || opened.title.contains("Cooking Pot"),
            "the menu has the pot's title: {}",
            opened.title
        );
        eprintln!("the cooking pot opened: {opened:?}");

        let no_inventory = inventory_json(&[]);
        // Standing two blocks from it, as a player who just clicked it does.
        let by_the_pot = [0.5, 64.0, 2.5];
        // And its screen draws: Farmer's Delight's own CookingPotScreen, built on the menu
        // the click opened and laid out at a GUI-scaled 1280x720, draws one frame into the
        // recorder. Its background is the pot's own texture, placed where a container screen
        // centres itself; its labels are the pot's title and the player's inventory.
        let frame = runtime
            .screen_frame(
                &pot,
                &empty_hand.uuid,
                &no_inventory,
                by_the_pot,
                427,
                240,
                -1,
                -1,
            )
            .expect("the screen draws")
            .unwrap_or_else(|| panic!("the pot has a screen"));
        eprintln!("a cooking pot's frame: {frame}");
        let ops: serde_json::Value = serde_json::from_str(&frame).expect("operations as JSON");
        let ops = ops.as_array().expect("an array of operations");
        let background = ops
            .iter()
            .find(|o| {
                o["op"] == "blit"
                    && o["texture"]
                        .as_str()
                        .is_some_and(|t| t.contains("farmersdelight") && t.contains("cooking_pot"))
            })
            .unwrap_or_else(|| panic!("the pot's own background is drawn: {frame}"));
        // 176 by 166, as a container screen is, centred on the 427 by 240 screen.
        assert_eq!(background["x0"], (427 - 176) / 2, "centred: {background}");
        assert_eq!(background["y0"], (240 - 166) / 2, "centred: {background}");
        // Behind it, the darkened world, in vanilla's own colours.
        let wash = ops
            .iter()
            .find(|o| o["op"] == "gradient")
            .unwrap_or_else(|| panic!("the world behind is darkened: {frame}"));
        assert_eq!(
            (&wash["top"], &wash["bottom"]),
            (&(-1_072_689_136).into(), &(-804_253_680).into()),
            "{wash}"
        );
        let texts: Vec<&str> = ops
            .iter()
            .filter(|o| o["op"] == "text")
            .filter_map(|o| o["json"].as_str().or(o["text"].as_str()))
            .collect();
        assert!(
            texts
                .iter()
                .any(|t| t.contains("cooking_pot") || t.contains("Cooking Pot")),
            "the pot's title is drawn: {texts:?}"
        );

        // The screen takes input. Five carrots in the first hotbar slot and three potatoes in
        // the second, as Horizon has them: the screen shows them where the hotbar is.
        let mut slots = vec![None; 36];
        slots[0] = Some(("minecraft:carrot", 5, 0));
        slots[1] = Some(("minecraft:potato", 3, 0));
        let inventory = inventory_json(&slots);
        let frame = runtime
            .screen_frame(
                &pot,
                &empty_hand.uuid,
                &inventory,
                by_the_pot,
                427,
                240,
                -1,
                -1,
            )
            .expect("the screen draws")
            .expect("still open");
        let ops: serde_json::Value = serde_json::from_str(&frame).expect("operations as JSON");
        let drawn_at = |item: &str| {
            ops.as_array()
                .into_iter()
                .flatten()
                .find(|o| o["op"] == "item" && o["item"].as_str().is_some_and(|i| i.contains(item)))
                .map(|o| {
                    (
                        o["x"].as_f64().unwrap_or(0.0),
                        o["y"].as_f64().unwrap_or(0.0),
                    )
                })
                .unwrap_or_else(|| panic!("the {item} is drawn in its slot: {frame}"))
        };
        let (carrot, potato) = (drawn_at("carrot"), drawn_at("potato"));
        // The hotbar is the bottom row of a 176x166 container screen, slots 18 apart.
        assert_eq!(potato.0 - carrot.0, 18.0, "{carrot:?} {potato:?}");
        assert_eq!(carrot.1, 37.0 + 142.0, "{carrot:?}");
        // The count over a stack of five; and with the mouse over it, its tooltip.
        let hovered = runtime
            .screen_frame(
                &pot,
                &empty_hand.uuid,
                &inventory,
                by_the_pot,
                427,
                240,
                carrot.0 as i32 + 8,
                carrot.1 as i32 + 8,
            )
            .expect("the screen draws")
            .expect("still open");
        let hovered: serde_json::Value = serde_json::from_str(&hovered).expect("JSON");
        let hovered = hovered.as_array().expect("operations");
        assert!(
            hovered
                .iter()
                .any(|o| o["op"] == "decorations" && o["count"] == "5"),
            "the carrots' count: {hovered:?}"
        );
        // Either as lines (vanilla's container tooltip) or as the stack, for Horizon to make
        // the lines (a screen that calls renderTooltip with the item, as the older Farmer's
        // Delight does): either way, the carrots'.
        let tooltip = hovered
            .iter()
            .find(|o| o["op"] == "tooltip" || o["op"] == "item_tooltip")
            .unwrap_or_else(|| panic!("a tooltip over the carrots: {hovered:?}"));
        let names_them = if tooltip["op"] == "tooltip" {
            tooltip["lines"][0].as_str()
        } else {
            tooltip["item"].as_str()
        };
        assert!(
            names_them.is_some_and(|l| l.contains("carrot")),
            "it names them: {tooltip}"
        );

        // A shift-click on the carrots moves them into the pot, as a server runs that click:
        // the menu's own quickMoveStack, so the pot's ingredient slots take them.
        let click = |kind: &str, (x, y): (f64, f64), mods: i32| {
            serde_json::json!({"kind": kind, "x": x + 8.0, "y": y + 8.0, "button": 0, "mods": mods})
                .to_string()
        };
        let moved = runtime
            .screen_input(
                &pot,
                &empty_hand.uuid,
                &inventory,
                &click("press", carrot, 1),
            )
            .expect("a shift-click");
        assert!(moved.open, "{moved:?}");
        assert_eq!(
            moved.slots,
            [(0, None)],
            "the carrots left the hotbar: {moved:?}"
        );
        assert!(
            moved
                .block_entities
                .iter()
                .any(|b| b.snbt.contains("carrot") && b.snbt.contains("5")),
            "and are in the pot: {moved:?}"
        );
        let pot_with_carrots = moved
            .block_entities
            .iter()
            .find(|b| b.snbt.contains("carrot"))
            .map(|b| b.snbt.clone())
            .expect("the pot's NBT");
        slots[0] = None;
        let inventory = inventory_json(&slots);
        runtime
            .screen_input(
                &pot,
                &empty_hand.uuid,
                &inventory,
                &click("release", carrot, 1),
            )
            .expect("the release");

        // A plain click picks the potatoes up onto the cursor: gone from the slot.
        let picked = runtime
            .screen_input(
                &pot,
                &empty_hand.uuid,
                &inventory,
                &click("press", potato, 0),
            )
            .expect("a click");
        assert_eq!(picked.slots, [(1, None)], "on the cursor now: {picked:?}");
        slots[1] = None;
        let inventory = inventory_json(&slots);
        // The cursor's stack is drawn under the mouse.
        let frame = runtime
            .screen_frame(
                &pot,
                &empty_hand.uuid,
                &inventory,
                by_the_pot,
                427,
                240,
                200,
                100,
            )
            .expect("the screen draws")
            .expect("still open");
        assert!(
            frame.contains("potato"),
            "the carried potatoes are drawn: {frame}"
        );

        // Escape closes the screen, as the screen's own key handling does, and the menu
        // closes as a server closes one: what was on the cursor goes back into the inventory.
        let closed = runtime
            .screen_input(
                &pot,
                &empty_hand.uuid,
                &inventory,
                &serde_json::json!({"kind": "key", "key": 256, "scancode": 9, "mods": 0})
                    .to_string(),
            )
            .expect("Escape");
        assert!(!closed.open, "Escape closes it: {closed:?}");
        assert!(
            closed
                .slots
                .iter()
                .any(|(_, s)| s.as_deref().is_some_and(|s| s.contains("potato"))),
            "the potatoes are back in the inventory: {closed:?}"
        );
        assert_eq!(
            runtime
                .screen_frame(
                    &pot,
                    &empty_hand.uuid,
                    &inventory,
                    by_the_pot,
                    427,
                    240,
                    -1,
                    -1
                )
                .expect("asking"),
            None,
            "no screen once closed"
        );

        // Open again, and walk away: the menu closes, as a server closes one its player can no
        // longer reach.
        let again = runtime.use_block(&pot, &empty_hand).expect("cooking pot");
        assert!(again.opened.is_some(), "opens again: {again:?}");
        assert!(
            runtime
                .screen_frame(
                    &pot,
                    &empty_hand.uuid,
                    &inventory,
                    by_the_pot,
                    427,
                    240,
                    -1,
                    -1
                )
                .expect("asking")
                .is_some(),
            "open by the pot"
        );
        assert_eq!(
            runtime
                .screen_frame(
                    &pot,
                    &empty_hand.uuid,
                    &inventory,
                    [40.5, 64.0, 40.5],
                    427,
                    240,
                    -1,
                    -1
                )
                .expect("asking"),
            None,
            "closed once out of reach"
        );

        // Phase 5: breaking the pot runs 1.20.1's own break (ServerPlayerGameMode.destroyBlock):
        // the carrots it holds come out with it, and the pot drops itself by its own loot
        // table. Horizon keeps the block entity as the mod's opaque NBT, so before this the
        // carrots were simply gone with the block.
        let full_pot = WithEntity {
            block: OneBlock(
                "farmersdelight:cooking_pot[facing=north,support=none,waterlogged=false]".into(),
                [0, 64, 0],
            ),
            entity: pot_with_carrots,
        };
        let broke = runtime
            .break_block(
                &full_pot,
                &horizon_jvm_remote::mirror120::BreakBlock {
                    uuid: empty_hand.uuid.clone(),
                    name: empty_hand.name.clone(),
                    dimension: "minecraft:overworld".into(),
                    pos: [0, 64, 0],
                    state: String::new(),
                    held: String::new(),
                    creative: false,
                    stance: None,
                    how: horizon_jvm_remote::mirror120::BreakHow::Theirs,
                },
            )
            .expect("breaking the pot");
        assert!(broke.goes, "{broke:?}");
        assert!(
            broke
                .set
                .iter()
                .any(|s| s.pos == [0, 64, 0] && s.state.starts_with("minecraft:air")),
            "the pot is gone: {broke:?}"
        );
        let dropped: Vec<&str> = broke.spawned.iter().map(|d| d.snbt.as_str()).collect();
        assert!(
            dropped.iter().any(|d| d.contains("carrot")),
            "its carrots drop: {dropped:?}"
        );
        assert!(
            dropped.iter().any(|d| d.contains("cooking_pot")),
            "and the pot itself: {dropped:?}"
        );
    } else {
        eprintln!("no Farmer's Delight in HORIZON_120_MODS: the stove is skipped");
    }

    // Phase 3b: block entities. A jukebox is Horizon's block entity read by 1.20.1's own code
    // and changed by it, with no menu or mod in the way: `RecordItem.useOn` asks the level for
    // the jukebox's block entity (a mirror without them would pass instead, and the disc would
    // stay in hand) and sets the disc on it. The disc is used up, and the block entity comes
    // back holding it.
    //
    // Taking it back out drops it into the world: `JukeboxBlock.use` hands the record to
    // `Level.addFreshEntity`, which the mirror sends on for Horizon to spawn.
    let world = WithEntity {
        block: OneBlock("minecraft:jukebox[has_record=false]".into(), [0, 64, 0]),
        entity: "{id:\"minecraft:jukebox\",x:0,y:64,z:0}".into(),
    };
    let mut with_disc = flint_and_steel_on_top();
    with_disc.item = "minecraft:music_disc_13".into();
    let used = runtime.use_block(&world, &with_disc).expect("jukebox in");
    assert!(used.result != 0, "the disc goes in: {used:?}");
    assert_eq!(used.count, 0, "the disc is used up: {used:?}");
    let filled = used
        .block_entities
        .iter()
        .find(|e| e.pos == [0, 64, 0])
        .unwrap_or_else(|| panic!("the jukebox's block entity comes back: {used:?}"));
    assert!(
        filled.snbt.contains("music_disc_13"),
        "it holds the record: {}",
        filled.snbt
    );

    // Out again, with an empty hand: the block's own `use` clears the jukebox and drops the
    // record, which reaches Horizon as an item to spawn rather than vanishing in the mirror.
    let world = WithEntity {
        block: OneBlock("minecraft:jukebox[has_record=true]".into(), [0, 64, 0]),
        entity: "{id:\"minecraft:jukebox\",x:0,y:64,z:0,RecordItem:{id:\"minecraft:music_disc_13\",Count:1b}}"
            .into(),
    };
    let mut empty_handed = flint_and_steel_on_top();
    empty_handed.item = String::new();
    empty_handed.count = 0;
    let used = runtime
        .use_block(&world, &empty_handed)
        .expect("jukebox out");
    assert_ne!(used.result, 0, "the record comes out: {used:?}");
    assert_eq!(used.set.len(), 1, "{used:?}");
    assert_eq!(used.set[0].state, "minecraft:jukebox[has_record=false]");
    let dropped = used
        .spawned
        .iter()
        .find(|s| s.snbt.contains("music_disc_13"))
        .unwrap_or_else(|| panic!("the record is dropped into the world: {used:?}"));
    assert_eq!(dropped.dimension, "minecraft:overworld");

    // The whole stack crosses, not just its name: a jukebox holding an enchanted, renamed
    // record hands both back, for Horizon to read with vanilla's own fixes. Anything a mod
    // leaves on a stack it ejects (an enchantment, a name, its own tag) rides along with it.
    let world = WithEntity {
        block: OneBlock::at("minecraft:jukebox[has_record=true]", [4, 64, 0]),
        entity:
            "{id:\"minecraft:jukebox\",x:4,y:64,z:0,RecordItem:{id:\"minecraft:music_disc_13\",\
                 Count:1b,tag:{display:{Name:'{\"text\":\"a good mixtape\"}'},\
                 Enchantments:[{id:\"minecraft:unbreaking\",lvl:3s}]}}}"
                .into(),
    };
    let mut empty_elsewhere = on_top_of([4, 64, 0]);
    empty_elsewhere.item = String::new();
    empty_elsewhere.count = 0;
    let used = runtime
        .use_block(&world, &empty_elsewhere)
        .expect("jukebox out");
    let dropped = used
        .spawned
        .iter()
        .find(|s| s.snbt.contains("music_disc_13"))
        .unwrap_or_else(|| panic!("the record is dropped: {used:?}"));
    assert!(
        dropped.snbt.contains("unbreaking"),
        "its enchantment rides along: {}",
        dropped.snbt
    );
    assert!(
        dropped.snbt.contains("mixtape"),
        "its name rides along: {}",
        dropped.snbt
    );

    // Phase 3c: their block entities tick. A lit furnace with fuel and something to smelt is
    // 1.20.1's own ticking code end to end -- it needs the block entity loaded from Horizon,
    // the recipes bound, and what it changes sent back -- with no mod in the way.
    let world = WithEntity {
        block: OneBlock::at("minecraft:furnace[facing=north,lit=true]", [8, 64, 0]),
        entity: "{id:\"minecraft:furnace\",x:8,y:64,z:0,BurnTime:200s,CookTime:0s,\
                 CookTimeTotal:200s,Items:[{Slot:0b,id:\"minecraft:raw_iron\",Count:1b},\
                 {Slot:1b,id:\"minecraft:coal\",Count:1b}]}"
            .into(),
    };
    let ticked = runtime
        .tick_block_entities(&world, "minecraft:overworld", &[8, 64, 0])
        .expect("the furnace ticks");
    assert_eq!(ticked.ticked, 1, "the furnace has a ticker: {ticked:?}");
    let after = ticked
        .block_entities
        .iter()
        .find(|e| e.pos == [8, 64, 0])
        .unwrap_or_else(|| panic!("the furnace changed and came back: {ticked:?}"));
    // One tick of smelting: it burned a tick of fuel and cooked for a tick.
    assert!(
        after.snbt.contains("BurnTime:199s"),
        "a tick of fuel burned: {}",
        after.snbt
    );
    assert!(
        after.snbt.contains("CookTime:1s"),
        "it cooked for a tick: {}",
        after.snbt
    );

    // And it stays alive between ticks. `world` hands out the same untouched NBT every time,
    // so a furnace read from Horizon on each tick would cook its first tick for ever; one kept
    // alive carries on from where it got to.
    let ticked = runtime
        .tick_block_entities(&world, "minecraft:overworld", &[8, 64, 0])
        .expect("the furnace ticks again");
    let after = ticked
        .block_entities
        .iter()
        .find(|e| e.pos == [8, 64, 0])
        .unwrap_or_else(|| panic!("the furnace changed again: {ticked:?}"));
    assert!(
        after.snbt.contains("CookTime:2s"),
        "the second tick carried on from the first: {}",
        after.snbt
    );

    // Reads are batched and kept for the length of a call. A block's own code reads the same
    // position more than once and its neighbours besides, and each read used to be its own
    // round trip; now the first one fetches the 3x3x3 around it and the rest are free.
    //
    // Counted rather than asserted to be fast: `calls_in` is what the runtime asked Horizon
    // for, so a stove's click reading a dozen positions one at a time could not come in under
    // the bound below.
    // Counted on a campfire, which is vanilla's, so the count does not depend on a mod set.
    let world = OneBlock::at(
        "minecraft:campfire[facing=north,lit=false,signal_fire=false,waterlogged=false]",
        [12, 64, 0],
    );
    let before = runtime.child.peer.stats.calls_in;
    let used = runtime
        .use_block(&world, &on_top_of([12, 64, 0]))
        .expect("the campfire lights");
    let asked = runtime.child.peer.stats.calls_in - before;
    assert_ne!(used.result, 0, "it still lights: {used:?}");
    assert!(
        asked <= 6,
        "a click asks Horizon a handful of times, not once per block read: {asked}"
    );
    eprintln!("a campfire's click asked Horizon {asked} time(s)");

    // A block placed from the hand. Placing asks the level whether entities are in the way,
    // and Porting Lib (inside Farmer's Delight Refabricated, Create's ports...) adds its own
    // entities to that answer from a map its mixin puts on the level -- created by a field
    // initializer, which an allocated level never ran, so every block placed with Porting Lib
    // loaded threw. Vanilla's stone, so the case runs with any mod set; with Porting Lib in it,
    // it is that case.
    let world = OneBlock::at("minecraft:grass_block[snowy=false]", [20, 64, 0]);
    let mut stone = on_top_of([20, 64, 0]);
    stone.item = "minecraft:stone".into();
    stone.count = 1;
    let placed = runtime
        .use_block(&world, &stone)
        .expect("a block placed from the hand");
    assert!(
        placed
            .set
            .iter()
            .any(|s| s.pos == [20, 65, 0] && s.state == "minecraft:stone"),
        "the stone is placed on top: {placed:?}"
    );

    // Phase 5c: the mods' own packets between their two halves, both in this runtime. The
    // server sends a player 41; the client half's receiver (on Minecraft.execute, as mods hand
    // their work on) answers 42; the server's receiver (on the server's execute) gets it.
    // Fabric's canSend says yes both ways.
    let probe = runtime
        .packet_probe(&world, "00000000-0000-0000-0000-00000000c0de", "Prober")
        .expect("the packet probe");
    assert_eq!(
        probe,
        horizon_jvm_remote::mirror120::PacketProbe {
            client_got: Some(41),
            server_got: Some(42),
            can_send_down: true,
            can_send_up: true,
        },
        "a packet down and its answer up"
    );

    // Phase 5d: a right-click with an item at nothing runs 1.20.1's own useItem, and a screen
    // the client half opens itself becomes the player's screen. Vanilla's written book takes
    // the path a mod's guide book does: the server opens it with a packet to the player, the
    // client half shows its own BookViewScreen, and that screen draws the book's page.
    let reader = horizon_jvm_remote::mirror120::UseItem {
        uuid: "00000000-0000-0000-0000-0000000b00c5".into(),
        name: "Reader".into(),
        dimension: "minecraft:overworld".into(),
        off_hand: false,
        held: r#"{id:"minecraft:written_book",Count:1b,tag:{title:"T",author:"A",resolved:1b,pages:['"Hello from 1.20.1"']}}"#.into(),
        creative: false,
        stance: None,
    };
    let read = runtime.use_item(&world, &reader).expect("reading a book");
    assert!(read.result > 0, "the book takes the click: {read:?}");
    let opened = read
        .opened
        .as_ref()
        .map(|o| o.kind.as_str())
        .unwrap_or_default();
    assert!(opened.starts_with("screen:"), "its screen opens: {read:?}");
    let page = runtime
        .screen_frame(
            &world,
            &reader.uuid,
            &inventory_json(&[]),
            [0.5, 64.0, 0.5],
            427,
            240,
            -1,
            -1,
        )
        .expect("the book draws")
        .expect("open");
    assert!(page.contains("Hello from 1.20.1"), "with its page: {page}");
    let closed = runtime
        .screen_input(
            &world,
            &reader.uuid,
            &inventory_json(&[]),
            &serde_json::json!({"kind": "key", "key": 256, "scancode": 9, "mods": 0}).to_string(),
        )
        .expect("Escape");
    assert!(!closed.open, "Escape closes it: {closed:?}");
    // The reader leaves: the mods hear it (Fabric's DISCONNECT) and the mirror forgets them.
    assert!(
        runtime.player_left(&world, &reader.uuid).expect("leaving"),
        "they were known"
    );
    assert!(
        !runtime
            .player_left(&world, &reader.uuid)
            .expect("leaving again"),
        "and are forgotten"
    );

    // Phase 5b: a server tick. The mods hear the server start (once) and each tick through
    // Fabric API's ServerTickEvents, each listener on its own; a mod that listens says so.
    // GraphLib (inside Wired Redstone) ticks its graphs at the end of each level tick.
    for time in 100..103 {
        let ticked = runtime
            .server_tick(&world, "minecraft:overworld", time)
            .expect("a server tick");
        if wired.is_some() {
            assert!(ticked.listening, "GraphLib listens to ticks: {ticked:?}");
        }
    }

    // Phase 5e: the mods' key bindings. A binding registered through Fabric's helper is in the
    // list the players are sent; a press held over a client tick is one click to the mod's
    // END_CLIENT_TICK listener, as it is in 1.20.1's own client.
    let prober = "00000000-0000-0000-0000-00000000c0de";
    assert_eq!(runtime.key_probe(&world).expect("the key probe"), 0);
    let keys = runtime.mod_keys(&world).expect("the mods' keys");
    assert!(keys.contains(&75), "K is bound: {keys:?}");
    runtime
        .key_press(&world, prober, "Prober", 75, true)
        .expect("K down");
    runtime
        .server_tick(&world, "minecraft:overworld", 103)
        .expect("a tick with K down");
    runtime
        .key_press(&world, prober, "Prober", 75, false)
        .expect("K up");
    runtime
        .server_tick(&world, "minecraft:overworld", 104)
        .expect("a tick with K up");
    assert_eq!(
        runtime.key_probe(&world).expect("the key probe"),
        1,
        "one click"
    );

    // Phase 6a: the mods' Fabric biome modifications, as data: what each biome's feature lists
    // gained once Fabric API applied them to the runtime's worldgen registries. Farmer's
    // Delight puts its wild crops into vanilla's biomes this way, in code.
    let changes: serde_json::Value = serde_json::from_str(
        &runtime
            .call_with("biomeChanges", &[])
            .expect("the mods' biome changes"),
    )
    .unwrap();
    let added: std::collections::BTreeSet<String> = changes["added"]
        .as_object()
        .unwrap()
        .values()
        .flat_map(|steps| steps.as_object().unwrap().values())
        .flat_map(|list| list.as_array().unwrap().iter())
        .map(|f| f.as_str().unwrap().to_owned())
        .collect();
    println!("biome changes: {added:?}");
    if delight.is_some() {
        let carrots = "farmersdelight:patch_wild_carrots";
        assert!(added.contains(carrots), "{added:?}");
        assert!(
            changes["added"]
                .as_object()
                .unwrap()
                .values()
                .any(|steps| steps["vegetal_decoration"]
                    .as_array()
                    .is_some_and(|l| l.iter().any(|f| f == carrots))),
            "the wild carrots are vegetation"
        );
        assert!(changes["placed"][carrots]["feature"].is_string());
        // Phase 6b: the feature is of FD's own type (`wild_crop`), so Horizon's generator
        // can't run it: the runtime places it, its own code, on the chunks being decorated.
        let features = runtime.mod_features().expect("the mods' features");
        assert!(features.iter().any(|f| f == carrots), "{features:?}");
        let biome = changes["added"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(_, steps)| {
                steps["vegetal_decoration"]
                    .as_array()
                    .is_some_and(|l| l.iter().any(|f| f == carrots))
            })
            .map(|(b, _)| b.clone())
            .unwrap();
        let mut placed_in = None;
        for chunk in 0..2000 {
            let mut region = Meadow::new(&biome);
            let place = horizon_jvm_remote::mirror120::PlaceFeature {
                dimension: "minecraft:overworld".into(),
                feature: carrots.into(),
                world_seed: 42,
                chunk: [chunk, 0],
                index: 0,
                step: 9,
                origin: [chunk * 16, -64, 0],
            };
            if runtime
                .place_feature(&mut region, &place)
                .expect("placing the wild carrots")
            {
                placed_in = Some((chunk, region));
                break;
            }
        }
        let (chunk, region) = placed_in.expect("the wild carrots placed somewhere in 2000 chunks");
        let carrots_set: Vec<_> = region
            .set
            .iter()
            .filter(|(_, s)| s == "farmersdelight:wild_carrots")
            .collect();
        assert!(!carrots_set.is_empty(), "{:?}", region.set);
        // On the grass, near the chunk (the patch spreads 6 out from a column in it).
        for ([x, y, _], _) in &carrots_set {
            assert_eq!(*y, 64, "on top of the grass");
            assert!(
                (chunk * 16 - 8..chunk * 16 + 24).contains(x),
                "{x} near chunk {chunk}"
            );
        }
        println!(
            "wild carrots in chunk {chunk}: {} blocks set, {} carrots",
            region.set.len(),
            carrots_set.len()
        );
    } else if wired.is_some() {
        assert!(added.is_empty(), "Wired Redstone adds nothing: {added:?}");
    }

    // A mod telling the player about something must not break the mod. Vanilla's chest is the
    // same shape with no mod needed: its `use` opens a menu, which sends a packet through the
    // player's listener, and asks the piglins nearby to take offence, which asks the level for
    // its entities. A mirror player has no connection behind that listener and the level has no
    // entity manager, so both were a NullPointerException thrown inside the block's own code.
    //
    // This is what Wired Redstone hit the long way round: placing a cable told the players
    // watching the chunk, through LibNetworkStack, which asks the chunk storage who they are.
    let world = WithEntity {
        block: OneBlock::at(
            "minecraft:chest[facing=north,type=single,waterlogged=false]",
            [16, 64, 0],
        ),
        entity: "{id:\"minecraft:chest\",x:16,y:64,z:0,Items:[]}".into(),
    };
    let mut empty_at_chest = on_top_of([16, 64, 0]);
    empty_at_chest.item = String::new();
    empty_at_chest.count = 0;
    let used = runtime
        .use_block(&world, &empty_at_chest)
        .expect("the chest's own code runs to the end");
    assert_ne!(
        used.result, 0,
        "the chest took the click rather than throwing: {used:?}"
    );

    // The reported case itself, when Wired Redstone is there: placing a bundled cable goes
    // through LibMultipart, which tells the players watching the chunk through LibNetworkStack,
    // which asks the chunk storage for a holder and uses it without checking. Two crashes in a
    // row lived here (a null chunk map, then a null holder).
    if wired.is_some() {
        let ground = OneBlock::at("minecraft:grass_block[snowy=false]", [20, 64, 0]);
        let mut cable = on_top_of([20, 64, 0]);
        cable.item = "wiredredstone:bundled_cable".into();
        cable.count = 4;
        let used = runtime
            .use_block(&ground, &cable)
            .expect("the cable's own placement runs to the end");
        assert_ne!(used.result, 0, "the cable was placed: {used:?}");
        eprintln!("a bundled cable placed: {used:?}");

        // And it ticks. A cable is a node of GraphLib's graph world, which GraphLib builds at
        // the end of ChunkMap's constructor -- the mirror skips constructors, so the cable's
        // first tick used to throw inside the mod (a null graph storage) and the block was
        // left out of every tick after it.
        let placed = used
            .set
            .iter()
            .find(|s| s.state.starts_with("libmultipart:container"))
            .expect("the cable set its container block");
        let entity = used
            .block_entities
            .first()
            .expect("the cable kept its parts in a block entity")
            .snbt
            .clone();
        let world = WithEntity {
            block: OneBlock(placed.state.clone(), placed.pos),
            entity,
        };
        let ticked = runtime
            .tick_block_entities(&world, "minecraft:overworld", &placed.pos)
            .expect("the cable's tick runs to the end");
        let failed: Vec<String> = seen
            .lock()
            .unwrap()
            .iter()
            .filter(|l| l.contains("the tick of"))
            .cloned()
            .collect();
        assert!(failed.is_empty(), "the cable's tick failed: {failed:#?}");
        eprintln!("a bundled cable ticked: {ticked:?}");

        // And it has a shape, which only the mod knows: a container's is the union of the
        // parts in it, so the registry's -- exported with no world and no block entity -- is
        // empty for every one of its states. A cable is a thin plate on the face it was put
        // on, so the boxes are inside the block and none of them fills it.
        let shapes = runtime
            .block_shapes(&world, "minecraft:overworld", &placed.pos)
            .expect("the cable's shape comes back");
        let boxes = shapes.first().expect("one position asked, one answered");
        eprintln!("a bundled cable's shape: {boxes:?}");
        assert!(!boxes.is_empty(), "the cable has a shape of its own");
        for b in boxes {
            for a in 0..3 {
                assert!(
                    b[a] >= 0.0 && b[a + 3] <= 1.0 && b[a] < b[a + 3],
                    "a box inside the block: {b:?}"
                );
            }
            let thin = (0..3).any(|a| b[a + 3] - b[a] < 0.5);
            assert!(thin, "a cable is a plate, not a cube: {b:?}");
        }
        // And the same question about a block with nothing there answers nothing, rather
        // than a box the player could aim at.
        let empty = runtime
            .block_shapes(&world, "minecraft:overworld", &[50, 70, 50])
            .expect("an empty position answers");
        assert_eq!(empty.first().map(Vec::len), Some(0), "{empty:?}");

        // Phase 7b: a cable put beside it joins it. Horizon runs the neighbour updates; the
        // cable already there hears that its neighbour changed (as vanilla's neighbour updater
        // tells it), and LibMultipart passes it to the part, which connects.
        let lone = runtime
            .render_block(&world, "minecraft:overworld", placed.pos)
            .expect("the lone cable draws");
        let first = (placed.pos, placed.state.clone(), world.entity.clone());
        let beside = [placed.pos[0] + 1, placed.pos[1], placed.pos[2]];
        let ground_and_first = Blocks::new(vec![first.clone()]);
        let mut second_cable = on_top_of([beside[0], beside[1] - 1, beside[2]]);
        second_cable.item = "wiredredstone:bundled_cable".into();
        second_cable.count = 4;
        let second_used = runtime
            .use_block(&ground_and_first, &second_cable)
            .expect("the second cable is placed");
        let second_state = second_used
            .set
            .iter()
            .find(|s| s.pos == beside)
            .expect("the second cable set its container")
            .state
            .clone();
        let second_entity = second_used
            .block_entities
            .iter()
            .find(|e| e.pos == beside)
            .expect("the second cable's parts")
            .snbt
            .clone();
        let both = Blocks::new(vec![first.clone(), (beside, second_state, second_entity)]);
        let heard = runtime
            .neighbours(&both, "minecraft:overworld", &beside)
            .expect("the first cable hears its new neighbour");
        assert!(heard.ticked > 0, "{heard:?}");
        // The game ticks the mods' block entities each tick, and the level after them. GraphLib
        // adds a node a tick (16 for a bundled cable), then links the two graphs.
        let positions = [placed.pos, beside].concat();
        let mut entities = horizon_jvm_remote::mirror120::Ticked::default();
        let mut ticked = horizon_jvm_remote::mirror120::ServerTicked::default();
        for time in 200..225 {
            let e = runtime
                .tick_block_entities(&both, "minecraft:overworld", &positions)
                .expect("both cables tick");
            entities.block_entities.extend(e.block_entities);
            let t = runtime
                .server_tick(&both, "minecraft:overworld", time)
                .expect("a tick after");
            ticked.block_entities.extend(t.block_entities);
        }
        let joined = heard
            .block_entities
            .iter()
            .chain(&entities.block_entities)
            .chain(&ticked.block_entities)
            .find(|e| e.pos == placed.pos)
            .unwrap_or_else(|| panic!("the first cable changed: {heard:?} {ticked:?}"));
        assert!(
            !joined.snbt.contains("connections:0b"),
            "it connects now: {}",
            joined.snbt
        );
        // And it draws its arm toward the neighbour (+x): its plate reaches the block's edge.
        let linked_world = Blocks::new(vec![
            (placed.pos, placed.state.clone(), joined.snbt.clone()),
            both.0[1].clone(),
        ]);
        let linked = runtime
            .render_block(&linked_world, "minecraft:overworld", placed.pos)
            .expect("the linked cable draws");
        let reach = |quads: &[horizon_jvm_remote::mirror120::CodeQuad]| {
            quads
                .iter()
                .flat_map(|q| q.corners.iter().map(|c| c[0]))
                .fold(f32::MIN, f32::max)
        };
        assert!(
            reach(&linked) > reach(&lone),
            "linked reaches x {}, lone {}",
            reach(&linked),
            reach(&lone)
        );

        // A red alloy wire beside a redstone block carries its power, and draws it: its sprite
        // is the powered one (Wired Redstone picks the sprite by power, in its model key).
        let wire_at = [placed.pos[0], placed.pos[1], placed.pos[2] + 4];
        let source = (
            [wire_at[0] - 1, wire_at[1], wire_at[2]],
            "minecraft:redstone_block".to_owned(),
            String::new(),
        );
        let mut wire = on_top_of([wire_at[0], wire_at[1] - 1, wire_at[2]]);
        wire.item = "wiredredstone:red_alloy_wire".into();
        wire.count = 4;
        let laid = runtime
            .use_block(&Blocks::new(vec![source.clone()]), &wire)
            .expect("the wire is laid");
        let wire_state = laid
            .set
            .iter()
            .find(|s| s.pos == wire_at)
            .expect("the wire set its container")
            .state
            .clone();
        let mut wire_entity = laid
            .block_entities
            .iter()
            .find(|e| e.pos == wire_at)
            .expect("the wire's parts")
            .snbt
            .clone();
        for time in 300..325 {
            let powered = Blocks::new(vec![
                source.clone(),
                (wire_at, wire_state.clone(), wire_entity.clone()),
            ]);
            let e = runtime
                .tick_block_entities(&powered, "minecraft:overworld", &wire_at)
                .expect("the wire ticks");
            let t = runtime
                .server_tick(&powered, "minecraft:overworld", time)
                .expect("a tick");
            if let Some(changed) = e
                .block_entities
                .iter()
                .chain(&t.block_entities)
                .rfind(|b| b.pos == wire_at)
            {
                wire_entity = changed.snbt.clone();
            }
        }
        assert!(wire_entity.contains("power:15b"), "{wire_entity}");
        let powered = Blocks::new(vec![source, (wire_at, wire_state, wire_entity)]);
        let drawn = runtime
            .render_block(&powered, "minecraft:overworld", wire_at)
            .expect("the wire draws");
        let sprites: std::collections::BTreeSet<&str> =
            drawn.iter().map(|q| q.sprite.as_str()).collect();
        assert!(
            sprites.contains("wiredredstone:block/red_alloy_wire_powered"),
            "{sprites:?}"
        );
        // What Horizon's redstone reads of it: 15 toward a side (a lamp it runs into lights).
        // And nothing to stand on: Wired Redstone's wires and cables on the ground are walked
        // through, like redstone dust (`getCollisionShape` is `Shapes.empty()`; only its
        // standing wires are solid).
        let signals = runtime
            .block_signals(&powered, "minecraft:overworld", &wire_at)
            .expect("the wire's signals");
        assert_eq!(signals.len(), 1);
        assert_eq!(signals[0][..6].iter().max(), Some(&15), "{signals:?}");
        let solid = runtime
            .collision_shapes(&powered, "minecraft:overworld", &wire_at)
            .expect("what is solid of the wire");
        assert_eq!(solid, vec![Vec::<[f64; 6]>::new()], "walked through");
        let sources = runtime.signal_sources().expect("the sources");
        assert!(
            sources.iter().any(|s| s.contains("emits_redstone=true")),
            "LibMultipart's container is a source when it says so: {} sources",
            sources.len()
        );

        // Phase 7: its look is its code. The container's model is a Fabric model made per
        // position from the parts in its block entity, so its states are drawn by code; the
        // cable drawn there is a thin plate of Wired Redstone's own sprites, inside the block.
        let by_code = runtime.drawn_by_code().expect("the states drawn by code");
        assert!(
            by_code
                .iter()
                .any(|s| s.starts_with("libmultipart:container")),
            "{} states: {:?}",
            by_code.len(),
            by_code.iter().take(5).collect::<Vec<_>>()
        );
        let quads = runtime
            .render_block(&world, "minecraft:overworld", placed.pos)
            .expect("the cable draws");
        let sprites: std::collections::BTreeSet<&str> =
            quads.iter().map(|q| q.sprite.as_str()).collect();
        eprintln!(
            "a bundled cable drawn: {} quads of {sprites:?}",
            quads.len()
        );
        assert!(!quads.is_empty(), "the cable draws something");
        assert!(
            sprites.iter().all(|s| s.starts_with("wiredredstone:")),
            "Wired Redstone's own sprites: {sprites:?}"
        );
        for q in &quads {
            for c in &q.corners {
                assert!(
                    (0..3).all(|k| (-1e-3..=1.001).contains(&c[k])),
                    "a corner inside the block: {q:?}"
                );
                assert!(
                    (-1e-3..=1.001).contains(&c[3]) && (-1e-3..=1.001).contains(&c[4]),
                    "UVs inside the sprite: {q:?}"
                );
            }
        }
        let top = quads
            .iter()
            .flat_map(|q| q.corners.iter().map(|c| c[1]))
            .fold(0f32, f32::max);
        assert!(top < 0.5, "a plate on the floor, not a cube: top {top}");
    } else {
        eprintln!("no Wired Redstone in HORIZON_120_MODS: the cable is skipped");
    }
    runtime.stop();
}

/// Phase 3f: energy between the mods' own machines (Team Reborn Energy's `EnergyStorage.SIDED`).
/// Tech Reborn's creative solar panel beside its electric furnace: the panel's tick hands its
/// energy to the furnace through the lookup, which finds the furnace on Horizon's blocks, and
/// the furnace smelts with it. Both block entities stay the runtime's between ticks, as a
/// chunk keeps them, and what they become goes to Horizon. Needs Tech Reborn among
/// HORIZON_120_MODS.
#[test]
fn the_mods_machines_pass_energy_between_them() {
    let Some(cache) = cache() else { return };
    let Some(bridge) = bridge_classes() else {
        return;
    };
    let platform = cache.join("horizon/platform120.jar");
    let Some(dir) = std::env::var_os("HORIZON_120_MODS") else {
        return;
    };
    let jars: Vec<PathBuf> = std::fs::read_dir(&dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .collect();
    if !platform.exists()
        || !jars
            .iter()
            .any(|p| p.to_string_lossy().contains("TechReborn"))
    {
        eprintln!("no Tech Reborn in HORIZON_120_MODS (or no platform jar): skipping");
        return;
    }
    let game = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("game120-energy");
    let _ = std::fs::remove_dir_all(&game);
    let mods = game.join("mods");
    std::fs::create_dir_all(&mods).unwrap();
    for jar in &jars {
        std::fs::copy(jar, mods.join(jar.file_name().unwrap())).unwrap();
    }
    let lines: Lines = Arc::default();
    let mut cfg = config(cache, bridge, game.clone(), lines);
    cfg.platform = Some(platform);
    let (mut runtime, _) = boot120::start(&cfg).expect("boots");
    let world = Blocks::new(vec![
        (
            [0, 65, 0],
            "techreborn:creative_solar_panel".into(),
            // None of Horizon's: the runtime makes the block's own, as a chunk would.
            String::new(),
        ),
        (
            [1, 65, 0],
            "techreborn:electric_furnace[active=false,facing=north]".into(),
            "{id:\"techreborn:electric_furnace\",x:1,y:65,z:0,\
             Items:{Items:[{Slot:0b,id:\"minecraft:raw_iron\",Count:8b}]}}"
                .into(),
        ),
    ]);
    let mut last = String::new();
    for _ in 0..400 {
        let ticked = runtime
            .tick_block_entities(&world, "minecraft:overworld", &[0, 65, 0, 1, 65, 0])
            .expect("a tick");
        if let Some(furnace) = ticked
            .block_entities
            .iter()
            .rev()
            .find(|e| e.pos == [1, 65, 0])
        {
            last = furnace.snbt.clone();
        }
        if last.contains("iron_ingot") {
            break;
        }
    }
    eprintln!("the furnace after: {last}");
    assert!(
        last.contains("minecraft:iron_ingot"),
        "powered by the panel, it smelted: {last}"
    );

    // Fluids (the Transfer API's `FluidStorage`): a water bucket emptied into Tech Reborn's tank
    // unit by a right-click goes into the tank, and the bucket comes back empty.
    let tank = Blocks::new(vec![(
        [0, 65, 0],
        "techreborn:basic_tank_unit[facing=north]".into(),
        String::new(),
    )]);
    let mut bucket = flint_and_steel_on_top();
    bucket.pos = [0, 65, 0];
    bucket.hit = [0.5, 66.0, 0.5];
    bucket.item = "minecraft:water_bucket".into();
    bucket.damage = 0;
    let used = runtime.use_block(&tank, &bucket).expect("the tank");
    eprintln!("the bucket on the tank: {used:?}");
    let kept = used
        .block_entities
        .iter()
        .rev()
        .find(|e| e.pos == [0, 65, 0])
        .unwrap_or_else(|| panic!("the tank changed: {used:?}"));
    assert!(
        kept.snbt.contains("minecraft:water"),
        "water in the tank: {}",
        kept.snbt
    );

    // Phase 5h: the mods' settings screens, as Mod Menu lists them, open for a player and draw
    // as any of their screens.
    let configs = runtime.config_mods().expect("the settings screens");
    eprintln!("settings screens: {configs:?}");
    let (id, _) = configs
        .iter()
        .find(|(id, _)| id == "appleskin" || id == "jade" || id == "waystones")
        .unwrap_or_else(|| panic!("a mod with a settings screen: {configs:?}"));
    let uuid = "00000000-0000-0000-0000-0000000c0f19";
    let kind = runtime
        .open_config(&tank, uuid, "Tester", id)
        .expect("the screen opens")
        .unwrap_or_else(|| panic!("{id}'s settings opened"));
    eprintln!("{id}'s settings: {kind}");
    assert!(kind.starts_with("screen:"), "{kind}");
    let no_inventory = horizon_jvm_remote::mirror120::inventory_json(&[]);
    let frame = runtime
        .screen_frame(
            &tank,
            uuid,
            &no_inventory,
            [0.5, 65.0, 0.5],
            427,
            240,
            -1,
            -1,
        )
        .expect("the screen draws")
        .unwrap_or_else(|| panic!("{id}'s settings draw"));
    eprintln!("{id}'s settings frame: {}", &frame[..frame.len().min(600)]);
    let ops: serde_json::Value = serde_json::from_str(&frame).expect("the frame is json");
    let ops = ops.as_array().expect("a list of drawing operations");
    // Cloth Config clips its list on GL itself, between the header and the footer: the rows
    // scrolled past them must not draw over its buttons.
    if id == "appleskin" {
        assert!(
            ops.iter().any(|o| o["op"] == "scissor"
                && o["y0"].as_i64().unwrap_or(0) > 0
                && o["y1"].as_i64().unwrap_or(240) < 240),
            "the list clipped above the footer: {frame}"
        );
        // A click on the first option's "Yes" turns it to "No", as in the mod's own screen.
        let yes = ops
            .iter()
            .find(|o| o["op"] == "text" && o["text"] == "Yes")
            .expect("a Yes button");
        let (x, y) = (
            yes["x"].as_f64().unwrap() + 4.0,
            yes["y"].as_f64().unwrap() + 3.0,
        );
        for kind in ["move", "press", "release"] {
            let input = serde_json::json!({"kind": kind, "x": x, "y": y, "button": 0, "mods": 0});
            runtime
                .screen_input(&tank, uuid, &no_inventory, &input.to_string())
                .expect("the click");
        }
        let after = runtime
            .screen_frame(
                &tank,
                uuid,
                &no_inventory,
                [0.5, 65.0, 0.5],
                427,
                240,
                x as i32,
                y as i32,
            )
            .expect("the screen draws")
            .expect("still open");
        assert!(
            after.contains("\"text\":\"No\""),
            "the option turned off: {after}"
        );
    }
    assert!(
        frame.contains("\"op\":\"text\""),
        "with its labels: {frame}"
    );
    // In the mod's words: the client's language, with the mods' translations.
    assert!(
        !frame.contains("\"text\":\"text.autoconfig."),
        "translated: {frame}"
    );

    // Phase 5j: the mods' commands, run as a player; what they say goes to that player.
    let names = runtime.command_names().expect("the commands");
    assert!(
        names.iter().any(|n| n == "supplementaries") && names.iter().any(|n| n == "give"),
        "the mods' commands and vanilla's: {names:?}"
    );
    let tree: serde_json::Value = serde_json::from_str(
        &runtime
            .command_tree(&["supplementaries".to_owned()])
            .expect("their tree"),
    )
    .expect("the tree is json");
    assert_eq!(
        tree["children"]["supplementaries"]["children"]["roll"]["children"]["dice"]["parser"],
        "brigadier:integer",
        "the command's arguments, for the players' games: {tree}"
    );
    let told = Told {
        world: &tank,
        told: Default::default(),
    };
    runtime
        .command(
            &told,
            uuid,
            "Tester",
            "minecraft:overworld",
            4,
            "supplementaries roll 6",
        )
        .expect("the command runs");
    let said = told.told.borrow().clone();
    eprintln!("the dice: {said:?}");
    assert!(
        said.iter()
            .any(|(player, json, overlay)| player == uuid && !overlay && json.contains("6")),
        "its answer, to the player who ran it: {said:?}"
    );
    // A line the dispatcher can't parse: vanilla's error, to the player.
    told.told.borrow_mut().clear();
    runtime
        .command(
            &told,
            uuid,
            "Tester",
            "minecraft:overworld",
            4,
            "supplementaries roll",
        )
        .expect("the command runs");
    let said = told.told.borrow().clone();
    assert!(
        said.iter()
            .any(|(_, json, _)| json.contains("command.unknown")
                || json.contains("Unknown or incomplete")),
        "the dispatcher's error: {said:?}"
    );

    // Phase 5k: the mods' effects act on Horizon's numbers. Farmer's Delight's Comfort heals
    // a hurt player with no saturation every 80 ticks, unless Regeneration does; its
    // Nourishment takes the exhaustion back from a player who isn't healing.
    let mut ticks = |health: f32, effects: serde_json::Value, tick: serde_json::Value| {
        let state = serde_json::json!({
            "dimension": "minecraft:overworld", "health": health, "max_health": 20.0,
            "absorption": 0.0, "food": 10, "saturation": 0.0, "exhaustion": 2.0, "air": 300,
            "effects": effects, "tick": tick,
        });
        let after: serde_json::Value = serde_json::from_str(
            &runtime
                .effect_ticks(&tank, uuid, "Tester", &state.to_string())
                .expect("the effects tick"),
        )
        .expect("json");
        after
    };
    let comfort =
        serde_json::json!([{"id": "farmersdelight:comfort", "duration": 1600, "amplifier": 0}]);
    let due = serde_json::json!([{"id": "farmersdelight:comfort", "ticks": 1600, "amplifier": 0}]);
    let healed = ticks(10.0, comfort.clone(), due.clone());
    assert_eq!(healed["health"], 11.0, "comfort heals: {healed}");
    let not_due =
        serde_json::json!([{"id": "farmersdelight:comfort", "ticks": 1601, "amplifier": 0}]);
    assert_eq!(
        ticks(10.0, comfort.clone(), not_due)["health"],
        10.0,
        "between its ticks"
    );
    let regenerating = serde_json::json!([
        {"id": "farmersdelight:comfort", "duration": 1600, "amplifier": 0},
        {"id": "minecraft:regeneration", "duration": 100, "amplifier": 0},
    ]);
    assert_eq!(
        ticks(10.0, regenerating, due)["health"],
        10.0,
        "Regeneration heals instead"
    );
    let nourished = ticks(
        20.0,
        serde_json::json!([{"id": "farmersdelight:nourishment", "duration": 1600, "amplifier": 0}]),
        serde_json::json!([{"id": "farmersdelight:nourishment", "ticks": 1600, "amplifier": 0}]),
    );
    assert_eq!(
        nourished["exhaustion"], 0.0,
        "nourishment takes the exhaustion back: {nourished}"
    );

    // Phase 6e: the mods' fluids run by their own code. Tech Reborn's oil is one of theirs;
    // an oil source on stone, told Horizon placed it, spreads as oil on its tick, not before.
    let fluids = runtime.mod_fluid_blocks().expect("their fluids");
    assert!(fluids.iter().any(|f| f == "techreborn:oil"), "{fluids:?}");
    let looks: serde_json::Value =
        serde_json::from_str(&runtime.fluid_looks().expect("their looks")).expect("json");
    assert_eq!(
        looks["techreborn:oil"]["still"], "techreborn:block/fluids/oil_still",
        "oil drawn with its own sprites: {looks}"
    );
    let pool = Blocks::new(vec![(
        [40, 65, 40],
        "techreborn:oil[level=0]".into(),
        String::new(),
    )]);
    let pending = runtime
        .fluid_changed(&pool, "minecraft:overworld", &[40, 65, 40], 1000)
        .expect("scheduled");
    assert_eq!(pending, 1, "the source's tick");
    let early = runtime
        .fluid_ticks(&pool, "minecraft:overworld", 1000)
        .expect("ticks");
    assert!(early.set.is_empty(), "not due yet: {:?}", early.set);
    let spread = runtime
        .fluid_ticks(&pool, "minecraft:overworld", 1100)
        .expect("ticks");
    eprintln!("oil spread: {:?}", spread.set);
    let beside: Vec<&str> = spread
        .set
        .iter()
        .filter(|s| s.pos[1] == 65)
        .map(|s| s.state.as_str())
        .collect();
    assert_eq!(beside.len(), 4, "to its four sides: {:?}", spread.set);
    assert!(
        beside.iter().all(|s| s.starts_with("techreborn:oil[")),
        "as oil, not water: {beside:?}"
    );
    // Horizon places them, and tells the runtime (its foreign_fluid_changed): their ticks.
    let placed: Vec<i32> = spread.set.iter().flat_map(|s| s.pos).collect();
    let spread_pool = Blocks::new(
        spread
            .set
            .iter()
            .map(|s| (s.pos, s.state.clone(), String::new()))
            .chain([(
                [40, 65, 40],
                "techreborn:oil[level=0]".into(),
                String::new(),
            )])
            .collect(),
    );
    let pending = runtime
        .fluid_changed(&spread_pool, "minecraft:overworld", &placed, 1100)
        .expect("scheduled");
    assert_eq!(pending, 4, "the spread oil's own ticks");

    // Phase 5n: a mod item's own tooltip lines. Farmer's Delight lists a meal's effects
    // under its name (its ConsumableItem's appendHoverText), which Horizon doesn't draw.
    let (hover, after) = runtime
        .tooltip("{id:\"farmersdelight:beef_stew\",Count:1b}", false)
        .expect("the tooltip");
    eprintln!("beef stew's tooltip: {hover} / {after}");
    let hover: serde_json::Value = serde_json::from_str(&hover).expect("json");
    assert!(
        hover.to_string().contains("farmersdelight.comfort"),
        "its effect under its name: {hover}"
    );
    let (vanilla, _) = runtime
        .tooltip("{id:\"minecraft:bread\",Count:1b}", false)
        .expect("the tooltip");
    assert_eq!(vanilla, "[]", "vanilla's own lines are Horizon's");

    // Phase 5i: what the mods draw on the HUD, over Horizon's. A hungry player in survival:
    // AppleSkin's saturation and exhaustion over the food bar, from Horizon's numbers; none
    // of vanilla's own hearts, bars or hotbar, which are Horizon's to draw.
    if configs.iter().any(|(id, _)| id == "appleskin") {
        let state = serde_json::json!({
            "dimension": "minecraft:overworld", "health": 15.0, "max_health": 20.0,
            "absorption": 0.0, "food": 12, "saturation": 4.0, "exhaustion": 2.5, "air": 300,
            "xp_level": 3, "xp_progress": 0.5, "hurtable": true,
        });
        let hud = runtime
            .hud_frame(&tank, uuid, "Tester", &state.to_string(), 427, 240)
            .expect("the HUD draws");
        eprintln!("the mods' HUD: {}", &hud[..hud.len().min(800)]);
        let ops: serde_json::Value = serde_json::from_str(&hud).expect("the HUD is json");
        let ops = ops.as_array().expect("a list of drawing operations");
        let textures: Vec<&str> = ops.iter().filter_map(|o| o["texture"].as_str()).collect();
        assert!(
            textures.iter().any(|t| t.starts_with("appleskin:")),
            "AppleSkin's overlay: {hud}"
        );
        assert!(
            !textures.iter().any(|t| t.ends_with("gui/widgets.png")),
            "not vanilla's hotbar: {hud}"
        );
        // Its saturation overlay sits on the food bar: right of the centre, over the hotbar.
        assert!(
            ops.iter()
                .filter(|o| o["texture"]
                    .as_str()
                    .is_some_and(|t| t.starts_with("appleskin:")))
                .all(|o| o["x0"].as_i64().unwrap_or(0) >= 427 / 2
                    && o["y0"].as_i64().unwrap_or(0) > 240 - 50),
            "on the food bar: {hud}"
        );
    }
}
