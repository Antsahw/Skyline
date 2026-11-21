//! Java mods' patches of `PlayerAdvancements.award`.
use super::*;

impl Game<'_> {
    /// Whether the Java mods refuse `criterion` of `a` (their patches of
    /// `PlayerAdvancements.award`, run on the mirror with the progress so far).
    pub(super) fn java_award_refused_in(&mut self, a: &adv::Advancement, criterion: &str) -> bool {
        let done: Vec<&str> = a
            .criteria
            .iter()
            .map(|c| c.name.as_str())
            .filter(|c| self.adv.progress.has(&a.id, c))
            .collect();
        self.java_award_refused(&a.id, criterion, &done)
    }
}
