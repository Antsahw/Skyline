//! Java mods' chunk tickets (chunk loaders): chunks kept loaded and ticking as `/forceload`ed
//! ones, but not `/forceload`'s (not saved, not listed).
use super::Streaming;
use std::collections::HashSet;

impl Streaming {
    /// The chunks Java mods' tickets keep (`horizon_jvm` `Action::ChunkTickets`).
    /// New ones are ranked for decoration and generation looks again, as `/forceload add`'s are.
    pub fn set_ticketed(&mut self, chunks: impl IntoIterator<Item = (i32, i32)>) {
        let chunks: HashSet<(i32, i32)> = chunks.into_iter().collect();
        for c in self.ticketed.difference(&chunks) {
            self.mod_forced.remove(c);
        }
        let mut new: Vec<(i32, i32)> = chunks
            .iter()
            .filter(|&&c| self.mod_forced.insert(c))
            .copied()
            .collect();
        new.sort_unstable();
        for (x, z) in new {
            self.rank_forced(x, z, x, z);
        }
        self.ticketed = chunks;
        self.generation_scan.exhausted = false;
    }
    /// What the Java mods' tickets keep (see [`Self::set_ticketed`]).
    pub fn ticketed(&self) -> &HashSet<(i32, i32)> {
        &self.ticketed
    }
    /// `/forceload`'s chunks and mods' tickets together, as the chunk tickets see them.
    pub fn kept_chunks(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.streamed_forced().copied()
    }
    /// `forced` and `mod_forced` together, once each.
    pub(super) fn streamed_forced(&self) -> impl Iterator<Item = &(i32, i32)> + '_ {
        self.forced
            .iter()
            .chain(self.mod_forced.iter().filter(|c| !self.forced.contains(c)))
    }
}
