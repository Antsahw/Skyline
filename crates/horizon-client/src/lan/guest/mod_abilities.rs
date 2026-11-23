//! Abilities a server mod (a Java mod's `Abilities`) gives a player on top of its game mode.
use super::*;

/// Abilities a server mod gave a player (vanilla's `Abilities`, as its
/// ClientboundPlayerAbilitiesPacket says), for the game mode the player was in.
#[derive(Clone, Debug, PartialEq)]
pub struct ModAbilities {
    pub game_mode: i32,
    /// invulnerable 1, flying 2, may fly 4, instabuild 8.
    pub flags: i8,
    pub flying_speed: f32,
    pub walking_speed: f32,
}

impl Guest {
    /// `Abilities.mayfly`: the game mode's, or what a server mod gave.
    pub fn may_fly(&self) -> bool {
        self.abilities.may_fly
    }
    /// `Abilities.invulnerable`.
    pub fn invulnerable(&self) -> bool {
        self.abilities.invulnerable
    }
    /// `Abilities.flyingSpeed` and `walkingSpeed`.
    pub fn speeds(&self) -> (f32, f32) {
        (self.abilities.flying_speed, self.abilities.walking_speed)
    }
}
