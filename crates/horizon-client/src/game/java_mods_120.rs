//! Fabric 1.20.1 mods (docs/java-bridge-120.md, phase 2): when `mods-java-1.20.1/` beside
//! `options.txt` holds jars, their blocks, items and assets become content mods the game loads
//! like any other. The jars run unmodified in a JVM of their own (horizon-jvm-remote's
//! `boot120`), which writes what they register; `content120` turns each jar into a content mod.
//! The result is cached under the jars' stamp, so a start with the same mods starts no Java.
//!
//! Needs, on this machine, what Mojang's files make (never shipped): `fetch120.py`'s folder
//! (`HORIZON_120_CACHE`, default `~/.cache/horizon-120`) and `build120.sh`'s platform jar in it.
//! Without them the game says which command to run and starts without the 1.20.1 mods.
//! Behaviour (phase 3): the runtime stays up after the content is made (or starts in the
//! background when the content came from the cache), and a right-click on one of the mods'
//! blocks, or with one of their items, runs the mods' own code there on this world's blocks
//! (`java120_use_block`, at ServerPlayerGameMode.useItemOn's head), on Horizon's blocks and
//! block entities (phase 3b: a mod's own block entity is kept as opaque NBT). What a mod drops
//! or ejects, ticking machines and menus are the next steps.

use horizon_jvm_remote::boot120::{self, Fabric120, Runtime120, Side};
use horizon_jvm_remote::mirror120::{UseBlock, World120};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The running 1.20.1 runtime, once it has booted (phase 3: the mods' code).
static RUNTIME: Mutex<Option<Runtime120>> = Mutex::new(None);

/// The 1.20.1 mods' namespaces (their content mods' `assets/`): what routes a click to them.
static NAMESPACES: Mutex<Option<HashSet<String>>> = Mutex::new(None);

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
        remember_namespaces(&folders);
        // Their code: the runtime boots beside the game's start (clicks pass until it's up).
        let (dir, home) = (dir.clone(), home.clone());
        let started = std::thread::Builder::new()
            .name("fabric-1.20.1 start".into())
            .spawn(move || match start(&dir, &home) {
                Ok((runtime, _)) => keep(runtime),
                Err(e) => log::warn!("1.20.1 mods: their code doesn't run: {e}"),
            });
        if let Err(e) = started {
            log::warn!("1.20.1 mods: their code doesn't run: {e}");
        }
        return folders;
    }
    match make(&dir, &jars, base, &home, &cache) {
        Ok(folders) => {
            remember_namespaces(&folders);
            folders
        }
        Err(e) => {
            log::warn!("1.20.1 mods: their content was not made: {e}");
            Vec::new()
        }
    }
}

/// The namespaces under the content mods' `assets/`, but vanilla's.
fn remember_namespaces(folders: &[PathBuf]) {
    let names: HashSet<String> = folders
        .iter()
        .flat_map(|f| {
            std::fs::read_dir(f.join("assets"))
                .into_iter()
                .flatten()
                .flatten()
        })
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n != "minecraft")
        .collect();
    *NAMESPACES.lock().unwrap_or_else(|e| e.into_inner()) = Some(names);
}

/// The runtime from now on: the mods' code runs in it.
fn keep(runtime: Runtime120) {
    log::info!("1.20.1 mods: their code runs");
    if let Some(old) = RUNTIME
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .replace(runtime)
    {
        old.stop();
    }
}

/// Starts the 1.20.1 runtime with the jars in `dir` and the platform jar: the runtime and its
/// report.
fn start(dir: &Path, home: &Path) -> Result<(Runtime120, String), String> {
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
    boot120::start(&config)
}

fn make(
    dir: &Path,
    jars: &[PathBuf],
    base: &Path,
    home: &Path,
    cache: &Path,
) -> Result<Vec<PathBuf>, String> {
    let started = std::time::Instant::now();
    log::info!(
        "1.20.1 mods: making the content of {} jar(s) (once for this set of mods)",
        jars.len()
    );
    let (mut runtime, report) = start(dir, home)?;
    log::info!(
        "1.20.1 mods: {} mods loaded",
        boot120::report_mods(&report).len()
    );
    let built = horizon_jvm_remote::content120::build(&mut runtime, jars, base, cache, &|l| {
        log::info!("{l}")
    });
    // Kept: the mods' code runs in it from now on.
    keep(runtime);
    let folders = built?;
    log::info!(
        "1.20.1 mods: {} content mod(s) made in {} ms",
        folders.len(),
        started.elapsed().as_millis()
    );
    Ok(folders)
}

