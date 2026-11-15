//! What the 1.20.1 mods make in code rather than ship as files, made into the files Horizon
//! reads: the resources of runtime resource packs ([`add_runtime_packs`]: ARRP's, which Medieval
//! Weapons makes its weapons' models in), and the items whose model is their renderer
//! ([`code_items`]: `builtin/entity`, Epic Knights' shields), drawn once by that renderer and
//! written as a model of flat elements on the textures it drew with.

use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Within this much (model units, sixteenths of a block) two coordinates are the same.
const EPS: f32 = 1e-3;

/// The files a runtime pack holds (the runtime's `runtimePacks`, written into `dir` as a jar
/// holds them: `assets/<ns>/...`), added to the jar in `mods` of the mod whose id is the
/// namespace, so the carry-over takes them as the mod's own. A file the jar has stays the jar's.
/// How many files were added.
pub(super) fn add_runtime_packs(
    written: &str,
    dir: &Path,
    mods: &Path,
    log: &dyn Fn(String),
) -> usize {
    let Ok(Value::Object(namespaces)) = serde_json::from_str(written) else {
        return 0;
    };
    let mut added = 0;
    for ns in namespaces.keys() {
        let Some(jar) = jar_of(mods, ns) else {
            continue;
        };
        let root = dir.join("assets").join(ns);
        let mut files = Vec::new();
        collect(&root, &mut files);
        let extra: Vec<(String, PathBuf)> = files
            .into_iter()
            .filter_map(|f| {
                let rel = f
                    .strip_prefix(dir)
                    .ok()?
                    .to_string_lossy()
                    .replace('\\', "/");
                Some((rel, f))
            })
            .collect();
        match add_to_jar(&jar, &extra) {
            Ok(n) => added += n,
            Err(e) => log(format!("{}: {e}", jar.display())),
        }
    }
    added
}

/// The jar in `mods` whose `fabric.mod.json` names the mod `id`.
fn jar_of(mods: &Path, id: &str) -> Option<PathBuf> {
    let entries = std::fs::read_dir(mods).ok()?;
    entries.flatten().map(|e| e.path()).find(|p| {
        let Ok(bytes) = std::fs::read(p) else {
            return false;
        };
        let Ok(mut zip) = zip::ZipArchive::new(std::io::Cursor::new(&bytes[..])) else {
            return false;
        };
        super::mod_json(&mut zip).is_some_and(|j| j["id"].as_str() == Some(id))
    })
}

/// Every file under `dir`, at any depth.
fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.push(path);
        }
    }
}

/// `jar` rewritten with `extra` (name in the jar, file) added where it has no such entry.
fn add_to_jar(jar: &Path, extra: &[(String, PathBuf)]) -> Result<usize, String> {
    use std::io::Write;
    let bytes = std::fs::read(jar).map_err(|e| e.to_string())?;
    let mut zip =
        zip::ZipArchive::new(std::io::Cursor::new(&bytes[..])).map_err(|e| e.to_string())?;
    let mut out = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let mut names = std::collections::HashSet::new();
    for i in 0..zip.len() {
        let entry = zip.by_index_raw(i).map_err(|e| e.to_string())?;
        names.insert(entry.name().to_owned());
        out.raw_copy_file(entry).map_err(|e| e.to_string())?;
    }
    let options: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    let mut added = 0;
    for (name, file) in extra {
        if names.contains(name) {
            continue;
        }
        let body = std::fs::read(file).map_err(|e| e.to_string())?;
        out.start_file(name.as_str(), options)
            .map_err(|e| e.to_string())?;
        out.write_all(&body).map_err(|e| e.to_string())?;
        added += 1;
    }
    let bytes = out.finish().map_err(|e| e.to_string())?.into_inner();
    std::fs::write(jar, bytes).map_err(|e| e.to_string())?;
    Ok(added)
}

