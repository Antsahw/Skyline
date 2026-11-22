//! The 1.20.1 mods' own mobs (phase 6d). They live in the runtime (`Entities120`): the server
//! half ticks them each tick (their AI, paths and physics on Horizon's blocks), has their own
//! renderers draw them (`Render120`: quads with their textures, relative to each mob) and sends
//! each player those in its dimension on [`CHANNEL`]. The window draws the quads where the mob
//! is, moving between the last two ticks as vanilla's renderer does.

use super::super::{Game, lan::LanHost};
use super::breaking::stack_to_120;
use super::{Level, runtime, tick_failed_once, wait_for_runtime};
use horizon_client::mesher::Vertex;
use horizon_core::geometry::{Aabb, Vec3};
use horizon_jvm_remote::boot120::Loader;
use horizon_jvm_remote::mirror120::EntityAction;
use horizon_protocol::packets::configuration::CustomPayload;
use horizon_protocol::packets::play::Interact;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Server to game, each tick there are mobs: `{"e": [{"id", "kind", "p", "r", "s", "h", "m"}]}`:
/// position, rotations (yaw, pitch, head, body), box (width, height), health and meshes
/// (`Runtime120::render_entities`' per entity).
pub(in crate::game) const CHANNEL: &str = "horizon:java120_entities";

/// While the runtime has no mob, it is asked again this often (in ticks): a spawn egg used
/// meanwhile shows up within a second.
const IDLE_EVERY: u64 = 20;

/// The server half: how many mobs the last tick had, by runtime (`Loader::index`).
static LAST_COUNTS: [Mutex<usize>; 2] = [Mutex::new(0), Mutex::new(0)];

/// Each runtime's mobs' last looks (`renderEntities`' meshes), by id: a far one is drawn only
/// every few ticks.
static LOOKS: [Mutex<Option<HashMap<i32, Box<serde_json::value::RawValue>>>>; 2] =
    [Mutex::new(None), Mutex::new(None)];

/// Each runtime's mobs as last sent to the players' games.
static LAST_ENTRIES: [Mutex<Vec<String>>; 2] = [Mutex::new(Vec::new()), Mutex::new(Vec::new())];

/// The calls' runtime's count in [`LAST_COUNTS`].
fn last_count() -> &'static Mutex<usize> {
    &LAST_COUNTS[super::current().index()]
}

/// The mods' entities' ids (`Entities120.FIRST_ID` and up): Horizon's own are below, the 26.3
/// bridge's (`java_entities::FIRST_ID`) above.
pub(in crate::game) const IDS: std::ops::Range<i32> = 0x7ffe_0000..0x7fff_0000;

/// The server half: each dimension's mobs after its last tick (a click's range check, the
/// mob caps' count), by runtime.
static SEEN: Mutex<Option<HashMap<(Loader, String), Vec<Seen>>>> = Mutex::new(None);

/// One of the mods' mobs as the server half last saw it.
#[derive(Clone)]
pub(in crate::game) struct Seen {
    pub(in crate::game) id: i32,
    pub(in crate::game) kind: String,
    pub(in crate::game) category: String,
    pub(in crate::game) pos: [f64; 3],
    pub(in crate::game) is_mob: bool,
    pub(in crate::game) persistent: bool,
}

/// The mods' mobs of a dimension after its last tick, for the mob caps (`natural_spawning`).
pub(in crate::game) fn seen_in(dimension: &str) -> Vec<Seen> {
    let all = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    let Some(all) = all.as_ref() else {
        return Vec::new();
    };
    Loader::ALL
        .iter()
        .filter_map(|l| all.get(&(*l, dimension.to_owned())))
        .flatten()
        .cloned()
        .collect()
}

/// Whether a type is one of the mods' (their runtime makes it).
pub(in crate::game) fn theirs(kind: &str) -> bool {
    TYPES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|t| t == kind)
}

/// The mods' entity types, from when their runtimes started: what `/summon` makes there.
static TYPES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A runtime is up: its mods' entity types, for Horizon's commands, beside the other's.
pub(super) fn know_types(runtime: &mut horizon_jvm_remote::boot120::Runtime120) {
    match runtime.mod_entity_types() {
        Ok(types) => TYPES
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .extend(types),
        Err(e) => log::warn!("1.20.1 mods: their entity types can't be listed: {e}"),
    }
}

/// The server half: which world's mobs the runtime holds (its folder), and the dimensions of it
/// whose saved mobs are in already. A dimension's save is written only once it is in, so a
/// save before that never writes over the mobs it would have loaded.
static HELDS: [Mutex<Option<(PathBuf, HashSet<String>)>>; 2] = [Mutex::new(None), Mutex::new(None)];

