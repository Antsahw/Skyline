//! Java mods' jars as data packs: a Fabric mod's data, which the mod loader adds as a pack of
//! its own, and the structures-only packs whose structures alone reach Horizon's generator.
use super::{DataPack, Source};
use crate::zip::Zip;
use std::path::Path;

impl DataPack {
    /// A folder pack (`pack.mcmeta` and `data/` in `dir`) under the id `id`.
    pub fn folder(id: &str, dir: &Path) -> Self {
        DataPack {
            id: id.into(),
            source: Source::Dir(dir.to_path_buf()),
            structures_only: false,
        }
    }
    /// A pack in an archive with `data/` at its root, under the id `id`: a
    /// Fabric mod's jar, whose data the mod loader adds as a pack of its own
    /// (no `pack.mcmeta` needed).
    pub fn archive(id: &str, path: &Path) -> std::io::Result<Self> {
        Ok(DataPack {
            id: id.into(),
            source: Source::Zip(std::sync::Arc::new(Zip::open(path)?)),
            structures_only: false,
        })
    }
    /// This pack with only its structures' files read by the generator:
    /// structures, structure sets, their biome tags and pieces ([`STRUCTURE_FILES`]).
    pub fn structures_only(mut self) -> Self {
        self.structures_only = true;
        self
    }
    /// A file the generator doesn't read from this pack (a structures-only pack's land).
    pub(super) fn skips(&self, file: &str) -> bool {
        self.structures_only && !is_structure_file(file)
    }
}

/// The folders of a pack's structures, under `data/<ns>/`: what a
/// structures-only pack gives the generator. The structures and their sets
/// (a mod's new villages), the biome tags their `biomes` name, the pieces
/// (templates, pools, processor lists) and the features a pool's
/// `feature_pool_element` places. A Java mod that changes a dimension's own
/// generation (its biomes, their features, the noise) has that dimension
/// generated on the Java side, so these new ids are all Horizon's generator
/// needs from it.
pub const STRUCTURE_FILES: [&str; 8] = [
    "structure/",
    "tags/worldgen/biome/",
    "worldgen/feature/",
    "worldgen/placed_feature/",
    "worldgen/processor_list/",
    "worldgen/structure/",
    "worldgen/structure_set/",
    "worldgen/template_pool/",
];
fn is_structure_file(file: &str) -> bool {
    file.strip_prefix("data/")
        .and_then(|r| r.split_once('/'))
        .is_some_and(|(_, rest)| STRUCTURE_FILES.iter().any(|k| rest.starts_with(k)))
}

#[cfg(test)]
mod tests {
    use super::super::*;
    /// A Java mod's jar as a structures-only pack (a Towns and Towers): its new structures,
    /// their sets and biome tags reach the generator, its land (biomes, noise, dimensions)
    /// does not.
    #[test]
    fn structures_only_pack_gives_new_structures_and_their_sets() {
        let dir =
            std::env::temp_dir().join(format!("hz-datapack-structures-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let write = |p: &str, t: &str| {
            let path = dir.join(p);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, t).unwrap();
        };
        write("data/tt/worldgen/structure/village_meadow.json", "s");
        write("data/tt/worldgen/structure_set/towns.json", "set");
        write("data/tt/worldgen/template_pool/meadow/centers.json", "pool");
        write("data/tt/worldgen/placed_feature/grass.json", "placed");
        write("data/tt/worldgen/feature/grass.json", "feature");
        write(
            "data/tt/tags/worldgen/biome/has_structure/meadow.json",
            "{\"values\":[\"minecraft:meadow\"]}",
        );
        write("data/tt/worldgen/biome/tt_meadow.json", "{}");
        write("data/minecraft/worldgen/noise/cave.json", "noise");
        write("data/minecraft/dimension/overworld.json", "{}");
        let pack = DataPack::folder("mod/tt.jar", &dir).structures_only();
        let (o, _) = worldgen_overrides(&[&pack], |_| true);
        let ids = |bundle: &str| {
            o.entries(bundle)
                .map(|(id, _)| id.to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(ids("structures"), ["tt:village_meadow"]);
        assert_eq!(ids("structure_sets"), ["tt:towns"]);
        assert_eq!(ids("template_pools"), ["tt:meadow/centers"]);
        assert_eq!(ids("placed_features"), ["tt:grass"]);
        assert_eq!(ids("features"), ["tt:grass"]);
        let tags: Vec<&str> = o.tags("biome_tags").map(|(id, _, _)| id).collect();
        assert_eq!(tags, ["tt:has_structure/meadow"]);
        assert!(ids("biomes").is_empty());
        assert!(ids("noises").is_empty());
        assert!(ids(overrides::DIMENSIONS).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
