//! The 1.20.1 mods' block entity renderers (phase 7b): what a mod draws on its block in code, a
//! cutting board's item, a skillet's food. The server half asks the runtime to draw the mods'
//! block entities near its players (`Runtime120::render_block_entities`: each by its own
//! renderer, through the client's baked models) and sends each player of the dimension what
//! changed on [`CHANNEL`]; the window draws those quads on their blocks.

use super::super::Game;
use super::mobs::{Mesh, parse_meshes, push_meshes};
use super::{Level, RUNTIME, tick_failed_once};
use horizon_client::mesher::Vertex;
use horizon_protocol::packets::configuration::CustomPayload;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

/// Server to game, when what's drawn changes: `{"x,y,z": [{"tex", "o", "v"}...]}`, every block
/// entity of the dimension near its players that draws something (the others are left out).
pub(in crate::game) const CHANNEL: &str = "horizon:java120_block_renders";

/// How far from a player (horizontally, in blocks) a block entity is drawn.
const RANGE: i32 = 48;
/// At most this many are drawn: the nearest first.
const MOST: usize = 256;
/// What hasn't changed is sent again this often anyway (in ticks), for a player who joined since.
const RESEND_EVERY: u64 = 100;

/// The server half: what was last sent, per dimension, and when.
static SENT: Mutex<Option<HashMap<String, (String, u64)>>> = Mutex::new(None);

/// The window: the meshes drawn on each block.
static WINDOW: Mutex<Option<HashMap<[i32; 3], Vec<Mesh>>>> = Mutex::new(None);

impl Game<'_> {
    /// The server half, each tick: the mods' block entities near the players, drawn by their
    /// renderers, sent when that changed.
    pub(in crate::game) fn tick_java120_block_renders(&mut self) {
        if self.remote.is_some() || !self.ticks.is_multiple_of(2) {
            return;
        }
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let players: Vec<[f64; 3]> = host
            .hub
            .guests
            .iter()
            .filter(|g| host.hub.here(g))
            .map(|g| g.position)
            .collect();
        if players.is_empty() {
            return;
        }
        let mut near: Vec<(i64, [i32; 3])> = self
            .world
            .block_entities
            .iter()
            .filter(|(_, e)| matches!(e, horizon_client::containers::BlockEntity::Mod(_)))
            .filter_map(|(&(x, y, z), _)| {
                let d = players
                    .iter()
                    .map(|p| {
                        let (dx, dz) = (i64::from(x) - p[0] as i64, i64::from(z) - p[2] as i64);
                        dx * dx + dz * dz
                    })
                    .min()?;
                (d <= i64::from(RANGE * RANGE)).then_some((d, [x, y, z]))
            })
            .collect();
        near.sort_unstable();
        near.truncate(MOST);
        let positions: Vec<i32> = near.iter().flat_map(|(_, p)| *p).collect();
        let dimension = self.dim.id().to_owned();
        let drawn = if positions.is_empty() {
            String::from("{}")
        } else {
            let Ok(mut guard) = RUNTIME.try_lock() else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            match runtime.render_block_entities(&Level(self), &dimension, &positions) {
                Ok(drawn) => drawn,
                Err(e) => {
                    tick_failed_once(&e);
                    return;
                }
            }
        };
        {
            let mut sent = SENT.lock().unwrap_or_else(|e| e.into_inner());
            let sent = sent.get_or_insert_with(HashMap::new);
            let now = self.ticks;
            if let Some((last, at)) = sent.get(&dimension)
                && *last == drawn
                && now.saturating_sub(*at) < RESEND_EVERY
            {
                return;
            }
            if drawn == "{}" && !sent.contains_key(&dimension) {
                return;
            }
            sent.insert(dimension, (drawn.clone(), now));
        }
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
                    data: drawn.clone().into_bytes(),
                }),
            );
        }
    }

    /// The window: what the server's block entities look like now, in place of the last.
    pub(in crate::game) fn remote_java120_block_renders(&mut self, data: &[u8]) {
        let Ok(Value::Object(json)) = serde_json::from_slice::<Value>(data) else {
            return;
        };
        let mut now = HashMap::new();
        for (key, meshes) in &json {
            let mut parts = key.split(',').filter_map(|p| p.parse::<i32>().ok());
            let (Some(x), Some(y), Some(z)) = (parts.next(), parts.next(), parts.next()) else {
                continue;
            };
            now.insert([x, y, z], parse_meshes(meshes));
        }
        *WINDOW.lock().unwrap_or_else(|e| e.into_inner()) = Some(now);
    }

    /// The window's entity pass: each block's meshes on it, lit by the light in its block.
    pub(in crate::game) fn java120_block_render_geometry(
        &self,
        camera: glam::DVec3,
        out: &mut Vec<Vertex>,
    ) {
        let window = WINDOW.lock().unwrap_or_else(|e| e.into_inner());
        let Some(window) = window.as_ref() else {
            return;
        };
        for (pos, meshes) in window {
            // A block entity in a chunk the window no longer has is gone with it.
            if !self.world.loaded(pos[0], pos[2]) {
                continue;
            }
            let corner = glam::DVec3::new(f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2]));
            let light = self.world.light(pos[0], pos[1], pos[2]);
            push_meshes(
                &self.gui.atlas,
                meshes,
                (corner - camera).as_vec3(),
                light,
                out,
            );
        }
    }
}
