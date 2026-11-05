//! Blocks with properties of their own: their states, each on the vanilla state with its
//! shapes (else the nearest).
use super::*;

/// Vanilla states by their (outline, collision), and every one with its shapes.
#[derive(Debug, Default)]
pub(super) struct ShapeIndex {
    exact: BTreeMap<(String, String), String>,
    /// Each state's outline, collision and occlusion (JSON text), by its full name.
    by_name: std::collections::HashMap<String, [String; 3]>,
    /// Each state, its outline's bounds and its collision's.
    all: Vec<(String, Option<[f64; 6]>, Option<[f64; 6]>)>,
    /// `all` with one entry per (outline bounds, collision bounds): the first name of each
    /// (the nearest search ties by name), a few hundred instead of every state.
    distinct: Vec<(String, Option<[f64; 6]>, Option<[f64; 6]>)>,
    /// The nearest vanilla state found for each (outline bounds, collision bounds) asked:
    /// a mod's blocks share a few shapes over thousands of states.
    nearest: std::sync::Mutex<std::collections::HashMap<BoundsKey, String>>,
}

/// A pair of shapes' bounds, bit for bit, as a map key.
type BoundsKey = (Option<[u64; 6]>, Option<[u64; 6]>);

fn bounds_key(outline: Option<[f64; 6]>, collision: Option<[f64; 6]>) -> BoundsKey {
    (
        outline.map(|b| b.map(f64::to_bits)),
        collision.map(|b| b.map(f64::to_bits)),
    )
}

/// A shape's bounds (a box list: `[x0 y0 z0 x1 y1 z1]...`); `None` when empty.
fn bounds(v: &Value) -> Option<[f64; 6]> {
    let boxes = v.as_array()?;
    let mut out = [f64::MAX, f64::MAX, f64::MAX, f64::MIN, f64::MIN, f64::MIN];
    for b in boxes {
        let c: Vec<f64> = b.as_array()?.iter().filter_map(Value::as_f64).collect();
        if c.len() != 6 {
            return None;
        }
        for i in 0..3 {
            out[i] = out[i].min(c[i]);
            out[i + 3] = out[i + 3].max(c[i + 3]);
        }
    }
    (!boxes.is_empty()).then_some(out)
}

/// How far apart two shapes' bounds are (a box list each: `[x0 y0 z0 x1 y1 z1]...`);
/// an empty shape is far from any other.
fn bounds_distance(a: Option<[f64; 6]>, b: Option<[f64; 6]>) -> f64 {
    match (a, b) {
        (Some(x), Some(y)) => x.iter().zip(y).map(|(p, q)| (p - q).abs()).sum(),
        (None, None) => 0.0,
        _ => 6.0,
    }
}

