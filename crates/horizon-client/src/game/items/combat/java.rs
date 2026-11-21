//! What Java mods hear of the player's combat.
use super::*;

impl CombatTracker {
    /// The damage type of the last recorded hit (`getLastEntry`), for what killed the player.
    #[cfg_attr(not(feature = "java-mods"), allow(dead_code))]
    pub(in crate::game) fn last_damage_type(&self) -> Option<&str> {
        self.entries.last().map(|e| e.damage_type.as_str())
    }
}
