//! A block's own states (a Java mod's block: horizon-port-pack's carry), in place of the
//! template's, and the collision templates of blocks with their template's rows.
use super::*;
use std::collections::HashMap;

/// One of a block's own states.
pub(super) struct OwnState {
    pub(super) properties: Vec<(String, String)>,
    /// The vanilla state whose shapes it has.
    pub(super) shape: u32,
    /// Its own outline, collision and occlusion, in place of `shape`'s (a Java mod's block
    /// whose shapes no vanilla state has).
    pub(super) outline: Option<VoxelShape>,
    pub(super) collision: Option<VoxelShape>,
    pub(super) occlusion: Option<VoxelShape>,
    light: Option<u8>,
    random_ticks: Option<bool>,
}

/// Its own states (a Java mod's block: horizon-port-pack's carry), in place of
/// the template's: the template's single state copied once per state, each
/// with its properties, the shapes of a vanilla state, its light.
pub(super) fn own_states(
    decl: &Block,
    states: &Range<u32>,
    combos: &[Vec<(String, String)>],
    shapes: &Option<Vec<VoxelShape>>,
    facts: &J,
    by_key: &mut Option<HashMap<String, u32>>,
) -> Result<Vec<OwnState>> {
    let mut own = Vec::new();
    if !decl.states.is_empty() {
        if states.len() != 1 || combos.len() != 1 || shapes.is_some() {
            return Err(format!(
                "{}: a block with its own states copies a template without \
                 properties, and declares no state properties or shapes ({})",
                decl.id, decl.template
            )
            .into());
        }
        let by_key = by_key.get_or_insert_with(|| state_keys(facts));
        // Own boxes shared over a block's states (a pipe's thousands have a few).
        let mut own_shapes: HashMap<Vec<[u64; 6]>, VoxelShape> = HashMap::new();
        for st in &decl.states {
            let shape = match &st.shape {
                None => states.start,
                Some(name) => shape_state(by_key, name)
                    .ok_or_else(|| format!("{}: shape {name} is not a block state", decl.id))?,
            };
            let mut boxed = |b: &Option<Vec<[f64; 6]>>| -> Result<Option<VoxelShape>> {
                let Some(b) = b else { return Ok(None) };
                let key: Vec<[u64; 6]> = b.iter().map(|x| x.map(f64::to_bits)).collect();
                if let Some(v) = own_shapes.get(&key) {
                    return Ok(Some(VoxelShape::clone(v)));
                }
                let v = boxes_shape(b)?;
                own_shapes.insert(key, v.clone());
                Ok(Some(v))
            };
            let (outline, collision, occlusion) = (
                boxed(&st.outline)?,
                boxed(&st.collision)?,
                boxed(&st.occlusion)?,
            );
            own.push(OwnState {
                properties: st
                    .properties
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
                shape,
                outline,
                collision,
                occlusion,
                light: st.light,
                random_ticks: st.random_ticks,
            });
        }
    }
    Ok(own)
}

/// How many states a block with these own states adds, if it has any.
pub(super) fn own_count(own: &[OwnState]) -> Option<u32> {
    (!own.is_empty()).then_some(own.len() as u32)
}

