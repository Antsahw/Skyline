//! The 1.20.1 mods' blocks whose look is their code (phase 7): a model a mod makes per
//! position (LibMultipart's container, whose parts are in its block entity: Wired Redstone's
//! cables). The runtime bakes the client's models and says which states those are; the server
//! half renders each such block again when its state or block entity changed (the runtime's
//! `renderBlock`: the mod's own model through Indigo), and the window's mesher draws the quads
//! at that position (`mesher::set_code_model`). Only where this process runs the mods (single
//! player, a LAN host's own window), as the 26.3 bridge's mods' fluids.

use super::{Level, RUNTIME, wait_for_runtime};
use horizon_jvm_remote::mirror120::{CodeQuad, World120};
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;

/// How often the server half looks for blocks to draw again: every half second.
const EVERY: u64 = 10;

/// The server half's view: the states drawn by code (by Horizon's ids; `None` before the
/// runtime said), and what each position was last drawn from.
#[derive(Default)]
struct Server {
    states: Option<HashSet<u32>>,
    drawn: HashMap<[i32; 3], u64>,
}

static SERVER: Mutex<Option<Server>> = Mutex::new(None);

/// Drawn by the server half, for the window: a position's quads, or `None` (gone).
type Drawn = ([i32; 3], Option<Vec<CodeQuad>>);
static PENDING: Mutex<Vec<Drawn>> = Mutex::new(Vec::new());

impl super::super::Game<'_> {
    /// The server half: the blocks drawn by code whose state or block entity changed since
    /// they were drawn, drawn again; those gone, said gone.
    pub(in crate::game) fn tick_java120_drawn(&mut self) {
        if self.remote.is_some() || !self.ticks.is_multiple_of(EVERY) {
            return;
        }
        let mut guard = SERVER.lock().unwrap_or_else(|e| e.into_inner());
        let server = guard.get_or_insert_with(Server::default);
        if server.states.is_none() {
            let Ok(mut runtime) = RUNTIME.try_lock() else {
                return;
            };
            let Some(runtime) = runtime.as_mut() else {
                return;
            };
            let registry = self.pack.assets.registry;
            let names = match runtime.drawn_by_code() {
                Ok(names) => names,
                Err(e) => {
                    log::warn!("1.20.1 mods: the blocks their code draws can't be listed: {e}");
                    Vec::new()
                }
            };
            let ids: HashSet<u32> = names
                .iter()
                .filter_map(|n| {
                    super::super::render_control::parse_state(
                        registry,
                        &super::worldgen::state_from_120(n),
                    )
                    .ok()
                    .map(|s| s.0)
                })
                .collect();
            log::info!(
                "1.20.1 mods: {} block state(s) drawn by their code",
                ids.len()
            );
            let list: Vec<u32> = ids.iter().copied().collect();
            horizon_client::mesher::set_drawn_by_code(&list);
            server.states = Some(ids);
        }
        let states = server.states.as_ref().expect("set above");
        if states.is_empty() {
            return;
        }
        let dimension = self.dim.id().to_owned();
        let here: Vec<[i32; 3]> = self
            .world
            .block_entities
            .mods_near(0.0, 0.0, f64::INFINITY)
            .map(|&(x, y, z)| [x, y, z])
            .filter(|&[x, y, z]| states.contains(&self.world.block(x, y, z).0))
            .collect();
        let level = Level(self);
        let mut changed = Vec::new();
        for pos in &here {
            let [x, y, z] = *pos;
            let key = {
                use std::hash::{Hash, Hasher};
                let mut h = std::collections::hash_map::DefaultHasher::new();
                self.world.block(x, y, z).0.hash(&mut h);
                level.block_entity(&dimension, x, y, z).hash(&mut h);
                h.finish()
            };
            if server.drawn.get(pos) != Some(&key) {
                changed.push((*pos, key));
            }
        }
        let gone: Vec<[i32; 3]> = server
            .drawn
            .keys()
            .filter(|p| !here.contains(p))
            .copied()
            .collect();
        let mut out = Vec::new();
        for pos in gone {
            server.drawn.remove(&pos);
            out.push((pos, None));
        }
        if !changed.is_empty() {
            let Some(mut runtime) = wait_for_runtime() else {
                return;
            };
            let Some(runtime) = runtime.as_mut() else {
                return;
            };
            for (pos, key) in changed {
                match runtime.render_block(&level, &dimension, pos) {
                    Ok(quads) => {
                        server.drawn.insert(pos, key);
                        out.push((pos, Some(quads)));
                    }
                    Err(e) => {
                        server.drawn.insert(pos, key);
                        log::warn!("1.20.1 mods: the block at {pos:?} didn't draw: {e}");
                    }
                }
            }
        }
        if !out.is_empty() {
            let quads: usize = out
                .iter()
                .filter_map(|(_, q)| q.as_ref().map(Vec::len))
                .sum();
            log::debug!(
                "1.20.1 mods: {} block(s) drawn by their code again ({quads} quads)",
                out.len()
            );
            if quads == 0 && out.iter().any(|(_, q)| q.is_some()) {
                log::info!("1.20.1 mods: a block drawn by its code drew nothing: {out:?}");
            }
            PENDING
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .extend(out);
        }
    }

    /// The window: the blocks drawn by code since, into the mesher, their sections meshed again.
    pub(in crate::game) fn java120_drawn_models(&mut self) {
        let drawn = std::mem::take(&mut *PENDING.lock().unwrap_or_else(|e| e.into_inner()));
        if drawn.is_empty() {
            return;
        }
        let min_y = self.world.layout().min_y();
        for (pos, quads) in drawn {
            let model = quads.map(|q| self.baked_from_code(&q));
            horizon_client::mesher::set_code_model(pos, model);
            let [x, y, z] = pos;
            self.world
                .dirty_sections
                .insert((x >> 4, (y - min_y) >> 4, z >> 4));
        }
    }

    /// The quads a mod's code drew, as the mesher's model: each quad on the sprite it names
    /// (the missing texture when the block atlas lacks it), its colour when not white.
    fn baked_from_code(&self, quads: &[CodeQuad]) -> horizon_client::models::catalog::BakedModel {
        use horizon_client::models::catalog::{BakedFace, BakedModel, BakedQuad};
        let layers = &self.pack.render.sprite_layers;
        let missing = layers.get("minecraft:missingno").copied().unwrap_or(0);
        let faces = quads
            .iter()
            .map(|q| {
                let positions = q.corners.map(|c| [c[0], c[1], c[2]]);
                let layer = layers.get(&q.sprite).copied().unwrap_or(missing);
                let color = q.colors[0];
                BakedFace {
                    quad: BakedQuad {
                        positions,
                        uv: q.corners.map(|c| [c[3], c[4]]),
                        facing: facing(&positions),
                        cull: None,
                        tint_index: -1,
                        shade_direction_override: None,
                        light_emission: 0,
                        light: [0, 0],
                        color: (color != 0xffff_ffff).then_some(color as i32),
                        ambient_occlusion: None,
                        render_type: None,
                        shade: true,
                    },
                    layer,
                    opaque: false,
                    translucent: self
                        .pack
                        .render
                        .translucent_layers
                        .get(layer as usize)
                        .copied()
                        .unwrap_or(false),
                    force_translucent: false,
                    fusion: None,
                }
            })
            .collect();
        BakedModel {
            ambient_occlusion: true,
            particle: None,
            faces,
            cube: None,
        }
    }
}

