//! Fabric 1.20.1 booted from Rust through the socket transport, in a child JVM, with the
//! pinned Fabric API and any mods in `HORIZON_120_MODS`. Needs fetch120.py's folder
//! (`HORIZON_120_CACHE`, default `~/.cache/horizon-120`) and a JDK; passes without them.
//!   python3 -I tools/java_bridge/fetch120.py
//!   cargo test -p horizon-jvm-remote --test boot120 -- --nocapture

mod support;

use horizon_jvm_remote::boot120::{self, Fabric120, Side};
use horizon_jvm_remote::mirror120::inventory_json;
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
    }
}

/// Phase 3a: a right-click on a block runs the 1.20.1 code of the block and the item, in the
/// child, on Horizon's blocks; what it sets comes back. Vanilla's campfire, then Farmer's
/// Delight's stove (a mod's own `use`), both lit with flint and steel, which wears by one.
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
    let jar_named = |needle: &str| -> Option<PathBuf> {
        std::fs::read_dir(std::env::var_os("HORIZON_120_MODS")?)
            .ok()?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.to_string_lossy().contains(needle))
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
    let mut cfg = config(cache, bridge, game, lines);
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
    } else {
        eprintln!("no Wired Redstone in HORIZON_120_MODS: the cable is skipped");
    }
    runtime.stop();
}