/// The calls' runtime's [`HELDS`].
fn held() -> &'static Mutex<Option<(PathBuf, HashSet<String>)>> {
    &HELDS[super::current().index()]
}

/// Where a dimension's mobs are kept, beside its other data (`raids.dat`): the Fabric runtime's
/// and the Forge one's each in a file of its own.
fn kept_at(root: &Path, dimension: &str) -> PathBuf {
    let file = match super::current() {
        Loader::Fabric => "data/skyline/entities120.json",
        Loader::Forge => "data/skyline/entities120-forge.json",
    };
    horizon_client::save::dimension_dir(root, dimension).join(file)
}

/// 1.20.1's `ServerGamePacketListenerImpl.MAX_INTERACTION_DISTANCE`: a player further than six
/// blocks from an entity can't hit or use it.
const REACH_SQUARED: f64 = 36.0;

/// One mob as the window has it.
#[derive(Clone)]
struct Mob {
    kind: String,
    health: Option<f64>,
    pos: [f64; 3],
    size: [f32; 2],
    meshes: Vec<Mesh>,
}

#[derive(Clone)]
pub(super) struct Mesh {
    /// Its texture's key in the GUI atlas (`friendsandfoes:entity/moobloom/moobloom`).
    sprite: String,
    overlay: u32,
    /// Per vertex: x, y, z (relative to the mob), u, v, the normal, ARGB.
    vertices: Vec<[f32; 9]>,
}

/// The window: the mobs of the last two ticks, by id.
#[derive(Default)]
struct Window {
    now: HashMap<i32, Mob>,
    before: HashMap<i32, Mob>,
}

static WINDOW: Mutex<Option<Window>> = Mutex::new(None);

/// The latest tick's mobs not read yet ([`read_mobs`]): a newer one replaces it unread.
static MOBS_IN: (Mutex<Option<Vec<u8>>>, std::sync::Condvar) =
    (Mutex::new(None), std::sync::Condvar::new());

/// A tick's mobs as the server sends them (`{"e": [...]}`).
#[derive(serde::Deserialize)]
struct MobsIn {
    e: Vec<MobIn>,
}

#[derive(serde::Deserialize)]
struct MobIn {
    id: i64,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    p: [f64; 3],
    #[serde(default)]
    s: [f32; 2],
    #[serde(default)]
    h: Option<f64>,
    #[serde(default)]
    m: Option<Vec<MeshIn>>,
}

#[derive(serde::Deserialize)]
struct MeshIn {
    tex: String,
    #[serde(default)]
    o: i64,
    v: Vec<f32>,
}

/// The mobs' reader: each tick's JSON into the window, off the frame's thread.
fn read_mobs() {
    let (slot, ready) = &MOBS_IN;
    loop {
        let data = {
            let mut waiting = slot.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if let Some(data) = waiting.take() {
                    break data;
                }
                waiting = ready.wait(waiting).unwrap_or_else(|e| e.into_inner());
            }
        };
        let Ok(read) = serde_json::from_slice::<MobsIn>(&data) else {
            continue;
        };
        let now: HashMap<i32, Mob> = read
            .e
            .into_iter()
            .map(|e| {
                let meshes =
                    e.m.unwrap_or_default()
                        .into_iter()
                        .map(|m| Mesh {
                            sprite: sprite_key(&m.tex),
                            overlay: m.o as u32,
                            vertices: m
                                .v
                                .chunks_exact(9)
                                .map(|c| std::array::from_fn(|i| c[i]))
                                .collect(),
                        })
                        .collect();
                (
                    e.id as i32,
                    Mob {
                        kind: e.kind,
                        health: e.h,
                        pos: e.p,
                        size: e.s,
                        meshes,
                    },
                )
            })
            .collect();
        let mut window = WINDOW.lock().unwrap_or_else(|e| e.into_inner());
        let window = window.get_or_insert_with(Window::default);
        window.before = std::mem::replace(&mut window.now, now);
    }
}

/// A texture's name (`ns:textures/entity/x.png`) as the GUI atlas keys it (`ns:entity/x`).
fn sprite_key(id: &str) -> String {
    let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
    let path = path
        .trim_start_matches("textures/")
        .trim_end_matches(".png");
    if ns == "minecraft" {
        path.to_owned()
    } else {
        format!("{ns}:{path}")
    }
}

/// Java's ARGB as a vertex colour (ABGR).
fn abgr(argb: u32) -> u32 {
    let (a, r, g, b) = (
        argb >> 24,
        (argb >> 16) & 255,
        (argb >> 8) & 255,
        argb & 255,
    );
    a << 24 | b << 16 | g << 8 | r
}