impl Facts {
    /// For a block no vanilla block stands in for (its properties are its own): a template
    /// without properties or behaviour (stone), and its own states, each with the vanilla
    /// state whose collision and outline it has (else the nearest), for the declaration's
    /// `states`.
    pub fn own_states(&self, id: &str, allowed: &dyn Fn(&str) -> bool) -> Option<(Choice, Value)> {
        let block = self.blocks.get(id)?;
        let template = [
            "minecraft:stone",
            "minecraft:smooth_stone",
            "minecraft:dirt",
        ]
        .into_iter()
        .find(|t| allowed(t) && self.blocks.get(*t).is_some_and(|b| b.properties.is_empty()))?;
        let mut choice = self.choice(block, template, &self.blocks[template], allowed);
        self.shapes.get_or_init(|| {
            let mut index = ShapeIndex::default();
            for (name, v) in &self.blocks {
                if !name.starts_with("minecraft:") || !allowed(name) {
                    continue;
                }
                for (key, st) in &v.states {
                    let full = format!("{name}[{}]", state_key(key));
                    index.by_name.insert(
                        full.clone(),
                        [
                            st.outline.to_string(),
                            st.collision.to_string(),
                            st.occlusion
                                .as_ref()
                                .map(Value::to_string)
                                .unwrap_or_default(),
                        ],
                    );
                    index
                        .exact
                        .entry((st.outline.to_string(), st.collision.to_string()))
                        .or_insert_with(|| full.clone());
                    index
                        .all
                        .push((full, bounds(&st.outline), bounds(&st.collision)));
                }
            }
            let mut first: BTreeMap<BoundsKey, usize> = BTreeMap::new();
            for (i, (name, o, c)) in index.all.iter().enumerate() {
                let at = first.entry(bounds_key(*o, *c)).or_insert(i);
                if name < &index.all[*at].0 {
                    *at = i;
                }
            }
            let mut picked: Vec<usize> = first.into_values().collect();
            picked.sort_unstable();
            index.distinct = picked.into_iter().map(|i| index.all[i].clone()).collect();
            index
        });
        let ShapeIndex {
            exact,
            distinct,
            nearest,
            by_name,
            ..
        } = self.shapes.get().expect("built above");
        let default = state_key(&block.default);
        let default_light = block.default_state().map_or(0, |s| s.emission);
        let mut near = 0;
        let mut states = Vec::new();
        for (key, st) in &block.states {
            let key = state_key(key);
            let shape = match exact.get(&(st.outline.to_string(), st.collision.to_string())) {
                Some(s) => s.clone(),
                None => {
                    near += 1;
                    let (outline, collision) = (bounds(&st.outline), bounds(&st.collision));
                    let key = bounds_key(outline, collision);
                    let known = nearest
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .get(&key)
                        .cloned();
                    match known {
                        Some(n) => n,
                        None => {
                            let d = |(_, o, c): &(String, Option<[f64; 6]>, Option<[f64; 6]>)| {
                                bounds_distance(*c, collision) + bounds_distance(*o, outline) * 0.5
                            };
                            // Over one state per pair of bounds: the same distances, and the
                            // tie on the name goes to the same state (each entry's name is
                            // the least of its pair's).
                            let n = distinct
                                .iter()
                                .min_by(|a, b| d(a).total_cmp(&d(b)).then(a.0.cmp(&b.0)))
                                .map(|(n, _, _)| n.clone())?;
                            nearest
                                .lock()
                                .unwrap_or_else(|e| e.into_inner())
                                .insert(key, n.clone());
                            n
                        }
                    }
                }
            };
            let properties: Map<String, Value> = pairs(&key)
                .into_iter()
                .map(|(k, v)| (k, Value::String(v)))
                .collect();
            let mut v = json!({"properties": properties, "shape": shape});
            // Its own boxes where the vanilla state's differ: the nearest state's outline and
            // collision are a guess, and a state's occlusion (the faces it hides) need not be
            // its stand-in's (a basin on a full cube's hid the floor under it).
            let like = by_name.get(&shape);
            let differs = |i: usize, own: &Value| {
                let own = own.to_string();
                like.is_none_or(|l| l[i] != own)
            };
            if differs(0, &st.outline) {
                v["outline"] = st.outline.clone();
            }
            if differs(1, &st.collision) {
                v["collision"] = st.collision.clone();
            }
            if let Some(occlusion) = &st.occlusion
                && differs(2, occlusion)
            {
                v["occlusion"] = occlusion.clone();
            }
            if st.emission != default_light {
                v["light"] = json!(st.emission);
            }
            if st.random_ticks {
                v["random_ticks"] = json!(true);
            }
            if key == default {
                v["default"] = json!(true);
            }
            states.push(v);
        }
        if near > 0 {
            choice.differences.push(format!(
                "shapes: {near} of {} states take the nearest vanilla state's",
                states.len()
            ));
        }
        choice.differences.push(format!(
            "its own states: behaviour beyond a plain block ({})",
            block.class.first().map_or("", String::as_str)
        ));
        Some((choice, compact_states(states)))
    }
}