/// The items whose model is their code (the runtime's `codeItems`: `{item: [mesh]}`, each mesh
/// `{"tex", "v"}` with nine numbers a vertex, four vertices a quad, in block units around the
/// item cube's centre; the textures written into `textures` by their resource path) as models
/// Horizon draws: the item's model, and each model its overrides switch to that is drawn by code
/// too, rewritten as flat elements, one a quad, keeping the display transforms the model had
/// through its parents. The textures are copied beside the mod's item textures
/// (`item/horizon_code/<ns>/<path>`). A quad that is not a rectangle on one of the block's axes
/// can't be an element and is left out. How many items.
pub(super) fn code_items(
    drawn: &str,
    textures: &Path,
    carried: &[PathBuf],
    log: &dyn Fn(String),
) -> usize {
    let Ok(Value::Object(items)) = serde_json::from_str(drawn) else {
        return 0;
    };
    let mut written = 0;
    let mut left_out = 0;
    for (id, meshes) in items {
        let Some((ns, path)) = id.split_once(':') else {
            continue;
        };
        let Some(carry) = super::carrying(carried, ns) else {
            continue;
        };
        let assets = carry.join("assets").join(ns);
        let model_file = assets.join("models/item").join(format!("{path}.json"));
        let Some(model) = read_json(&model_file) else {
            continue;
        };
        let mut texture_names = Map::new();
        let mut elements = Vec::new();
        for (k, mesh) in meshes.as_array().into_iter().flatten().enumerate() {
            let Some(tex) = mesh["tex"].as_str() else {
                continue;
            };
            let Some(sprite) = copy_texture(tex, textures, &assets) else {
                continue;
            };
            let key = format!("c{k}");
            texture_names.insert(key.clone(), json!(format!("{ns}:{sprite}")));
            let v: Vec<f32> = mesh["v"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|n| n.as_f64().map(|n| n as f32))
                .collect();
            for quad in v.chunks_exact(36) {
                let corners: [([f32; 3], [f32; 2]); 4] = std::array::from_fn(|i| {
                    let at = &quad[i * 9..];
                    (
                        [
                            (at[0] + 0.5) * 16.0,
                            (at[1] + 0.5) * 16.0,
                            (at[2] + 0.5) * 16.0,
                        ],
                        [at[3] * 16.0, at[4] * 16.0],
                    )
                });
                match element(&corners, &key) {
                    Some(e) => elements.push(e),
                    None => left_out += 1,
                }
            }
        }
        if elements.is_empty() {
            continue;
        }
        if let Some(first) = texture_names.values().next().cloned() {
            texture_names.entry("particle".to_owned()).or_insert(first);
        }
        let rewrite = |file: &Path, model: &Value| {
            let mut out = Map::new();
            out.insert("textures".into(), Value::Object(texture_names.clone()));
            out.insert("elements".into(), Value::Array(elements.clone()));
            let (display, gui_light) = inherited(model, &carry.join("assets"));
            if !display.is_empty() {
                out.insert("display".into(), Value::Object(display));
            }
            if let Some(light) = gui_light {
                out.insert("gui_light".into(), light);
            }
            if let Some(overrides) = model.get("overrides") {
                out.insert("overrides".into(), overrides.clone());
            }
            std::fs::write(file, Value::Object(out).to_string()).is_ok()
        };
        if !rewrite(&model_file, &model) {
            continue;
        }
        written += 1;
        // The models its overrides switch to (a shield raised), drawn by code as it is.
        for o in model["overrides"].as_array().into_iter().flatten() {
            let Some((tns, tpath)) = o["model"].as_str().and_then(|m| m.split_once(':')) else {
                continue;
            };
            let file = carry
                .join("assets")
                .join(tns)
                .join("models")
                .join(format!("{tpath}.json"));
            if let Some(target) = read_json(&file)
                && reaches_builtin(&target, &carry.join("assets"))
            {
                rewrite(&file, &target);
            }
        }
    }
    if left_out > 0 {
        log(format!(
            "{left_out} quad(s) of items drawn by code are not on an axis, left out"
        ));
    }
    written
}

fn read_json(file: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(file).ok()?).ok()
}

/// A model's parent as a file under `assets`, or None for `builtin/...` and models the carry
/// doesn't hold.
fn parent_of(model: &Value, assets: &Path) -> Option<(String, Value)> {
    let parent = model["parent"].as_str()?;
    let (ns, path) = parent.split_once(':').unwrap_or(("minecraft", parent));
    let file = assets.join(ns).join("models").join(format!("{path}.json"));
    Some((parent.to_owned(), read_json(&file)?))
}