/// Java's packed overlay as Horizon's overlay code: v below 8 is the hurt flash's red.
fn overlay_code(packed: u32) -> u32 {
    let (u, v) = (packed & 0xFFFF, packed >> 16);
    if v < 8 {
        1
    } else if u == 0 {
        0
    } else {
        horizon_client::overlay::WHITE + u.min(15)
    }
}

/// Meshes as the runtime writes them (`[{"tex", "o", "v"}...]`).
pub(super) fn parse_meshes(json: &Value) -> Vec<Mesh> {
    json.as_array()
        .into_iter()
        .flatten()
        .filter_map(|m| {
            let raw = m["v"].as_array()?;
            Some(Mesh {
                sprite: sprite_key(m["tex"].as_str()?),
                overlay: m["o"].as_u64().unwrap_or(0) as u32,
                vertices: raw
                    .chunks_exact(9)
                    .map(|c| std::array::from_fn(|i| c[i].as_f64().unwrap_or(0.0) as f32))
                    .collect(),
            })
        })
        .collect()
}

/// The quads of two ticks' meshes part of the way from `from` to `to` (`t`: 0 to 1), when they
/// are the same quads (each mesh's sprite and vertex count): a mob's limbs and head move
/// smoothly between ticks, as vanilla's renderer poses them at the partial tick. `None` when the
/// mob's quads changed (a part shown or hidden), to draw `to` as it is.
pub(super) fn lerp_meshes(from: &[Mesh], to: &[Mesh], t: f32) -> Option<Vec<Mesh>> {
    if from.len() != to.len()
        || from
            .iter()
            .zip(to)
            .any(|(a, b)| a.sprite != b.sprite || a.vertices.len() != b.vertices.len())
    {
        return None;
    }
    Some(
        from.iter()
            .zip(to)
            .map(|(a, b)| Mesh {
                sprite: b.sprite.clone(),
                overlay: b.overlay,
                vertices: a
                    .vertices
                    .iter()
                    .zip(&b.vertices)
                    .map(|(va, vb)| {
                        let mut v = *vb;
                        for i in 0..3 {
                            v[i] = va[i] + (vb[i] - va[i]) * t;
                        }
                        v
                    })
                    .collect(),
            })
            .collect(),
    )
}

/// `meshes` drawn with their origin at `at` (relative to the camera), lit by `light` (Horizon's
/// sky << 4 | block), with the GUI atlas' sprites.
pub(super) fn push_meshes(
    atlas: &horizon_client::hud::HudAtlas,
    meshes: &[Mesh],
    at: glam::Vec3,
    light: u8,
    out: &mut Vec<Vertex>,
) {
    // The lightmap's (block, sky): Horizon packs them sky << 4 | block.
    let packed = (u32::from(light & 15) * 16) | ((u32::from(light >> 4) * 16) << 8);
    let base = glam::Mat4::from_translation(at);
    for m in meshes {
        let Some(r) = atlas.sprites.get(&m.sprite) else {
            continue;
        };
        let overlay = overlay_code(m.overlay);
        for v in &m.vertices {
            let n = [v[5], v[6], v[7]].map(|c| (c.clamp(-1.0, 1.0) * 127.0) as i8);
            out.push(Vertex {
                pos: base
                    .transform_point3(glam::Vec3::new(v[0], v[1], v[2]))
                    .to_array(),
                uv: [r.u0 + (r.u1 - r.u0) * v[3], r.v0 + (r.v1 - r.v0) * v[4]],
                layer: overlay,
                color: abgr(v[8] as u32),
                light: packed,
                normal: [n[0], n[1], n[2], -127],
                glint_uv: [0.0, 0.0],
            });
        }
    }
}

