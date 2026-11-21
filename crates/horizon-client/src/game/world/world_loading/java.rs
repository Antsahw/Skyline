//! Java mods' worldgen as a world loads (game/java_worldgen.rs): their chunk steps for the
//! dimensions they change. Without the `java-mods` feature: `java_hooks/worldgen.rs`.
#![cfg(feature = "java-mods")]
use super::*;

impl Game<'_> {
    /// The streaming of `dim` when the Java mods' chunk steps generate it (around Horizon's
    /// generator `inner`); its settings are still to set (`finish_streaming`). None when they
    /// leave it to Horizon.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn java_streaming(
        inner: &Arc<VanillaTerrain>,
        res: &Resources,
        pack: &PackResources,
        options: &Options,
        world_root: &std::path::Path,
        dim: Dim,
        seed: i64,
        structures: bool,
        enabled_features: &[String],
        preset: &str,
        biomes: &Arc<Biomes>,
        save: &Arc<SaveDir>,
    ) -> Option<Streaming> {
        let java = super::super::java_worldgen::JavaTerrain::wrap(
            inner.clone(),
            dim.id(),
            seed,
            structures,
            world_root,
            pack.assets.clone(),
            res.biomes().clone(),
            super::super::java_worldgen::NoiseWorld {
                bundle: options.resources.join("worldgen"),
                resources: options.resources.clone(),
                world_root: world_root.to_path_buf(),
                enabled_features: enabled_features.to_vec(),
                preset: preset.to_string(),
                biomes: biomes.clone(),
                seed,
            },
        )?;
        Some(Streaming::new(
            Arc::new(Persistent {
                inner: Arc::new(java),
                save: save.clone(),
                registry: pack.assets.clone(),
                biomes: res.biomes().clone(),
            }),
            pack.context.clone(),
            res.threads(),
        ))
    }
}
