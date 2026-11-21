//! Java mods in the hosted world: players who join before the mods' data-pack load functions
//! ran wait for them (`hold_join`).
use super::*;

impl Game<'_> {
    /// A vanilla server runs its data packs' load functions before anyone joins; the Java
    /// mods' run on the mirror's first ticks, so a player who joins before then waits (up to
    /// two minutes): Simply OneBlock's load function moves the world spawn onto its block.
    pub(super) fn hold_join(&mut self, conn: u32) -> bool {
        if self.java.held_joins.iter().any(|(c, ..)| *c == conn) {
            return true;
        }
        // With Java mods, also until the spawn's chunks are there: their tick functions may
        // build on them first (the spawn a load function moved can be anywhere).
        if super::super::java_mods::has_java_mods() {
            let loading = super::super::java_mods::java_loading();
            let lit = self.spawn_chunks_lit();
            if loading || !lit {
                self.java.held_joins.push((conn, 0, 0));
                return true;
            }
            log::info!(
                "Java mods: a player joins now (the mods' load functions ran, the spawn's chunks are there)"
            );
        }
        false
    }

    /// Whether the chunks around the spawn are loaded and lit (none: nothing to wait for).
    fn spawn_chunks_lit(&self) -> bool {
        self.spawn.is_none_or(|s| {
            let (cx, cz) = ((s.x.floor() as i32) >> 4, (s.z.floor() as i32) >> 4);
            (-1..=1).all(|dz| (-1..=1).all(|dx| self.streaming.is_lit(cx + dx, cz + dz)))
        })
    }

    /// The held players who may join now: the load functions ran (or they waited too long),
    /// and the spawn's chunks are there.
    fn release_held_joins(&mut self) -> Vec<u32> {
        // In the overworld's pass: the spawn and its chunks are its (a server ticks a pass
        // for each level).
        if self.java.held_joins.is_empty() || self.dim != super::super::Dim::Overworld {
            return Vec::new();
        }
        let loading = super::super::java_mods::java_loading();
        // The (new) spawn's chunks, as the player would load them.
        let lit = self.spawn_chunks_lit();
        if !lit && let Some(s) = self.spawn {
            let chunk = ((s.x.floor() as i32) >> 4, (s.z.floor() as i32) >> 4);
            // (A dedicated server streams no center: `lan_update_streaming` adds the spawn
            // as an area while players are held.)
            self.streaming.center = chunk;
        }
        for held in &mut self.java.held_joins {
            held.1 += 1;
            if !loading && lit {
                held.2 += 1;
            }
        }
        // A second after the load functions ran and the spawn's chunks are there: the
        // first tick functions have run on them (Simply OneBlock's places its block, as its
        // load function's setblock found the chunk unloaded), and the spawn finder sees
        // what they made.
        let waited = self.java.held_joins.iter().map(|h| h.1).max().unwrap_or(0);
        let since = self.java.held_joins.iter().map(|h| h.2).min().unwrap_or(0);
        if since < 20 && waited < 2400 {
            return Vec::new();
        }
        if waited >= 2400 {
            log::warn!(
                "a player waited two minutes for the Java mods' load functions; placed anyway"
            );
        }
        log::info!(
            "Java mods: {} held player(s) join at the spawn {:?} after the load functions",
            self.java.held_joins.len(),
            self.spawn.map(|s| [s.x, s.y, s.z])
        );
        self.java.held_joins.drain(..).map(|(c, ..)| c).collect()
    }

    /// Players held until the Java mods' load functions ran (`hold_join`) join once they have,
    /// at the spawn those functions may have moved: `events` with their joins first, and the
    /// spawn.
    pub(super) fn release_held_joins_into(
        &mut self,
        events: Vec<HostEvent>,
        spawn: [f64; 3],
    ) -> (Vec<HostEvent>, [f64; 3]) {
        let mut events = events;
        let released = self.release_held_joins();
        let spawn = if released.is_empty() {
            spawn
        } else {
            self.spawn.map_or(spawn, |s| [s.x, s.y, s.z])
        };
        events.splice(0..0, released.into_iter().map(HostEvent::Joined));
        (events, spawn)
    }

    /// Players held until the Java mods' load functions ran wait for the spawn's 3x3
    /// (`release_held_joins`): a dedicated server with nobody in loads it this way, as an
    /// extra streaming area.
    pub(super) fn held_joins_spawn_area(&self) -> Option<((i32, i32), i32)> {
        self.spawn
            .filter(|_| {
                !self.java.held_joins.is_empty() && self.dim == super::super::Dim::Overworld
            })
            .map(|s| (((s.x.floor() as i32) >> 4, (s.z.floor() as i32) >> 4), 1))
    }
}
