//! The 1.20.1 mods' own mobs (phase 6d). They live in the runtime (`Entities120`): the server
//! half ticks them each tick (their AI, paths and physics on Horizon's blocks), has their own
//! renderers draw them (`Render120`: quads with their textures, relative to each mob) and sends
//! each player those in its dimension on [`CHANNEL`]. The window draws the quads where the mob
//! is, moving between the last two ticks as vanilla's renderer does.

use super::super::{Game, lan::LanHost};
use super::breaking::stack_to_120;
use super::{Level, RUNTIME, tick_failed_once, wait_for_runtime};
use horizon_client::mesher::Vertex;
use horizon_core::geometry::{Aabb, Vec3};
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

/// The server half: how many mobs the last tick had.
static LAST_COUNT: Mutex<usize> = Mutex::new(0);

/// The mods' entities' ids (`Entities120.FIRST_ID` and up): Horizon's own are below, the 26.3
/// bridge's (`java_entities::FIRST_ID`) above.
pub(in crate::game) const IDS: std::ops::Range<i32> = 0x7ffe_0000..0x7fff_0000;

/// The server half: each dimension's mobs after its last tick (a click's range check, the
/// mob caps' count).
static SEEN: Mutex<Option<HashMap<String, Vec<Seen>>>> = Mutex::new(None);

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
    SEEN.lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|all| all.get(dimension).cloned())
        .unwrap_or_default()
}

/// Whether a type is one of the mods' (their runtime makes it).
pub(in crate::game) fn theirs(kind: &str) -> bool {
    TYPES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|t| t == kind)
}

/// The mods' entity types, from when their runtime started: what `/summon` makes there.
static TYPES: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// The runtime is up: its mods' entity types, for Horizon's commands.
pub(super) fn know_types(runtime: &mut horizon_jvm_remote::boot120::Runtime120) {
    match runtime.mod_entity_types() {
        Ok(types) => *TYPES.lock().unwrap_or_else(|e| e.into_inner()) = types,
        Err(e) => log::warn!("1.20.1 mods: their entity types can't be listed: {e}"),
    }
}

/// The server half: which world's mobs the runtime holds (its folder), and the dimensions of it
/// whose saved mobs are in already. A dimension's save is written only once it is in, so a
/// save before that never writes over the mobs it would have loaded.
static HELD: Mutex<Option<(PathBuf, HashSet<String>)>> = Mutex::new(None);

/// Where a dimension's mobs are kept, beside its other data (`raids.dat`).
fn kept_at(root: &Path, dimension: &str) -> PathBuf {
    horizon_client::save::dimension_dir(root, dimension).join("data/skyline/entities120.json")
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
struct Mesh {
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

impl Game<'_> {
    /// The server half, each tick: the mods' mobs tick, and each player of this dimension is
    /// sent how they look.
    pub(in crate::game) fn tick_java120_mobs(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let dimension = self.dim.id().to_owned();
        let answer = {
            let Ok(mut guard) = RUNTIME.try_lock() else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            let level = Level(self);
            self.java120_bring_mobs(runtime, &level, &dimension);
            let idle = *LAST_COUNT.lock().unwrap_or_else(|e| e.into_inner()) == 0;
            if idle && !self.ticks.is_multiple_of(IDLE_EVERY) {
                return;
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
                return;
            }
        };
        let was = std::mem::replace(
            &mut *LAST_COUNT.lock().unwrap_or_else(|e| e.into_inner()),
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
                dimension.clone(),
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
            return;
        }
        let looks: Value = serde_json::from_str(&looks).unwrap_or(Value::Null);
        let entries: Vec<Value> = ticked
            .entities
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": e.id, "kind": e.kind, "p": e.pos, "r": e.rot, "s": e.size,
                    "h": e.health, "m": looks.get(e.id.to_string()).cloned().unwrap_or(Value::Null),
                })
            })
            .collect();
        let data = serde_json::to_vec(&serde_json::json!({ "e": entries })).unwrap_or_default();
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

    /// The server half, before a dimension's first tick: the mobs its save kept go back into
    /// the runtime (and those of another world, opened before in this game, go).
    fn java120_bring_mobs(
        &self,
        runtime: &mut horizon_jvm_remote::boot120::Runtime120,
        level: &Level<'_, '_>,
        dimension: &str,
    ) {
        let mut held = HELD.lock().unwrap_or_else(|e| e.into_inner());
        if held
            .as_ref()
            .is_some_and(|(root, _)| root != &self.world_root)
        {
            if let Err(e) = runtime.clear_entities(level) {
                log::warn!("1.20.1 mods: the last world's mobs didn't go: {e}");
            }
            *LAST_COUNT.lock().unwrap_or_else(|e| e.into_inner()) = 0;
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
        let dimension = self.dim.id().to_owned();
        let in_already = HELD
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
            .is_some_and(|(root, loaded)| root == &self.world_root && loaded.contains(&dimension));
        if !in_already {
            return;
        }
        let saved = {
            let mut guard = RUNTIME.lock().unwrap_or_else(|e| e.into_inner());
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
                    .map(|g| horizon_jvm_remote::mirror120::PlayerHere {
                        uuid: super::uuid_string(g.uuid),
                        name: g.name.clone(),
                        pos: g.position,
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
        let made = {
            let Ok(mut guard) = RUNTIME.try_lock() else {
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
        *LAST_COUNT.lock().unwrap_or_else(|e| e.into_inner()) += 1;
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
        let made = {
            let Ok(mut guard) = RUNTIME.try_lock() else {
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
                    *LAST_COUNT.lock().unwrap_or_else(|e| e.into_inner()) += 1;
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
        let made = {
            let mut guard = wait_for_runtime()?;
            let runtime = guard.as_mut()?;
            runtime.summon(&Level(self), &dimension, kind, pos, nbt)
        };
        match made {
            Ok(made) => {
                // The next tick asks for them, rather than wait out the idle pause.
                *LAST_COUNT.lock().unwrap_or_else(|e| e.into_inner()) += 1;
                Some(made.is_some())
            }
            Err(e) => {
                log::warn!("1.20.1 mods: /summon {kind} failed: {e}");
                Some(false)
            }
        }
    }

    /// The window: the mobs a tick of the server's has, in place of the last.
    pub(in crate::game) fn remote_java120_mobs(&mut self, data: &[u8]) {
        let Ok(json) = serde_json::from_slice::<Value>(data) else {
            return;
        };
        let mut now = HashMap::new();
        for e in json["e"].as_array().into_iter().flatten() {
            let Some(id) = e["id"].as_i64() else { continue };
            let f = |v: &Value, i: usize| v.get(i).and_then(Value::as_f64).unwrap_or(0.0);
            let meshes = e["m"]
                .as_array()
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
                .collect();
            now.insert(
                id as i32,
                Mob {
                    kind: e["kind"].as_str().unwrap_or_default().to_owned(),
                    health: e["h"].as_f64(),
                    pos: [f(&e["p"], 0), f(&e["p"], 1), f(&e["p"], 2)],
                    size: [f(&e["s"], 0) as f32, f(&e["s"], 1) as f32],
                    meshes,
                },
            );
        }
        let mut window = WINDOW.lock().unwrap_or_else(|e| e.into_inner());
        let window = window.get_or_insert_with(Window::default);
        window.before = std::mem::replace(&mut window.now, now);
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
            // The lightmap's (block, sky): Horizon packs them sky << 4 | block.
            let packed = (u32::from(light & 15) * 16) | ((u32::from(light >> 4) * 16) << 8);
            let base = glam::Mat4::from_translation(d.as_vec3());
            for m in &mob.meshes {
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