/// The side a quad faces: its normal's largest axis.
fn facing(p: &[[f32; 3]; 4]) -> horizon_assets::model::Direction {
    use horizon_assets::model::Direction as D;
    let a = [p[1][0] - p[0][0], p[1][1] - p[0][1], p[1][2] - p[0][2]];
    let b = [p[2][0] - p[0][0], p[2][1] - p[0][1], p[2][2] - p[0][2]];
    let n = [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ];
    let (ax, ay, az) = (n[0].abs(), n[1].abs(), n[2].abs());
    if ay >= ax && ay >= az {
        if n[1] >= 0.0 { D::Up } else { D::Down }
    } else if ax >= az {
        if n[0] >= 0.0 { D::East } else { D::West }
    } else if n[2] >= 0.0 {
        D::South
    } else {
        D::North
    }
}

#[cfg(test)]
mod tests {
    use horizon_assets::model::Direction as D;

    /// A quad's corners in vanilla's order (counter-clockwise seen from outside) face out.
    #[test]
    fn a_quad_faces_where_its_corners_turn() {
        let up = [
            [0.0, 0.1, 0.0],
            [0.0, 0.1, 1.0],
            [1.0, 0.1, 1.0],
            [1.0, 0.1, 0.0],
        ];
        assert_eq!(super::facing(&up), D::Up);
        let down = [
            [0.0, 0.0, 1.0],
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 0.0, 1.0],
        ];
        assert_eq!(super::facing(&down), D::Down);
        let north = [
            [1.0, 1.0, 0.0],
            [1.0, 0.0, 0.0],
            [0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        assert_eq!(super::facing(&north), D::North);
    }
}
