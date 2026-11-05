//! Java mods and the trial spawner's omen: their heads of `addEffect` may refuse the Trial
//! Omen a Bad Omen becomes.
use super::*;

/// The Trial Omen a Bad Omen of `amplifier` becomes: 15 minutes per level.
fn trial_omen(amplifier: i32) -> horizon_client::effects::EffectInstance {
    horizon_client::effects::EffectInstance::new(
        "trial_omen",
        TRIAL_OMEN_TICKS_PER_LEVEL * (amplifier + 1),
        0,
    )
}

impl Game<'_> {
    /// `transformBadOmenIntoTrialOmen`'s player `uuid`: whether it is the one in the game's
    /// frame (else a guest), and whether the Java mods refuse the Trial Omen it would get.
    /// None when it has no Bad Omen.
    pub(super) fn java_trial_omen(&mut self, uuid: Uuid) -> Option<(bool, bool)> {
        // A guest of that UUID (not the one acting) first: a server thread's own acting_uuid
        // falls back to the world owner's, who is a guest there.
        let acting = self.lan_acting;
        let guest = self.lan.as_ref().and_then(|h| {
            h.hub.guests.iter().find(|g| {
                horizon_client::lan::playerdata::uuid_ints(g.uuid) == uuid && Some(g.conn) != acting
            })
        });
        let mine = guest.is_none() && self.acting_uuid() == uuid;
        // The Trial Omen it would get: the Java mods' heads of addEffect may refuse it (the
        // Bad Omen goes all the same, as vanilla removes it first).
        let amplifier = if mine {
            self.potions.effects.amplifier("bad_omen")
        } else {
            guest.and_then(|g| g.effects.amplifier("bad_omen"))
        }?;
        let player = horizon_client::lan::playerdata::ints_uuid(uuid);
        let refused = self.java_add_effect_to(Some(player), &trial_omen(amplifier));
        Some((mine, refused))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use horizon_client::effects::{EffectInstance, PlayerEffects};

    /// A mod refusing the Trial Omen (`addEffect`'s head) still loses the Bad Omen, as
    /// vanilla's `removeEffect` comes first; nothing is new.
    #[test]
    fn a_refused_trial_omen_still_takes_the_bad_omen() {
        let mut e = PlayerEffects::default();
        e.add(EffectInstance::new("bad_omen", 100, 2));
        assert!(!transform_bad_omen(&mut e, false));
        assert!(e.list.is_empty());
    }
}