impl NewBlock<'_> {
    /// How many states it adds.
    pub(super) fn count(&self) -> u32 {
        if self.own.is_empty() {
            (self.rename.states.len() as u32).saturating_mul(self.combos.len() as u32)
        } else {
            self.own.len() as u32
        }
    }
    /// Each new state's source, in registry order: the template state it copies, the
    /// declared properties' combination it adds, and its own state's index (one per own
    /// state).
    pub(super) fn sources(&self) -> Vec<(u32, &[(String, String)], Option<usize>)> {
        if self.own.is_empty() {
            self.state_combinations()
                .map(|(id, combo)| (id, combo, None))
                .collect()
        } else {
            (0..self.own.len())
                .map(|i| (self.rename.states.start, &[][..], Some(i)))
                .collect()
        }
    }
    /// `k=v,...` of an own state.
    fn key(&self, i: usize) -> String {
        let pairs: Vec<String> = self.own[i]
            .properties
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        pairs.join(",")
    }

    /// Own state `o`'s row of a table whose rows follow `rows`, made from the template's
    /// `row`; `list` is the table's vanilla rows, `next` the state's id.
    pub(super) fn own_row(&self, o: usize, rows: Rows, next: u32, list: &[J], row: &mut J) {
        let state = &self.own[o];
        let shape = &list[state.shape as usize];
        match rows {
            Rows::Facts => {
                if let J::Arr(cols) = row {
                    let key = self.key(o);
                    let name = if key.is_empty() {
                        format!("Block{{{}}}", self.decl.id)
                    } else {
                        format!("Block{{{}}}[{key}]", self.decl.id)
                    };
                    let props: Vec<(String, J)> = state
                        .properties
                        .iter()
                        .map(|(k, v)| (k.clone(), J::Str(v.clone())))
                        .collect();
                    let mut identity = Vec::new();
                    if !props.is_empty() {
                        identity.push(("properties".to_owned(), J::Obj(props)));
                    }
                    identity.push(("id".to_owned(), J::Str(self.decl.id.clone())));
                    let water = state
                        .properties
                        .iter()
                        .any(|(k, v)| k == "waterlogged" && v == "true");
                    if cols.len() >= 5 {
                        cols[2] = J::Bool(water);
                        cols[3] = J::Str(name);
                        cols[4] = J::Obj(identity);
                    }
                }
            }
            Rows::Gameplay => {
                for field in SHAPE_FIELDS {
                    if let Some(v) = shape.get(field) {
                        row.set(field, v.clone());
                    }
                }
            }
            Rows::Behaviour => {}
            Rows::Shape => *row = shape.clone(),
            Rows::StateIds => {
                if let J::Arr(cols) = row {
                    cols.fill(J::Num(next.into()));
                }
            }
        }
    }

    /// An own state's light and random ticks on its gameplay row.
    pub(super) fn own_gameplay(&self, own: Option<usize>, row: &mut J) {
        if let Some(l) = own.and_then(|o| self.own[o].light) {
            row.set("emission", J::Num(l.into()));
        }
        if let Some(r) = own.and_then(|o| self.own[o].random_ticks) {
            row.set("random_ticks", J::Bool(r));
        }
    }

    /// A block with its own states: its entry `v` of a map by name, the template's renamed.
    pub(super) fn own_names(&self, v: &mut J, ids: bool) {
        if !self.own.is_empty() {
            if ids && let Some(states) = v.get_mut("states") {
                // render-definitions: the block's state ids.
                *states = J::Arr(
                    (0..self.count())
                        .map(|i| J::Num((self.rename.first + i).into()))
                        .collect(),
                );
            } else if !ids && v.get("schema").is_some() {
                // model-states: its properties and states.
                *v = own_model_states(self);
            }
        }
    }

    /// facts.json's default of a block with its own states.
    pub(super) fn own_default(&self) -> Option<u32> {
        (!self.own.is_empty()).then(|| {
            let i = self.decl.states.iter().position(|s| s.default).unwrap_or(0);
            self.rename.first + i as u32
        })
    }

    /// The NBT `states` rows of a block with its own states.
    pub(super) fn own_nbt_states(&self) -> impl Iterator<Item = Tag> + '_ {
        self.own.iter().map(|o| {
            horizon_world::chunk::BlockState {
                name: self.decl.id.clone(),
                properties: o.properties.iter().cloned().collect(),
            }
            .to_nbt()
        })
    }

    /// `cached_shapes` of a block with its own states: an own state its shape state's.
    pub(super) fn own_cached_shapes(&self, a: &mut Vec<i32>) {
        for o in &self.own {
            a.push(a[o.shape as usize]);
        }
    }
}

/// What a table's rows follow for a block's own states.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Rows {
    /// facts.json: the template's row with the state's name and properties.
    Facts,
    /// gameplay-facts.json: the template's row with the shape state's shapes.
    Gameplay,
    /// The template's row.
    Behaviour,
    /// The shape state's row (pathfinding, `full_block`).
    Shape,
    /// State ids (rotate, mirror): the state itself.
    StateIds,
}

/// The gameplay fields that follow a state's shapes.
const SHAPE_FIELDS: [&str; 10] = [
    "outline",
    "outline_edges",
    "outline_offset",
    "offset",
    "full_collision",
    "shape_occlusion",
    "can_occlude",
    "solid_render",
    "skylight_down",
    "dampening",
];

impl Plan<'_> {
    /// Appends each new block's copies of the template's states to `list`;
    /// `gameplay` rows get their index and the declared overrides.
    pub(super) fn states(&self, list: &mut Vec<J>, ids: bool, gameplay: bool) -> Result<()> {
        let rows = if gameplay {
            Rows::Gameplay
        } else if ids {
            Rows::StateIds
        } else {
            Rows::Behaviour
        };
        self.rows(list, rows)
    }

    /// [`Self::rows`] on the list at `path` of `doc`.
    pub(super) fn rows_at(&self, doc: &mut J, path: &str, rows: Rows) -> Result<()> {
        match doc.at(path) {
            Some(J::Arr(a)) => self.rows(a, rows),
            _ => Err(format!("no list at /{path}").into()),
        }
    }

    /// Blocks with their template's rows: where a template state has no cached shape
    /// (scaffolding, bamboo...), its rule is the template's (`CachedCollisionRegistry::template`).
    pub(super) fn collision_templates(&self) -> Tag {
        let templates: Compound = self
            .blocks
            .iter()
            .filter(|b| b.collision_shapes.is_none() && b.shapes.is_none())
            .map(|b| {
                (
                    JavaString::from(b.rename.to.as_str()),
                    Tag::String(JavaString::from(b.rename.from.as_str())),
                )
            })
            .collect();
        Tag::Compound(templates)
    }
}