impl Game<'_> {
    /// The server half, each tick: the mods' mobs tick, and each player of this dimension is
    /// sent how they look.
    pub(in crate::game) fn tick_java120_mobs(&mut self) {
        if self.remote.is_some() {
            return;
        }
        // Each runtime ticks its own mobs; the players' games get them all in one message.
        // Each entry is its JSON already (the meshes as the runtime wrote them, never parsed
        // here: megabytes a tick).
        let mut entries: Vec<String> = Vec::new();
        let mut any = false;
        for loader in super::loaders() {
            let _route = super::route(loader);
            let mut last = LAST_ENTRIES[loader.index()]
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match self.tick_java120_mobs_of() {
                Some((theirs, had)) => {
                    any |= had;
                    *last = theirs;
                }
                // Skipped (busy, idle): its mobs as last sent, so they don't vanish for a tick
                // from a message the other runtime's mobs make.
                None => any |= !last.is_empty(),
            }
            entries.extend(last.iter().cloned());
        }
        if !any {
            return;
        }
        let data = format!("{{\"e\":[{}]}}", entries.join(",")).into_bytes();
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let here = host.hub.config.dimension.clone();
        let conns: Vec<u32> = host
            .hub
            .guests
            .iter()
            .filter(|g| g.placed && g.dimension == here)
            .map(|g| g.conn)
            .collect();
        for conn in conns {
            host.hub.send_to(
                conn,
                &horizon_protocol::packets::play::Clientbound::CustomPayload(CustomPayload {
                    channel: CHANNEL.into(),
                    data: data.clone(),
                }),
            );
        }
    }

    /// The calls' runtime's mobs ticked: their entries for the players' games, and whether
    /// there are or were any (else nothing need be sent). None when the tick was skipped.
    fn tick_java120_mobs_of(&mut self) -> Option<(Vec<String>, bool)> {
        let dimension = self.dim.id().to_owned();
        let answer = {
            // A chunk worker placing a feature holds the runtime for a moment: wait for it
            // rather than freeze the mobs for a tick.
            // The chunk workers step aside meanwhile (they'd take it again between placements,
            // and the mobs skipped tick after tick while the world generated).
            let mut guard = super::runtime_ahead_of_worldgen(std::time::Duration::from_millis(25))?;
            let runtime = guard.as_mut()?;
            let level = Level(self);
            self.java120_bring_mobs(runtime, &level, &dimension);
            let idle = *last_count().lock().unwrap_or_else(|e| e.into_inner()) == 0;
            if idle && !self.ticks.is_multiple_of(IDLE_EVERY) {
                return None;
            }
            runtime
                .tick_entities(&level, &dimension, &self.java120_players_here())
                .and_then(|ticked| {
                    let looks = if ticked.entities.is_empty() {
                        String::from("{}")
                    } else {
                        runtime.render_entities(&level, &dimension)?
                    };
                    Ok((ticked, looks))
                })
        };
        let (ticked, looks) = match answer {
            Ok(answer) => answer,
            Err(e) => {
                tick_failed_once(&e);
                return None;
            }
        };
        let was = std::mem::replace(
            &mut *last_count().lock().unwrap_or_else(|e| e.into_inner()),
            ticked.entities.len(),
        );
        self.apply_java120(
            &dimension,
            &ticked.set,
            &ticked.block_entities,
            &ticked.spawned,
        );
        SEEN.lock()
            .unwrap_or_else(|e| e.into_inner())
            .get_or_insert_with(HashMap::new)
            .insert(
                (super::current(), dimension.clone()),
                ticked
                    .entities
                    .iter()
                    .map(|e| Seen {
                        id: e.id,
                        kind: e.kind.clone(),
                        category: e.category.clone(),
                        pos: e.pos,
                        is_mob: e.is_mob,
                        persistent: e.persistent,
                    })
                    .collect(),
            );
        if ticked.entities.is_empty() && was == 0 {
            return Some((Vec::new(), false));
        }
        // Each mob's meshes kept as the runtime wrote them: only scanned for where each ends.
        let looks: HashMap<String, Box<serde_json::value::RawValue>> =
            serde_json::from_str(&looks).unwrap_or_default();
        // A far mob is drawn every few ticks (Render120): between, its last look.
        let mut guard = LOOKS[super::current().index()]
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let kept = guard.get_or_insert_with(HashMap::new);
        for (id, look) in looks {
            if let Ok(id) = id.parse::<i32>() {
                kept.insert(id, look);
            }
        }
        let alive: HashSet<i32> = ticked.entities.iter().map(|e| e.id).collect();
        kept.retain(|id, _| alive.contains(id));
        let entries: Vec<String> = ticked
            .entities
            .iter()
            .map(|e| {
                let head = serde_json::json!({
                    "id": e.id, "kind": e.kind, "p": e.pos, "r": e.rot, "s": e.size, "h": e.health,
                })
                .to_string();
                let meshes = kept.get(&e.id).map_or("null", |m| m.get());
                // `{...}` and the meshes: the head's closing brace makes way for "m".
                format!("{},\"m\":{meshes}}}", &head[..head.len() - 1])
            })
            .collect();
        drop(guard);
        Some((entries, true))
    }

    /// The server half, before a dimension's first tick: the mobs its save kept go back into
    /// the runtime (and those of another world, opened before in this game, go).
    fn java120_bring_mobs(
        &self,
        runtime: &mut horizon_jvm_remote::boot120::Runtime120,
        level: &Level<'_, '_>,
        dimension: &str,
    ) {
        let mut held = held().lock().unwrap_or_else(|e| e.into_inner());
        if held
            .as_ref()
            .is_some_and(|(root, _)| root != &self.world_root)
        {
            if let Err(e) = runtime.clear_entities(level) {
                log::warn!("1.20.1 mods: the last world's mobs didn't go: {e}");
            }
            *last_count().lock().unwrap_or_else(|e| e.into_inner()) = 0;
            *held = None;
        }
        let (_, loaded) = held.get_or_insert_with(|| (self.world_root.clone(), HashSet::new()));
        if !loaded.insert(dimension.to_owned()) {
            return;
        }
        let path = kept_at(&self.world_root, dimension);
        let Ok(text) = std::fs::read_to_string(&path) else {
            return;
        };
        let saved: Vec<String> = match serde_json::from_str(&text) {
            Ok(saved) => saved,
            Err(e) => {
                log::warn!("1.20.1 mods: {} unread: {e}", path.display());
                return;
            }
        };
        match runtime.load_entities(level, dimension, &saved) {
            Ok(n) => log::info!("1.20.1 mods: {n} of their mobs back in {dimension}"),
            Err(e) => log::warn!("1.20.1 mods: their mobs in {dimension} didn't load: {e}"),
        }
    }

    /// The server half, as a dimension saves (`save_level`): its mobs as the mods' code saves
    /// them, beside its other data. Not before they were brought in from there.
    pub(in crate::game) fn save_java120_mobs(&mut self) {
        if self.remote.is_some() {
            return;
        }
        for loader in super::loaders() {
            let _route = super::route(loader);
            self.save_java120_mobs_of();
        }
    }

    /// [`Self::save_java120_mobs`] for the calls' runtime.
    fn save_java120_mobs_of(&mut self) {
        let dimension = self.dim.id().to_owned();
        let in_already = held()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|(root, loaded)| root == &self.world_root && loaded.contains(&dimension));
        if !in_already {
            return;
        }
        let saved = {
            let mut guard = runtime().lock().unwrap_or_else(|e| e.into_inner());
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            runtime.save_entities(&Level(self), &dimension)
        };
        let saved = match saved {
            Ok(saved) => saved,
            Err(e) => {
                log::warn!("1.20.1 mods: their mobs in {dimension} weren't saved: {e}");
                return;
            }
        };
        let path = kept_at(&self.world_root, &dimension);
        let written = (|| -> std::io::Result<()> {
            if saved.is_empty() && !path.exists() {
                return Ok(());
            }
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir)?;
            }
            // Whole or not at all: a save cut short leaves the last one.
            let part = path.with_extension("json.part");
            std::fs::write(&part, serde_json::to_vec(&saved).unwrap_or_default())?;
            std::fs::rename(&part, &path)
        })();
        if let Err(e) = written {
            log::warn!("1.20.1 mods: {} not written: {e}", path.display());
        }
    }

    /// The mods' entity types into the commands' registries (`/summon`'s argument), once their
    /// runtime has said them: it may start after the world's registries were made.
    pub(in crate::game) fn java120_command_types(&mut self) {
        let types = TYPES.lock().unwrap_or_else(|e| e.into_inner());
        let reg = &mut self.ui.registries;
        let Some(first) = types.first() else {
            return;
        };
        if reg.ids("minecraft:entity_type").contains(first) {
            return;
        }
        log::info!(
            "1.20.1 mods: {} entity type(s) of theirs for the commands ({})",
            types.len(),
            if self.remote.is_some() {
                "window"
            } else {
                "server"
            }
        );
        // The registry's ids are read from its export's list when it has one, else the field.
        let listed = reg
            .by_registry
            .get_mut("minecraft:entity_type")
            .map(|r| &mut r.ids);
        let mut lists = vec![&mut reg.entity_types, &mut reg.summonable];
        lists.extend(listed);
        for kind in types.iter() {
            for list in lists.iter_mut() {
                if let Err(at) = list.binary_search(kind) {
                    list.insert(at, kind.clone());
                }
            }
        }
    }

    /// The server half: the players in this level, for the mods' mobs to see (despawning far
    /// from them, following them).
    fn java120_players_here(&self) -> Vec<horizon_jvm_remote::mirror120::PlayerHere> {
        self.lan
            .iter()
            .flat_map(|h| {
                h.hub
                    .guests
                    .iter()
                    .filter(|g| h.hub.here(g) && !g.spectator() && !g.dead)
                    .map(|g| {
                        let uuid = super::uuid_string(g.uuid);
                        super::items::remember_player(
                            g.entity_id,
                            uuid.clone(),
                            g.name.clone(),
                            self.dim.id().to_owned(),
                        );
                        horizon_jvm_remote::mirror120::PlayerHere {
                            uuid,
                            name: g.name.clone(),
                            pos: g.position,
                        }
                    })
            })
            .collect()
    }

    /// The server half of a natural spawn of one of the mods' types, where Horizon's spawner
    /// picked it (`NaturalLevel::foreign_spawn`).
    pub(in crate::game) fn java120_spawn_natural(
        &self,
        kind: &str,
        pos: [f64; 3],
        yaw: f32,
        distance: f64,
    ) -> Option<(Aabb, u32)> {
        let dimension = self.dim.id().to_owned();
        let _route = super::route_id(kind);
        let made = {
            let Ok(mut guard) = runtime().try_lock() else {
                return None;
            };
            let runtime = guard.as_mut()?;
            runtime.spawn_natural(&Level(self), &dimension, kind, pos, yaw, distance)
        };
        let made = match made {
            Ok(made) => made?,
            Err(e) => {
                tick_failed_once(&e);
                return None;
            }
        };
        *last_count().lock().unwrap_or_else(|e| e.into_inner()) += 1;
        let half = f64::from(made.size[0]) / 2.0;
        Some((
            Aabb::new(
                Vec3::new(pos[0] - half, pos[1], pos[2] - half),
                Vec3::new(
                    pos[0] + half,
                    pos[1] + f64::from(made.size[1]),
                    pos[2] + half,
                ),
            ),
            made.max_cluster,
        ))
    }

    /// The server half of a chunk-generation spawn of one of the mods' types
    /// (`NaturalLevel::foreign_generation_spawn`).
    pub(in crate::game) fn java120_spawn_generated(
        &self,
        kind: &str,
        column: [i32; 2],
        chunk: (i32, i32),
        yaw: f32,
    ) -> bool {
        let dimension = self.dim.id().to_owned();
        if super::tracing() {
            log::info!("1.20.1 mods (trace): a chunk-generation spawn of {kind} at {column:?}");
        }
        let _route = super::route_id(kind);
        let made = {
            let Ok(mut guard) = runtime().try_lock() else {
                if super::tracing() {
                    log::info!("1.20.1 mods (trace): the runtime was busy");
                }
                return false;
            };
            let Some(runtime) = guard.as_mut() else {
                return false;
            };
            runtime.spawn_generated(&Level(self), &dimension, kind, column, chunk, yaw)
        };
        match made {
            Ok(made) => {
                if made {
                    *last_count().lock().unwrap_or_else(|e| e.into_inner()) += 1;
                }
                made
            }
            Err(e) => {
                tick_failed_once(&e);
                false
            }
        }
    }

    /// The server half of `/summon` for one of the mods' types: made in the runtime, from
    /// `nbt` when given. None when `kind` isn't theirs (Horizon's summon goes on), else whether
    /// it was made.
    pub(in crate::game) fn java120_summon(
        &mut self,
        kind: &str,
        pos: [f64; 3],
        nbt: Option<&str>,
    ) -> Option<bool> {
        if !theirs(kind) {
            return None;
        }
        let dimension = self.dim.id().to_owned();
        let _route = super::route_id(kind);
        let made = {
            let mut guard = wait_for_runtime()?;
            let runtime = guard.as_mut()?;
            runtime.summon(&Level(self), &dimension, kind, pos, nbt)
        };
        match made {
            Ok(made) => {
                // The next tick asks for them, rather than wait out the idle pause.
                *last_count().lock().unwrap_or_else(|e| e.into_inner()) += 1;
                Some(made.is_some())
            }
            Err(e) => {
                log::warn!("1.20.1 mods: /summon {kind} failed: {e}");
                Some(false)
            }
        }
    }

    /// The window: the mobs a tick of the server's has, in place of the last. Read on a
    /// thread of its own ([`read_mobs`]): a tick's meshes are megabytes of JSON, and reading
    /// them here held a frame every tick (350 frames a second that felt like 50).
    pub(in crate::game) fn remote_java120_mobs(&mut self, data: &[u8]) {
        let (slot, ready) = &MOBS_IN;
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(data.to_vec());
        ready.notify_one();
        static READER: std::sync::Once = std::sync::Once::new();
        READER.call_once(|| {
            let spawned = std::thread::Builder::new()
                .name("1.20.1 mobs reader".into())
                .spawn(read_mobs);
            if let Err(e) = spawned {
                log::warn!("1.20.1 mods: the mobs' reader didn't start: {e}");
            }
        });
    }

    /// The window's entity pass: each mob's quads, where it is between the last two ticks,
    /// lit by the light where it stands.
    pub(in crate::game) fn java120_mob_geometry(
        &self,
        camera: glam::DVec3,
        alpha: f64,
        out: &mut Vec<Vertex>,
    ) {
        let window = WINDOW.lock().unwrap_or_else(|e| e.into_inner());
        let Some(window) = window.as_ref() else {
            return;
        };
        let atlas = &self.gui.atlas;
        for (id, mob) in &window.now {
            let from = window.before.get(id).map_or(mob.pos, |b| b.pos);
            let at: [f64; 3] = std::array::from_fn(|i| from[i] + (mob.pos[i] - from[i]) * alpha);
            let d = glam::DVec3::from_array(at) - camera;
            // Entity.shouldRenderAtSqrDistance: 64 times the box's mean edge.
            let range = f64::from(mob.size[0] * 2.0 + mob.size[1]) / 3.0 * 64.0;
            if d.length_squared() >= range * range {
                continue;
            }
            let light = self.world.light(
                at[0].floor() as i32,
                (at[1] + f64::from(mob.size[1]) * 0.85).floor() as i32,
                at[2].floor() as i32,
            );
            // Its pose between the last two ticks too, as vanilla renders with the partial
            // tick: the same quads, moved part of the way.
            let meshes = window
                .before
                .get(id)
                .and_then(|b| lerp_meshes(&b.meshes, &mob.meshes, alpha as f32));
            push_meshes(
                atlas,
                meshes.as_deref().unwrap_or(&mob.meshes),
                d.as_vec3(),
                light,
                out,
            );
        }
    }

    /// The window: its mobs for the agent's entity list (id, type, position, health).
    pub(in crate::game) fn java120_mob_list(&self) -> Vec<(i32, String, [f64; 3], Option<f64>)> {
        let window = WINDOW.lock().unwrap_or_else(|e| e.into_inner());
        window.as_ref().map_or_else(Vec::new, |w| {
            w.now
                .iter()
                .map(|(&id, m)| (id, m.kind.clone(), m.pos, m.health))
                .collect()
        })
    }

    /// The window: the mods' mob the crosshair is on, nearest first, within reach, and how far
    /// along the look it is (as `ProjectileUtil.getEntityHitResult` picks).
    pub(in crate::game) fn java120_mob_hit(&self) -> Option<(i32, f64)> {
        let eye = self.eye_position()?;
        let dir = super::super::view_vector(self.yaw, self.pitch);
        let reach: f64 = if self.creative { 5.0 } else { 3.0 };
        let window = WINDOW.lock().unwrap_or_else(|e| e.into_inner());
        window
            .as_ref()?
            .now
            .iter()
            .filter_map(|(&id, mob)| {
                let half = f64::from(mob.size[0]) / 2.0;
                let [x, y, z] = mob.pos;
                let bounds = Aabb::new(
                    Vec3::new(x - half, y, z - half),
                    Vec3::new(x + half, y + f64::from(mob.size[1]), z + half),
                );
                Some((id, super::super::ray_box(eye, dir, reach, bounds)?))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// The window: where on mob `id` the crosshair is, relative to it (the use's location).
    pub(in crate::game) fn java120_mob_location(&self, id: i32, distance: f64) -> [f64; 3] {
        let window = WINDOW.lock().unwrap_or_else(|e| e.into_inner());
        let (Some(eye), Some(mob)) = (
            self.eye_position(),
            window.as_ref().and_then(|w| w.now.get(&id)),
        ) else {
            return [0.0; 3];
        };
        let dir = super::super::view_vector(self.yaw, self.pitch);
        std::array::from_fn(|i| eye[i] + dir[i] * distance - mob.pos[i])
    }

    /// The server half: a connection's player attacks (`used` None) or uses one of the mods'
    /// entities. False when `id` isn't theirs, for Horizon to go on; else it was handled here,
    /// as 1.20.1's handleInteract does: nothing for a spectator or from over six blocks away,
    /// else the mods' code runs it (`Runtime120::entity_action`).
    pub(in crate::game) fn java120_entity_action(
        &mut self,
        host: &mut LanHost,
        conn: u32,
        id: i32,
        used: Option<&Interact>,
    ) -> bool {
        if !IDS.contains(&id) {
            return false;
        }
        let Some(guest) = host.hub.guest(conn) else {
            return true;
        };
        if guest.spectator() {
            return true;
        }
        let feet = guest.position;
        let near = SEEN
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .and_then(|all| all.values().flatten().find(|s| s.id == id).map(|s| s.pos))
            .is_some_and(|at| {
                (0..3)
                    .map(|i| (at[i] - feet[i]) * (at[i] - feet[i]))
                    .sum::<f64>()
                    < REACH_SQUARED
            });
        if !near {
            return true;
        }
        let swung = self
            .as_guest(host, conn, |g| g.java120_act_on(id, used))
            .flatten();
        if used.is_some() {
            self.lan_swing_back(host, conn, swung);
        }
        true
    }

    /// The player acting now hits or uses mod entity `id` with what's in that hand; whether
    /// the arm swings (None when it couldn't be asked).
    fn java120_act_on(&mut self, id: i32, used: Option<&Interact>) -> Option<bool> {
        let off_hand = used.is_some_and(|p| p.hand == 1);
        let held = if off_hand {
            self.offhand.as_ref()
        } else {
            self.inventory.get(self.selected).and_then(Option::as_ref)
        };
        let action = EntityAction {
            uuid: self.java120_actor(),
            name: self.acting_player_name().to_owned(),
            dimension: self.dim.id().to_owned(),
            id,
            attack: used.is_none(),
            off_hand,
            held: held.map(stack_to_120).unwrap_or_default(),
            creative: self.mode == horizon_client::command::args::GameMode::Creative,
            sneaking: used.is_some_and(|p| p.secondary),
            ticks: i32::try_from(self.attack_ticks).unwrap_or(i32::MAX),
            location: used.map_or([0.0; 3], |p| p.location),
            stance: self.java120_stance(),
        };
        let _route = super::route(super::loader_of_entity(id));
        let done = {
            let mut guard = wait_for_runtime()?;
            let runtime = guard.as_mut()?;
            runtime.entity_action(&Level(self), &action)
        };
        let done = match done {
            Ok(done) => done,
            Err(e) => {
                log::warn!("1.20.1 mods: acting on entity {id} failed: {e}");
                return None;
            }
        };
        if action.attack {
            self.attack_ticks = 0;
        }
        self.apply_java120(
            &action.dimension,
            &done.set,
            &done.block_entities,
            &done.spawned,
        );
        if let Some(opened) = &done.opened {
            self.java120_opened(&action.uuid, &opened.kind);
        }
        self.java120_held_after(off_hand, &action.held, &done.held);
        for given in &done.given {
            if let Some(stack) = super::stack_from_120(given) {
                self.give_or_drop(stack);
            }
        }
        Some(done.result == 2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_ticks_mobs_read_as_the_server_writes_them() {
        let data = br#"{"e":[{"id":7,"kind":"alexsmobs:seal","p":[1.5,64,2],"r":[0,0,0,0],"s":[0.9,0.6],"h":10.0,
            "m":[{"tex":"alexsmobs:textures/entity/seal.png","o":655360,"v":[0,1,2,0.5,0.25,0,1,0,4294967295]}]},
            {"id":8,"kind":"naturalist:bird","p":[0,0,0],"r":[0,0,0,0],"s":[0.4,0.4],"h":null,"m":null}]}"#;
        let read: MobsIn = serde_json::from_slice(data).unwrap();
        assert_eq!(read.e.len(), 2);
        let seal = &read.e[0];
        assert_eq!(seal.id, 7);
        assert_eq!(seal.p, [1.5, 64.0, 2.0]);
        let meshes = seal.m.as_ref().unwrap();
        assert_eq!(meshes[0].v.len(), 9);
        assert_eq!(meshes[0].o, 655360);
        assert!(read.e[1].m.is_none() && read.e[1].h.is_none());
    }

    fn mesh(sprite: &str, x: f32) -> Mesh {
        Mesh {
            sprite: sprite.into(),
            overlay: 0,
            vertices: vec![[x, 1.0, 0.0, 0.5, 0.5, 0.0, 1.0, 0.0, -1.0]],
        }
    }

    #[test]
    fn a_pose_between_two_ticks_moves_the_quads_part_of_the_way() {
        let now = lerp_meshes(&[mesh("a", 0.0)], &[mesh("a", 2.0)], 0.25).expect("same quads");
        assert_eq!(now[0].vertices[0][0], 0.5, "a quarter of the way");
        assert_eq!(
            now[0].vertices[0][3], 0.5,
            "texture coordinates are the tick's own"
        );
        // A part shown or hidden since: drawn as the tick has it.
        assert!(lerp_meshes(&[mesh("a", 0.0)], &[mesh("b", 2.0)], 0.5).is_none());
        assert!(lerp_meshes(&[], &[mesh("a", 2.0)], 0.5).is_none());
    }
}
