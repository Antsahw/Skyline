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
    /// The mod-given abilities, while the player is still in the game mode they were given in
    /// (`GameType.updatePlayerAbilities` resets them on a change; the mod sends them again).
    fn granted(&self) -> Option<&ModAbilities> {
        self.mod_abilities
            .as_ref()
            .filter(|a| a.game_mode == self.game_mode)
    }
    /// `Abilities.mayfly`.
    pub fn may_fly(&self) -> bool {
        self.granted()
            .map_or(self.creative() || self.spectator(), |a| a.flags & 4 != 0)
    }
    /// `Abilities.invulnerable`.
    pub fn invulnerable(&self) -> bool {
        self.granted()
            .map_or(self.creative() || self.spectator(), |a| a.flags & 1 != 0)
    }
    /// `Abilities.flyingSpeed` and `walkingSpeed`.
    pub fn speeds(&self) -> (f32, f32) {
        self.granted().map_or((self.flying_speed, 0.1), |a| {
            (a.flying_speed, a.walking_speed)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guest() -> Guest {
        let (tx, _rx) = std::sync::mpsc::channel();
        Guest::new(
            Joined {
                conn: 2,
                uuid: 2,
                name: "Alex".into(),
                properties: Vec::new(),
                info: horizon_protocol::packets::configuration::ClientInformation::new(8),
                brand: "test".into(),
                sender: PlayerSender::new(tx, Default::default()),
                latency: Default::default(),
                address: "127.0.0.1:25565".parse().unwrap(),
                expected: false,
            },
            2,
            [0.0, 64.0, 0.0],
            8,
        )
    }

    #[test]
    fn mod_abilities_hold_in_their_game_mode_only() {
        let mut g = guest();
        assert!(!g.may_fly());
        assert_eq!(g.ability_flags(), 0);
        // A mod's /fly in survival.
        g.mod_abilities = Some(ModAbilities {
            game_mode: SURVIVAL,
            flags: 4,
            flying_speed: 0.1,
            walking_speed: 0.1,
        });
        assert!(g.may_fly() && !g.invulnerable());
        assert_eq!(g.speeds(), (0.1, 0.1));
        g.flying = true;
        assert_eq!(g.ability_flags(), 2 | 4);
        // Creative: the game mode's abilities until the mod speaks again.
        g.game_mode = CREATIVE;
        assert_eq!(g.ability_flags(), 1 | 2 | 4 | 8);
        assert_eq!(g.speeds(), (0.05, 0.1));
        // Survival without the mod's flight: no flying.
        g.game_mode = SURVIVAL;
        g.mod_abilities = None;
        assert_eq!(g.ability_flags(), 0);
    }
}