/// `minecraft:oak_log[axis=y]`: a state's name as both versions write it.
fn state_name(
    registry: &horizon_world::registry::BlockRegistry,
    state: horizon_world::registry::BlockStateId,
) -> String {
    let Some(s) = registry.state(state) else {
        return "minecraft:air".into();
    };
    if s.properties.is_empty() {
        return s.name.clone();
    }
    let props: Vec<String> = s
        .properties
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    format!("{}[{}]", s.name, props.join(","))
}

/// `UUID.toString()`.
fn uuid_string(u: u128) -> String {
    let h = format!("{u:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &h[..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..]
    )
}

/// The level running now, as the 1.20.1 mods' code reads it.
struct Level<'a, 'g>(&'a super::Game<'g>);

impl World120 for Level<'_, '_> {
    fn block_state(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        let game = self.0;
        if dimension != game.dim.id() || !game.world.loaded(x, z) {
            return None;
        }
        Some(state_name(
            game.pack.assets.registry,
            game.world.block(x, y, z),
        ))
    }

    fn block_entity(&self, dimension: &str, x: i32, y: i32, z: i32) -> Option<String> {
        let game = self.0;
        if dimension != game.dim.id() || !game.world.loaded(x, z) {
            return None;
        }
        // `saveWithFullMetadata`: with `id` and the position, without the chunk-only
        // `keepPacked`. A 1.20.1 mod's own block entity is opaque NBT here
        // (`BlockEntity::Mod`), so its items and progress cross as the mod wrote them.
        let mut tag = game.block_entity_nbt([x, y, z])?;
        if let horizon_world::nbt::Tag::Compound(c) = &mut tag {
            c.remove(&"keepPacked".into());
        }
        Some(horizon_world::snbt_text::to_snbt(&tag))
    }
}

