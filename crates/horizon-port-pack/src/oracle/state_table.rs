//! A block's many own states as a `state_table`, small enough for the content fence.
use super::*;

/// From this many own states, a block's declaration lists them as a `state_table` (each
/// property's values and the distinct shapes once, a row of indices per state) instead of a
/// `states` array: ALA Industrial's pipes (76,832 states each) took 18.8 MB as an array, over
/// the content fence (`horizon_mods::content::MAX_FILE`), and the whole mod was dropped.
pub const STATE_TABLE_FROM: usize = 1024;

/// `states` (the declaration's array) as it goes in the file: the array itself when short,
/// else an object for `state_table`, rows in the same order.
pub fn compact_states(states: Vec<Value>) -> Value {
    if states.len() < STATE_TABLE_FROM {
        return Value::Array(states);
    }
    let mut props: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for s in &states {
        for (k, v) in s["properties"].as_object().into_iter().flatten() {
            let vals = props.entry(k.clone()).or_default();
            let v = v.as_str().unwrap_or_default();
            if !vals.iter().any(|x| x == v) {
                vals.push(v.to_owned());
            }
        }
    }
    let mut shapes: Vec<Value> = Vec::new();
    let mut shape_at: BTreeMap<String, usize> = BTreeMap::new();
    let (mut rows, mut light, mut ticks, mut default) = (Vec::new(), Map::new(), Vec::new(), None);
    for (i, s) in states.iter().enumerate() {
        let shape = match &s["shape"] {
            Value::String(n) => n.clone(),
            _ => String::new(),
        };
        let at = *shape_at.entry(shape.clone()).or_insert_with(|| {
            shapes.push(if shape.is_empty() {
                Value::Null
            } else {
                json!(shape)
            });
            shapes.len() - 1
        });
        let mut row = vec![at];
        for (k, vals) in &props {
            let v = s["properties"][k].as_str().unwrap_or_default();
            row.push(vals.iter().position(|x| x == v).unwrap_or(0));
        }
        rows.push(row);
        if let Some(l) = s.get("light") {
            light.insert(i.to_string(), l.clone());
        }
        if s["random_ticks"] == json!(true) {
            ticks.push(i);
        }
        if s["default"] == json!(true) {
            default = Some(i);
        }
    }
    let props: Vec<Value> = props.into_iter().map(|(k, v)| json!([k, v])).collect();
    let mut t = json!({"properties": props, "shapes": shapes, "rows": rows});
    if !light.is_empty() {
        t["light"] = Value::Object(light);
    }
    if !ticks.is_empty() {
        t["random_ticks"] = json!(ticks);
    }
    if let Some(d) = default {
        t["default"] = json!(d);
    }
    t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn many_own_states_fit_the_content_fence_and_read_back() {
        // A pipe: six sides of four kinds and a light level, 4^6 * 4 = 16,384 states, with
        // a handful of shapes (ALA Industrial's have 76,832, 49 shapes).
        let kinds = ["none", "pipe", "machine", "blocked"];
        let sides = ["down", "east", "north", "south", "up", "west"];
        let mut states = Vec::new();
        for n in 0..4usize.pow(7) {
            let mut properties = Map::new();
            for (i, side) in sides.iter().enumerate() {
                properties.insert((*side).into(), json!(kinds[n >> (2 * i) & 3]));
            }
            properties.insert("power".into(), json!((n >> 12).to_string()));
            let mut v = json!({"properties": properties, "shape": format!("minecraft:cake[bites={}]", n % 7)});
            if n >> 12 == 3 {
                v["light"] = json!(9);
            }
            if n == 5 {
                v["default"] = json!(true);
            }
            states.push(v);
        }
        let array = serde_json::to_vec(&Value::Array(states.clone())).unwrap();
        let table = compact_states(states.clone());
        assert!(table.is_object());
        let file =
            serde_json::to_vec(&json!({"template": "minecraft:stone", "state_table": table}))
                .unwrap();
        assert!(
            file.len() * 4 < array.len(),
            "{} vs {}",
            file.len(),
            array.len()
        );
        let manifest = "id = \"pipes\"\nname = \"Pipes\"\nversion = \"0.1.0\"\napi = \"0.2\"\nside = \"both\"\ncapabilities = [\"registries\"]\n";
        let dir = std::env::temp_dir().join(format!("horizon-state-table-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("m.hmod");
        let files: &[(&str, &[u8])] = &[("data/pipes/horizon/block/pipe.json", &file)];
        std::fs::write(
            &path,
            horizon_mods::Package::write_files(manifest, None, files).unwrap(),
        )
        .unwrap();
        let package = horizon_mods::Package::read(&path).unwrap();
        let content = horizon_mods::Content::read(&path, &package.manifest).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        let read = &content.blocks[0].states;
        assert_eq!(read.len(), states.len());
        for (r, s) in read.iter().zip(&states) {
            let props: BTreeMap<String, String> =
                serde_json::from_value(s["properties"].clone()).unwrap();
            assert_eq!(r.properties, props);
            assert_eq!(r.shape.as_deref(), s["shape"].as_str());
            assert_eq!(r.light, s.get("light").map(|l| l.as_u64().unwrap() as u8));
            assert_eq!(r.default, s.get("default").is_some());
        }
        // Few states stay an array.
        assert!(compact_states(states[..10].to_vec()).is_array());
    }
}
