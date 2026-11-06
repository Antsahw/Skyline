//! Mods' fluids' sprites for the Java mod bridge's fluids (mesher/mod_blocks.rs).
use crate::Result;
use horizon_world::registry::{BlockRegistry, BlockStateId};
use std::path::Path;

/// Mods' fluids' sprites, which no block model names either: a mod block's
/// `<ns>:block/<name>_still` and `_flow` (or under `fluid/`) where the files are there
/// (Create's honey and chocolate). The Java mods' models say which a fluid draws with
/// (mesher::set_mod_fluids); a sprite not loaded here leaves it drawn as lava. None when
/// nothing is drawn (`draws` false: a server).
pub(super) fn insert_mod_fluid_sprites(
    root: &Path,
    registry: &BlockRegistry,
    sprites: &mut crate::sprites::Sprites,
    draws: bool,
) -> Result<()> {
    if !draws {
        return Ok(());
    }
    let mut names = std::collections::BTreeSet::new();
    for id in 0..registry.len() as u32 {
        if let Some(state) = registry.state(BlockStateId(id))
            && !state.name.starts_with("minecraft:")
        {
            names.insert(state.name.as_str());
        }
    }
    for name in names {
        let Some((ns, path)) = name.split_once(':') else {
            continue;
        };
        for folder in ["block", "fluid"] {
            for suffix in ["still", "flow"] {
                let file = root.join(format!("assets/{ns}/textures/{folder}/{path}_{suffix}.png"));
                if file.is_file() {
                    sprites.insert(root, &format!("{ns}:{folder}/{path}_{suffix}"))?;
                }
            }
        }
    }
    Ok(())
}