/// Whether a model's parents end in `builtin/entity`.
fn reaches_builtin(model: &Value, assets: &Path) -> bool {
    let mut at = model.clone();
    for _ in 0..16 {
        if at["parent"]
            .as_str()
            .is_some_and(|p| p.trim_start_matches("minecraft:") == "builtin/entity")
        {
            return true;
        }
        match parent_of(&at, assets) {
            Some((_, p)) => at = p,
            None => return false,
        }
    }
    false
}

/// The display transforms a model has, its own over its parents', and its `gui_light`.
fn inherited(model: &Value, assets: &Path) -> (Map<String, Value>, Option<Value>) {
    let mut chain = vec![model.clone()];
    for _ in 0..16 {
        match parent_of(chain.last().expect("one"), assets) {
            Some((_, p)) => chain.push(p),
            None => break,
        }
    }
    let mut display = Map::new();
    let mut gui_light = None;
    for m in chain.iter().rev() {
        if let Some(d) = m["display"].as_object() {
            for (k, v) in d {
                display.insert(k.clone(), v.clone());
            }
        }
        if let Some(l) = m.get("gui_light") {
            gui_light = Some(l.clone());
        }
    }
    (display, gui_light)
}

/// `tex` (`ns:textures/path.png`, as the runtime wrote it under `textures`) copied into
/// `assets/textures/item/horizon_code/<ns>/<path>.png`: its sprite name, `item/horizon_code/...`.
fn copy_texture(tex: &str, textures: &Path, assets: &Path) -> Option<String> {
    let (ns, path) = tex.split_once(':')?;
    let from = textures.join(ns).join(path);
    let inner = path.strip_prefix("textures/")?.strip_suffix(".png")?;
    let sprite = format!("item/horizon_code/{ns}/{inner}");
    let to = assets.join("textures").join(format!("{sprite}.png"));
    if !to.is_file() {
        std::fs::create_dir_all(to.parent()?).ok()?;
        std::fs::copy(&from, &to).ok()?;
        let meta = from.with_extension("png.mcmeta");
        if meta.is_file() {
            let _ = std::fs::copy(&meta, to.with_extension("png.mcmeta"));
        }
    }
    Some(sprite)
}

/// The corners of a face as vanilla bakes an element's face (`FaceInfo`): which of `from` (0)
/// or `to` (1) each axis takes, corner by corner. Directions in the order down, up, north,
/// south, west, east.
const CORNERS: [[[usize; 3]; 4]; 6] = [
    [[0, 0, 1], [0, 0, 0], [1, 0, 0], [1, 0, 1]],
    [[0, 1, 0], [0, 1, 1], [1, 1, 1], [1, 1, 0]],
    [[1, 1, 0], [1, 0, 0], [0, 0, 0], [0, 1, 0]],
    [[0, 1, 1], [0, 0, 1], [1, 0, 1], [1, 1, 1]],
    [[0, 1, 0], [0, 0, 0], [0, 0, 1], [0, 1, 1]],
    [[1, 1, 1], [1, 0, 1], [1, 0, 0], [1, 1, 0]],
];
const NAMES: [&str; 6] = ["down", "up", "north", "south", "west", "east"];

