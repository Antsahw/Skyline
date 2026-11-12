//! The 1.20.1 mods' own mobs (phase 6d). They live in the runtime (`Entities120`): the server
//! half ticks them each tick (their AI, paths and physics on Horizon's blocks), has their own
//! renderers draw them (`Render120`: quads with their textures, relative to each mob) and sends
//! each player those in its dimension on [`CHANNEL`]. The window draws the quads where the mob
//! is, moving between the last two ticks as vanilla's renderer does.

use super::super::Game;
use super::{Level, RUNTIME, tick_failed_once};
use horizon_client::mesher::Vertex;
use horizon_protocol::packets::configuration::CustomPayload;
use serde_json::Value;
use std::collections::HashMap;
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

/// One mob as the window has it.
#[derive(Clone)]
struct Mob {
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
        let idle = *LAST_COUNT.lock().unwrap_or_else(|e| e.into_inner()) == 0;
        if idle && !self.ticks.is_multiple_of(IDLE_EVERY) {
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
            runtime
                .tick_entities(&level, &dimension)
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
            let packed = (u32::from(light >> 4) * 16) | ((u32::from(light & 15) * 16) << 8);
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
}
