//! Java mods' chunk tickets (chunk loaders): chunks kept loaded and ticking as `/forceload`ed
//! ones, but not `/forceload`'s (not saved, not listed).
use super::Streaming;
use std::collections::HashSet;

/// The chunks Java mods' tickets keep, and `forced` with them for the ranges.
#[derive(Default)]
pub(super) struct Tickets {
    /// Chunks Java mods' chunk tickets keep. Set with [`Streaming::set_ticketed`].
    ticketed: HashSet<(i32, i32)>,
    /// `forced` and `ticketed` together, while `ticketed` has any.
    kept: HashSet<(i32, i32)>,
}

impl Tickets {
    /// The forced chunks the ranges use: `forced`, or `kept` while mods' tickets add to them.
    pub(super) fn or<'a>(&'a self, forced: &'a HashSet<(i32, i32)>) -> &'a HashSet<(i32, i32)> {
        if self.ticketed.is_empty() {
            forced
        } else {
            &self.kept
        }
    }
    /// How many chunks mods' tickets keep.
    pub(super) fn len(&self) -> usize {
        self.ticketed.len()
    }
    /// `forced` changes outside Streaming (`/forceload`): the union follows each update.
    pub(super) fn refresh(&mut self, forced: &HashSet<(i32, i32)>) {
        if !self.ticketed.is_empty() {
            self.refresh_kept(forced);
        }
    }
    fn refresh_kept(&mut self, forced: &HashSet<(i32, i32)>) {
        self.kept.clear();
        if !self.ticketed.is_empty() {
            self.kept.extend(forced.iter().copied());
            self.kept.extend(self.ticketed.iter().copied());
        }
    }
}

impl Streaming {
    /// The chunks Java mods' tickets keep (`horizon_jvm` `Action::ChunkTickets`).
    /// New ones are ranked for decoration and generation looks again, as `/forceload add`'s are.
    pub fn set_ticketed(&mut self, chunks: impl IntoIterator<Item = (i32, i32)>) {
        let chunks: HashSet<(i32, i32)> = chunks.into_iter().collect();
        let mut new: Vec<(i32, i32)> = chunks
            .iter()
            .filter(|c| !self.tickets.ticketed.contains(c))
            .copied()
            .collect();
        new.sort_unstable();
        for (x, z) in new {
            self.rank_forced(x, z, x, z);
        }
        self.tickets.ticketed = chunks;
        self.tickets.refresh_kept(&self.forced);
        self.generation_scan.exhausted = false;
    }
    /// What mods' tickets keep (see [`Self::set_ticketed`]).
    pub fn ticketed(&self) -> &HashSet<(i32, i32)> {
        &self.tickets.ticketed
    }
    /// The forced chunks the ranges use (`kept` while mods' tickets add to them).
    pub(super) fn kept_or_forced(&self) -> impl Iterator<Item = &(i32, i32)> + '_ {
        self.tickets.or(&self.forced).iter()
    }
    /// `forced` and `ticketed` together, as the chunk tickets see them.
    pub fn kept_chunks(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        self.forced
            .iter()
            .chain(
                self.tickets
                    .ticketed
                    .iter()
                    .filter(|c| !self.forced.contains(c)),
            )
            .copied()
    }
}
