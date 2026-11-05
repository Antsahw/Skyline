//! Java mods' worldgen in the commands' registries: their biomes and structures, for
//! /locate and the biome and structure arguments.
use super::*;
use horizon_client::biome::Biomes;

/// Mods' biomes (Java mods' worldgen), as Horizon's biome table has them.
pub(super) fn add_mod_biomes(reg: &mut Registries, biomes: &Biomes) {
    reg.biomes.extend(
        biomes
            .list
            .iter()
            .filter(|b| !b.name.starts_with("minecraft:"))
            .map(|b| b.name.clone()),
    );
    reg.biomes.sort();
    reg.biomes.dedup();
}

/// Mods' structures and biomes in the registries vanilla's export filled (content root
/// `root`).
pub(super) fn add_mod_structures(reg: &mut Registries, root: &std::path::Path) {
    // Mods' structures and structure tags (data/<ns>/worldgen/structure and
    // tags/worldgen/structure of the content root), for /locate structure.
    let (structures, structure_tags) = mod_structures(root);
    for id in &structures {
        if !reg.structures.contains(id) {
            reg.structures.push(id.clone());
        }
    }
    reg.structures.sort();
    if let Some(entry) = reg.by_registry.get_mut("minecraft:worldgen/structure") {
        for id in structures {
            if !entry.ids.contains(&id) {
                entry.ids.push(id);
            }
        }
        for tag in structure_tags {
            if !entry.tags.contains(&tag) {
                entry.tags.push(tag);
            }
        }
        entry.ids.sort();
        entry.tags.sort();
    }
    // The export is vanilla's: mods' biomes join its biome ids (Java mods' worldgen).
    if let Some(biomes) = reg.by_registry.get_mut("minecraft:worldgen/biome") {
        for name in &reg.biomes {
            if !biomes.ids.contains(name) {
                biomes.ids.push(name.clone());
            }
        }
        biomes.ids.sort();
    }
}

/// The structure ids and structure tag ids of every namespace but vanilla's in the content
/// root `root` (`data/<ns>/worldgen/structure/**.json`, `data/<ns>/tags/worldgen/structure/**.json`).
fn mod_structures(root: &std::path::Path) -> (Vec<String>, Vec<String>) {
    let walk = |base: &std::path::Path, ns: &str, out: &mut Vec<String>| {
        let mut stack = vec![base.to_owned()];
        while let Some(d) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&d) else {
                continue;
            };
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    stack.push(p);
                } else if p.extension().and_then(|x| x.to_str()) == Some("json")
                    && let Ok(rel) = p.strip_prefix(base)
                {
                    out.push(format!(
                        "{ns}:{}",
                        rel.with_extension("").to_string_lossy().replace('\\', "/")
                    ));
                }
            }
        }
    };
    let (mut ids, mut tags) = (Vec::new(), Vec::new());
    let Ok(namespaces) = std::fs::read_dir(root.join("data")) else {
        return (ids, tags);
    };
    for ns in namespaces.flatten() {
        let name = ns.file_name().to_string_lossy().into_owned();
        if name == "minecraft" {
            continue;
        }
        walk(&ns.path().join("worldgen/structure"), &name, &mut ids);
        walk(&ns.path().join("tags/worldgen/structure"), &name, &mut tags);
    }
    (ids, tags)
}