impl super::Game<'_> {
    /// The player acting now right-clicks the block at `pos` (face `face`, vanilla's
    /// `Direction` id, hit at `point`): when the block or the held item is a 1.20.1 mod's, the
    /// mods' code runs it (vanilla 1.20.1's useItemOn), and what it set and did to the held
    /// stack lands here. Some(swing) when it was used; None to let the game go on (no 1.20.1
    /// mod involved, the runtime not up yet, or the click passed there).
    pub(in crate::game) fn java120_use_block(
        &mut self,
        pos: [i32; 3],
        face: i32,
        point: [f64; 3],
        off_hand: bool,
    ) -> Option<bool> {
        if self.remote.is_some() {
            return None;
        }
        let hand = if off_hand {
            super::Hand::Off
        } else {
            super::Hand::Main
        };
        let held = self.with_hand(hand, |g| g.held().cloned());
        let registry = self.pack.assets.registry;
        let clicked = state_name(registry, self.world.block(pos[0], pos[1], pos[2]));
        let clicked_is_ours;
        {
            let namespaces = NAMESPACES.lock().unwrap_or_else(|e| e.into_inner());
            let ours = |id: &str| {
                id.split_once(':')
                    .is_some_and(|(ns, _)| namespaces.as_ref().is_some_and(|n| n.contains(ns)))
            };
            clicked_is_ours = ours(&clicked);
            if !clicked_is_ours && !held.as_ref().is_some_and(|h| ours(&h.item)) {
                return None;
            }
        }
        let uuid = self.lan_acting_uuid.unwrap_or_else(|| {
            self.level.player_uuid.map_or_else(
                || horizon_protocol::crypto::offline_uuid(super::chat::PLAYER_NAME),
                |[a, b, c, d]| {
                    (u128::from(a as u32) << 96)
                        | (u128::from(b as u32) << 64)
                        | (u128::from(c as u32) << 32)
                        | u128::from(d as u32)
                },
            )
        });
        let request = UseBlock {
            uuid: uuid_string(uuid),
            name: self.acting_player_name().to_owned(),
            dimension: self.dim.id().to_owned(),
            pos,
            face: face.clamp(0, 5) as u8,
            hit: point,
            hand: u8::from(off_hand),
            item: held
                .as_ref()
                .map(|h| h.item.to_string())
                .unwrap_or_default(),
            count: held.as_ref().map_or(0, |h| h.count as i32),
            damage: held.as_ref().map_or(0, |h| h.damage as i32),
            creative: self.mode == horizon_client::command::args::GameMode::Creative,
        };
        let used = {
            // Clicks while the runtime boots (or another call runs) pass.
            let mut guard = RUNTIME.try_lock().ok()?;
            let runtime = guard.as_mut()?;
            runtime.use_block(&Level(self), &request)
        };
        let used = match used {
            Ok(used) => used,
            Err(e) => {
                log::warn!("1.20.1 mods: using {clicked} failed: {e}");
                return None;
            }
        };
        for set in &used.set {
            if set.dimension != self.dim.id() {
                continue;
            }
            match super::render_control::parse_state(registry, &set.state) {
                Ok(state) => {
                    if let Err(e) = self.place_state(set.pos, state, false) {
                        log::warn!("1.20.1 mods: placing {}: {e:?}", set.state);
                    }
                }
                Err(e) => log::warn!("1.20.1 mods: a mod set {}: {e}", set.state),
            }
        }
        // The block entities the mods' code changed (a cooking pot's items and progress):
        // kept as they saved them, after the blocks so one it placed is already there.
        for set in &used.block_entities {
            if set.dimension != self.dim.id() {
                continue;
            }
            self.set_block_entity_120(set.pos, &set.snbt);
        }
        // The held stack as the mods' code left it (worn, used up, turned into another).
        let after = (used.item.as_str(), used.count, used.damage);
        let before = held.as_ref().map_or(("minecraft:air", 0, 0), |h| {
            (&*h.item, h.count as i32, h.damage as i32)
        });
        if after != before {
            self.with_hand(hand, |g| {
                let extra = g.held().and_then(|h| h.extra.clone());
                *g.held_mut() = (used.count > 0 && used.item != "minecraft:air").then(|| {
                    horizon_client::items::ItemStack {
                        item: used.item.as_str().into(),
                        count: used.count as u32,
                        damage: used.damage.max(0) as u32,
                        extra: if used.item == before.0 { extra } else { None },
                    }
                });
            });
        }
        // result 2 = SUCCESS (swing), 1 = CONSUME (no swing), 0 = PASS.
        if used.result != 0 {
            return Some(used.result == 2);
        }
        // Passed with an empty hand on one of their blocks: consumed all the same, so the game
        // goes no further. A click sends both hands, and the off hand is usually empty: its
        // pass used to reach the game's own use-item, which, on a block only the 1.20.1 mods
        // know, undid what the main hand had just done (a lit stove went out again).
        //
        // Only with an empty hand. A pass with something held is the mods' code saying it has
        // no use for it here, and the game goes on to place or use it as usual: that path is
        // `BlockItem.place`, so consuming it here would mean nothing could be placed against a
        // 1.20.1 mod's block at all.
        if clicked_is_ours && held.is_none() {
            return Some(false);
        }
        None
    }

    /// A 1.20.1 mod's block entity as its code saved it (`saveWithFullMetadata` in SNBT): what
    /// is at `pos` from now on. A mod's own type is kept opaquely (`BlockEntity::Mod`), so it
    /// saves and reloads with the world without Horizon knowing the mod.
    fn set_block_entity_120(&mut self, [x, y, z]: [i32; 3], snbt: &str) {
        let tag = match horizon_world::nbt::parse_snbt(snbt) {
            Ok((tag, _)) => tag,
            Err(e) => {
                log::warn!("1.20.1 mods: a block entity at {x} {y} {z}: {e}");
                return;
            }
        };
        let Some((_, entity)) = horizon_client::save::decode_block_entity(&tag) else {
            log::warn!("1.20.1 mods: a block entity at {x} {y} {z} was not read");
            return;
        };
        self.world.block_entities.insert((x, y, z), entity);
        // Redraw and resend what shows it.
        let state = self.world.block(x, y, z);
        let _ = self.world.set(x, y, z, state);
    }
}
