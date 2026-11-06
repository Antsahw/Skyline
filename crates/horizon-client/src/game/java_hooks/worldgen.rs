//! Hooks of worldgen and level loading without the `java-mods` feature (bodies:
//! `java_server/blocks.rs`, `initial_spawn/java.rs`, `world_loading/java.rs`,
//! `java_worldgen.rs`): mods' chunk tickets, their generator's spawn origin, their chunk steps
//! and structures as a world loads. Horizon generates and keeps its chunks alone.
use super::super::*;

impl Game<'_> {
    /// The chunks Java mods' chunk loaders keep in `dimension`: none.
    pub(in crate::game) fn java_tickets(_dimension: &str) -> Vec<(i32, i32)> {
        Vec::new()
    }
    /// The spawn search's origin: Horizon's own.
    pub(in crate::game) fn java_spawn_origin(
        &mut self,
        origin: (i32, i32),
        _height: i32,
    ) -> Option<(i32, i32)> {
        Some(origin)
    }
    /// The Java mods' structure data under the world's packs: none.
    pub(in crate::game) fn java_structure_packs() -> Vec<horizon_client::datapack::DataPack> {
        Vec::new()
    }
    /// The streaming of `dim` when the Java mods' chunk steps generate it: never, Horizon's.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::game) fn java_streaming(
        _inner: &Arc<VanillaTerrain>,
        _res: &Resources,
        _pack: &PackResources,
        _options: &Options,
        _world_root: &std::path::Path,
        _dim: Dim,
        _seed: i64,
        _structures: bool,
        _enabled_features: &[String],
        _preset: &str,
        _biomes: &Arc<Biomes>,
        _save: &Arc<SaveDir>,
    ) -> Option<Streaming> {
        None
    }
}
