//! Java mods' biomes in the synced biome registry.
use super::*;

/// Appends the mods' biomes to registry `key`'s `entries` if it is the biome registry.
pub(super) fn push_mod_biomes(
    key: &str,
    resources: &Path,
    entries: &mut Vec<((String, String), Tag, bool)>,
) {
    // Mods' biomes after vanilla's, in crate::biome::Biomes' order.
    if key == "minecraft:worldgen/biome" {
        for (namespace, path) in crate::biome::mod_biomes(resources) {
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let Ok(value) = std::fs::read_to_string(&path)
                .map_err(|e| e.to_string())
                .and_then(|t| serde_json::from_str::<Value>(&t).map_err(|e| e.to_string()))
            else {
                continue;
            };
            entries.push(((namespace, stem.to_owned()), json_to_nbt(&value), false));
        }
    }
}
