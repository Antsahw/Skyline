//! Wavefront OBJ geometry for block and item models, in the form Java mods ship it for
//! NeoForge's model loader (`"loader": "neoforge:obj"`, also `forge:obj`): the model JSON names
//! the `.obj` (`"model": "ns:models/block/x.obj"`) beside its usual parent, textures and display.
//! Fabric ports (Create Fly) carry those models unchanged and bake them with their own loader.
//!
//! As NeoForge's `ObjModel` reads them: positions in block units (0..1), texture coordinates
//! in the sprite's own 0..1 (`flip_v` turns V over), each face's material (`usemtl`) naming the
//! texture its `.mtl` gives as `map_Kd` (a model slot `#name`, or a texture id); triangles
//! become quads with the last corner repeated, faces of more corners fans of quads.
use std::collections::BTreeMap;

/// What a model JSON's loader asks for: the OBJ file and how to read it.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjModel {
    /// The `.obj` file's resource id (`ns:models/block/x.obj`).
    pub model: String,
    pub flip_v: bool,
    /// Filled once the file is read ([`ObjModel::load`]); empty until then.
    pub quads: Vec<ObjQuad>,
}

/// One face: four corners (a triangle's last repeated), UVs in the sprite's 0..1, and the
/// texture: a model slot without its `#`, or a texture id.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjQuad {
    pub positions: [[f32; 3]; 4],
    pub uv: [[f32; 2]; 4],
    pub texture: String,
}

/// Whether a model JSON's `loader` is an OBJ loader.
pub fn is_obj_loader(loader: &str) -> bool {
    matches!(
        loader,
        "neoforge:obj" | "forge:obj" | "minecraft:obj" | "fabric:obj"
    )
}

impl ObjModel {
    /// The `.obj` file under a resource root: `assets/<ns>/<path>`.
    pub fn file(&self) -> Option<(String, String)> {
        let (namespace, path) = self
            .model
            .split_once(':')
            .unwrap_or(("minecraft", &self.model));
        (!path.contains("..")).then(|| (namespace.to_owned(), path.to_owned()))
    }

    /// Reads the OBJ text; `mtl` gives a material library's text by its file name (relative
    /// to the `.obj`'s folder), or None.
    pub fn load(&mut self, obj: &str, mut mtl: impl FnMut(&str) -> Option<String>) {
        let mut positions: Vec<[f32; 3]> = Vec::new();
        let mut uvs: Vec<[f32; 2]> = Vec::new();
        let mut materials: BTreeMap<String, String> = BTreeMap::new();
        let mut texture = String::from("missingno");
        let mut quads = Vec::new();
        for line in obj.lines() {
            let line = line.trim();
            let mut words = line.split_whitespace();
            let Some(key) = words.next() else {
                continue;
            };
            let numbers = |words: std::str::SplitWhitespace| -> Vec<f32> {
                words.filter_map(|w| w.parse::<f32>().ok()).collect()
            };
            match key {
                "v" => {
                    let n = numbers(words);
                    if n.len() >= 3 {
                        positions.push([n[0], n[1], n[2]]);
                    }
                }
                "vt" => {
                    let n = numbers(words);
                    let u = n.first().copied().unwrap_or(0.0);
                    let v = n.get(1).copied().unwrap_or(0.0);
                    uvs.push([u, if self.flip_v { 1.0 - v } else { v }]);
                }
                "mtllib" => {
                    for name in words {
                        if let Some(text) = mtl(name) {
                            read_mtl(&text, &mut materials);
                        }
                    }
                }
                "usemtl" => {
                    let name = words.next().unwrap_or_default();
                    texture = materials
                        .get(name)
                        .cloned()
                        .unwrap_or_else(|| name.to_owned());
                }
                "f" => {
                    let corners: Vec<([f32; 3], [f32; 2])> =
                        words.filter_map(|w| corner(w, &positions, &uvs)).collect();
                    if corners.len() < 3 {
                        continue;
                    }
                    // A fan of quads from the first corner: (0, i, i+1, i+2), the last repeated
                    // when the face runs out.
                    let mut i = 1;
                    while i + 1 < corners.len() {
                        let pick = |k: usize| corners[(i + k).min(corners.len() - 1)];
                        let c = [corners[0], pick(0), pick(1), pick(2)];
                        quads.push(ObjQuad {
                            positions: c.map(|(p, _)| p),
                            uv: c.map(|(_, t)| t),
                            texture: texture.clone(),
                        });
                        i += 2;
                    }
                }
                _ => {}
            }
        }
        self.quads = quads;
    }
}