/// A quad (positions and UVs in model units) as a flat element with the one face, on texture
/// `#key`: None when it is not a rectangle on one of the block's axes, lies outside what a model
/// may hold, or its UVs are no rotation or flip of a face's.
fn element(quad: &[([f32; 3], [f32; 2]); 4], key: &str) -> Option<Value> {
    let p = quad.map(|(p, _)| p);
    let lo: [f32; 3] = std::array::from_fn(|a| p.iter().map(|v| v[a]).fold(f32::MAX, f32::min));
    let hi: [f32; 3] = std::array::from_fn(|a| p.iter().map(|v| v[a]).fold(f32::MIN, f32::max));
    let axis = (0..3).find(|&a| hi[a] - lo[a] < EPS)?;
    if (0..3).any(|a| a != axis && hi[a] - lo[a] < EPS) {
        return None;
    }
    if lo
        .iter()
        .chain(hi.iter())
        .any(|&c| !(-16.0..=32.0).contains(&c))
    {
        return None;
    }
    // A rectangle: every corner on a corner of the bounds.
    let on_corner =
        |v: &[f32; 3]| (0..3).all(|a| (v[a] - lo[a]).abs() < EPS || (v[a] - hi[a]).abs() < EPS);
    if !p.iter().all(on_corner) {
        return None;
    }
    let n = {
        let a: [f32; 3] = std::array::from_fn(|i| p[1][i] - p[0][i]);
        let b: [f32; 3] = std::array::from_fn(|i| p[2][i] - p[0][i]);
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ][axis]
    };
    if n.abs() < EPS * EPS {
        return None;
    }
    let direction = match (axis, n > 0.0) {
        (1, false) => 0,
        (1, true) => 1,
        (2, false) => 2,
        (2, true) => 3,
        (0, false) => 4,
        _ => 5,
    };
    let (from, to) = (lo, hi);
    // The UV each of the face's corners has, from the quad's vertex there.
    let uv_at: Vec<[f32; 2]> = CORNERS[direction]
        .iter()
        .map(|sel| {
            let at: [f32; 3] = std::array::from_fn(|a| if sel[a] == 0 { from[a] } else { to[a] });
            quad.iter()
                .find(|(v, _)| (0..3).all(|a| (v[a] - at[a]).abs() < EPS))
                .map(|(_, uv)| *uv)
        })
        .collect::<Option<_>>()?;
    let close = |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() < EPS && (a[1] - b[1]).abs() < EPS;
    for rotation in 0..4 {
        // Corner i takes bounds' corner (i + rotation) % 4: 0 (u1 v1), 1 (u1 v2), 2 (u2 v2), 3 (u2 v1).
        let of = |c: usize| uv_at[(c + 4 - rotation) % 4];
        let (a, b) = (of(0), of(2));
        if close(of(1), [a[0], b[1]]) && close(of(3), [b[0], a[1]]) {
            let r = |x: f32| (x * 1000.0).round() / 1000.0;
            let mut faces = BTreeMap::new();
            faces.insert(
                NAMES[direction],
                json!({"uv": [r(a[0]), r(a[1]), r(b[0]), r(b[1])], "rotation": rotation * 90,
                       "texture": format!("#{key}")}),
            );
            let r3 = |v: [f32; 3]| v.map(r);
            return Some(json!({"from": r3(from), "to": r3(to), "shade": true, "faces": faces}));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A north face drawn the way vanilla bakes one, its UVs [2, 4, 6, 12] turned each of the
    /// four ways: comes back as a face that bakes the same UV at every corner.
    #[test]
    fn a_quad_on_an_axis_is_the_face_it_was_baked_from() {
        let (from, to) = ([1.0, 2.0, 5.0], [9.0, 14.0, 5.0]);
        let at = |sel: [usize; 3]| -> [f32; 3] {
            std::array::from_fn(|a| if sel[a] == 0 { from[a] } else { to[a] })
        };
        let corner_uv = |bounds: [f32; 4], c: usize| {
            [
                bounds[if c < 2 { 0 } else { 2 }],
                bounds[if c == 0 || c == 3 { 1 } else { 3 }],
            ]
        };
        for rotation in 0..4 {
            let quad: [([f32; 3], [f32; 2]); 4] = std::array::from_fn(|i| {
                (
                    at(CORNERS[2][i]),
                    corner_uv([2.0, 4.0, 6.0, 12.0], (i + rotation) % 4),
                )
            });
            let e = element(&quad, "c0").expect("an element");
            assert_eq!(e["from"], json!([1.0, 2.0, 5.0]));
            assert_eq!(e["to"], json!([9.0, 14.0, 5.0]));
            let face = &e["faces"]["north"];
            let bounds: Vec<f32> = face["uv"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect();
            let turn = face["rotation"].as_u64().unwrap() as usize / 90;
            for (i, (_, uv)) in quad.iter().enumerate() {
                let baked = corner_uv([bounds[0], bounds[1], bounds[2], bounds[3]], (i + turn) % 4);
                assert_eq!(baked, *uv, "corner {i} of {e}");
            }
        }
    }

    /// A quad tilted off the axes can't be an element.
    #[test]
    fn a_tilted_quad_is_left_out() {
        let quad = [
            ([0.0, 0.0, 0.0], [0.0, 0.0]),
            ([0.0, 4.0, 1.0], [0.0, 1.0]),
            ([4.0, 4.0, 1.0], [1.0, 1.0]),
            ([4.0, 0.0, 0.0], [1.0, 0.0]),
        ];
        assert!(element(&quad, "c0").is_none());
    }
}