/// `name[k=v,...]`, its properties sorted, for each state in facts.json.
fn state_keys(facts: &J) -> HashMap<String, u32> {
    let Some(J::Arr(rows)) = facts.get("states") else {
        return HashMap::new();
    };
    rows.iter()
        .enumerate()
        .filter_map(|(id, row)| {
            let J::Arr(row) = row else { return None };
            let Some(J::Str(name)) = row.first() else {
                return None;
            };
            let mut pairs: Vec<String> = match row.get(4).and_then(|r| r.get("properties")) {
                Some(J::Obj(p)) => p
                    .iter()
                    .filter_map(|(k, v)| match v {
                        J::Str(v) => Some(format!("{k}={v}")),
                        _ => None,
                    })
                    .collect(),
                _ => Vec::new(),
            };
            pairs.sort();
            Some((format!("{name}[{}]", pairs.join(",")), id as u32))
        })
        .collect()
}

/// A state's name, `minecraft:cake[bites=1]` or `minecraft:stone`, as [`state_keys`] has it.
fn normal_state(name: &str) -> String {
    let (block, props) = match name.split_once('[') {
        Some((b, rest)) => (b, rest.trim_end_matches(']')),
        None => (name, ""),
    };
    let mut pairs: Vec<&str> = props.split(',').filter(|p| !p.is_empty()).collect();
    pairs.sort_unstable();
    format!("{block}[{}]", pairs.join(","))
}

/// The game's state a carried block's shape names: `name` itself, else, when a Java mod gives
/// the vanilla block properties of its own (The Sift's `ichorlogged` on every leaves block,
/// through a mixin on its state definition), the state of that block with the most of the
/// named properties and none other: the shape is the vanilla block's.
fn shape_state(by_key: &HashMap<String, u32>, name: &str) -> Option<u32> {
    let key = normal_state(name);
    if let Some(&id) = by_key.get(&key) {
        return Some(id);
    }
    let (block, props) = key.split_once('[')?;
    let named: std::collections::HashSet<&str> = props
        .trim_end_matches(']')
        .split(',')
        .filter(|p| !p.is_empty())
        .collect();
    let prefix = format!("{block}[");
    by_key
        .iter()
        .filter_map(|(k, &id)| {
            let pairs = k.strip_prefix(&prefix)?.strip_suffix(']')?;
            let pairs: Vec<&str> = pairs.split(',').filter(|p| !p.is_empty()).collect();
            pairs.iter().all(|p| named.contains(p)).then_some((
                pairs.len(),
                std::cmp::Reverse(id),
                id,
            ))
        })
        .max()
        .map(|(_, _, id)| id)
}

/// model-states.json's entry for a block with its own states: each property's values
/// (`integer` when they all are), and the states in order.
fn own_model_states(b: &NewBlock) -> J {
    let mut values: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for s in &b.own {
        for (k, v) in &s.properties {
            let list = values.entry(k).or_default();
            if !list.contains(&v.as_str()) {
                list.push(v);
            }
        }
    }
    let schema = values
        .into_iter()
        .map(|(k, mut vs)| {
            let ints: Option<Vec<i64>> = vs.iter().map(|v| v.parse().ok()).collect();
            let kind = match ints {
                Some(ints) => J::Obj(vec![(
                    "integer".into(),
                    J::Arr(vec![
                        J::Num((*ints.iter().min().unwrap_or(&0)).into()),
                        J::Num((*ints.iter().max().unwrap_or(&0)).into()),
                    ]),
                )]),
                None => {
                    vs.sort_unstable();
                    J::Obj(vec![(
                        "named".into(),
                        J::Arr(vs.into_iter().map(|v| J::Str(v.into())).collect()),
                    )])
                }
            };
            (k.to_owned(), kind)
        })
        .collect();
    let states = b
        .own
        .iter()
        .map(|s| {
            J::Obj(
                s.properties
                    .iter()
                    .map(|(k, v)| (k.clone(), J::Str(v.clone())))
                    .collect(),
            )
        })
        .collect();
    J::Obj(vec![
        ("schema".into(), J::Obj(schema)),
        ("states".into(), J::Arr(states)),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mods_extra_vanilla_property_falls_back_to_the_vanilla_state() {
        let by_key: HashMap<String, u32> = [
            (
                "minecraft:acacia_leaves[distance=1,persistent=false,waterlogged=false]",
                7,
            ),
            (
                "minecraft:acacia_leaves[distance=2,persistent=false,waterlogged=false]",
                8,
            ),
            ("minecraft:stone[]", 1),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v))
        .collect();
        let named = "minecraft:acacia_leaves[distance=1,ichorlogged=false,persistent=false,waterlogged=false]";
        assert_eq!(shape_state(&by_key, named), Some(7));
        assert_eq!(shape_state(&by_key, "minecraft:stone"), Some(1));
        assert_eq!(shape_state(&by_key, "minecraft:dirt[snowy=false]"), None);
    }
}