/// A face corner `v`, `v/vt`, `v//vn` or `v/vt/vn` (1-based; negative counts from the end).
fn corner(word: &str, positions: &[[f32; 3]], uvs: &[[f32; 2]]) -> Option<([f32; 3], [f32; 2])> {
    let mut parts = word.split('/');
    let index = |s: Option<&str>, len: usize| -> Option<usize> {
        let i: i64 = s.filter(|s| !s.is_empty())?.parse().ok()?;
        let i = if i < 0 { len as i64 + i } else { i - 1 };
        (0..len as i64).contains(&i).then_some(i as usize)
    };
    let p = positions[index(parts.next(), positions.len())?];
    let t = index(parts.next(), uvs.len()).map_or([0.0, 0.0], |i| uvs[i]);
    Some((p, t))
}

/// `newmtl name` … `map_Kd texture`: the material's texture, a slot reference (`#0`, kept
/// without its `#`) or a texture id.
fn read_mtl(text: &str, out: &mut BTreeMap<String, String>) {
    let mut current: Option<String> = None;
    for line in text.lines() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("newmtl") => current = words.next().map(str::to_owned),
            Some("map_Kd") => {
                if let (Some(name), Some(texture)) = (&current, words.next()) {
                    let texture = texture.strip_prefix('#').unwrap_or(texture);
                    out.insert(name.clone(), texture.to_owned());
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MTL: &str = "newmtl side\nmap_Kd #0\nnewmtl top\nmap_Kd create:block/axis_top\n";

    fn load(obj: &str, flip_v: bool) -> Vec<ObjQuad> {
        let mut m = ObjModel {
            model: "create:models/block/x.obj".into(),
            flip_v,
            quads: Vec::new(),
        };
        m.load(obj, |name| (name == "x.mtl").then(|| MTL.to_owned()));
        m.quads
    }

    #[test]
    fn quads_triangles_and_materials() {
        let obj = "mtllib x.mtl\nv 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 1 1\nvt 0 1\n\
                   usemtl side\nf 1/1 2/2 3/3 4/4\nusemtl top\nf 1/1/1 2/2/1 3/3/1\n";
        let q = load(obj, false);
        assert_eq!(q.len(), 2);
        assert_eq!(q[0].texture, "0");
        assert_eq!(q[0].positions[2], [1.0, 1.0, 0.0]);
        assert_eq!(q[0].uv[3], [0.0, 1.0]);
        assert_eq!(q[1].texture, "create:block/axis_top");
        // A triangle's last corner is repeated.
        assert_eq!(q[1].positions[3], q[1].positions[2]);
    }

    #[test]
    fn flip_v_and_negative_indexes() {
        let obj = "v 0 0 0\nv 1 0 0\nv 1 1 0\nvt 0.25 0.25\nf -3/-1 -2/-1 -1/-1\n";
        let q = load(obj, true);
        assert_eq!(q.len(), 1);
        assert_eq!(q[0].uv[0], [0.25, 0.75]);
        assert_eq!(q[0].positions[0], [0.0, 0.0, 0.0]);
    }

    #[test]
    fn a_pentagon_is_a_fan_of_two_quads() {
        let obj = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0.5 2 0\nv 0 1 0\nf 1 2 3 4 5\n";
        let q = load(obj, false);
        assert_eq!(q.len(), 2);
        assert_eq!(
            q[1].positions,
            [
                [0.0, 0.0, 0.0],
                [0.5, 2.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 1.0, 0.0]
            ]
        );
    }
}
