//! Java mods and the world spawn's search: a generator of their own (a mod's world type) and
//! their generator's origin.
use super::*;

impl Game<'_> {
    /// Java mods may generate the overworld with a generator of their own (a mod's world type:
    /// One Block's spawn is its block): true while that is not known yet (the search waits).
    pub(super) fn java_spawn_generator_pending(&mut self) -> bool {
        if let Some(terrain) = self.end.locator().filter(|t| !t.oracle_decided()) {
            // The first chunk made decides it: a dedicated server with nobody online
            // makes none on its own, so it asks for the origin's (else it waited forever).
            if self.standalone_server()
                && let Ok(((cx, cz), _)) = terrain.spawn_origin()
            {
                self.request_spawn_chunk(cx, cz);
            }
            return true;
        }
        false
    }

    /// When Java mods generate the overworld, their generator's origin (its climate's spawn
    /// target: a mod's noise router moves the land, and Horizon's vanilla origin can be sea
    /// there), in place of `(ox, oz)`. Asked once the first chunk is made: that chunk starts
    /// the mods' generation. None while it loads.
    #[cfg(feature = "java-mods")]
    pub(super) fn java_spawn_origin(
        &mut self,
        (mut ox, mut oz): (i32, i32),
        height: i32,
    ) -> Option<(i32, i32)> {
        if !self.java.spawn_origin_asked && super::super::java_mods::has_java_mods() {
            // A dedicated server with nobody online streams nothing around a center: the chunk
            // is asked for alone, as the spiral below asks for its chunks (else it waited
            // forever: "Preparing level" and no more).
            if self.standalone_server() {
                if !self.world.chunks.contains_key(&(ox, oz)) {
                    self.request_spawn_chunk(ox, oz);
                    return None;
                }
            } else if !self.streaming.is_lit(ox, oz) {
                self.streaming.center = (ox, oz);
                return None;
            }
            self.java.spawn_origin_asked = true;
            if let Some((x, z)) = horizon_jvm::events::worldgen_origin("minecraft:overworld")
                .ok()
                .flatten()
                .map(|o| (o.x, o.z))
                && (x, z) != (ox, oz)
            {
                log::info!(
                    "Java mods: the world spawn's search starts at chunk {x} {z} (their generator's)"
                );
                (ox, oz) = (x, z);
                self.spawn_origin = Some(((x, z), height));
                self.spawn_spiral = Default::default();
            }
        }
        Some((ox, oz))
    }

    /// A dedicated server's spawn search asks for one chunk alone (getChunk(FULL)): with
    /// nobody online it streams nothing around a center.
    fn request_spawn_chunk(&mut self, cx: i32, cz: i32) {
        let area = [((cx, cz), -1)];
        if self.streaming.extra != area {
            self.streaming.extra = area.to_vec();
            self.streaming.rank_full_task(cx, cz);
        }
    }
}
