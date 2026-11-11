//! Landings Java mods judge the damage of (`PlayerMotion::route_fall`).
use super::*;

impl MotionWorld<'_, '_, '_> {
    /// The damage Java mods settled on for a [`FallEvent::Routed`] landing, dealt as
    /// `LivingEntity.causeFallDamage` deals it: the fall sounds, then the hurt.
    pub fn routed_fall_damage(
        &mut self,
        damage: f32,
        source: LandingDamage,
        contact: Option<(BlockStateId, BlockPos, f64)>,
    ) -> Result<()> {
        // A mod block's fallOn (native mods) changes what the Java mods settled on.
        let damage = contact.map_or(damage, |(state, position, distance)| {
            self.mod_block_fall(state, position, distance, damage)
        });
        if damage <= 0.0 {
            return Ok(());
        }
        let mut call = FallCall {
            world: self,
            source,
            contact: None,
        };
        call.reset_impulse()?;
        let sound = call.fall_sound(damage > 4.0)?;
        call.play_fall_sound(sound)?;
        call.play_block_fall_sound()?;
        call.hurt(damage)
    }

    /// [`Self::player_fall_damage`] when Java mods patch it (`PlayerMotion::route_fall`):
    /// the landing left as a [`FallEvent::Routed`].
    pub(super) fn route_fall(
        &mut self,
        distance: f64,
        multiplier: f32,
        source: LandingDamage,
        contact: Option<(BlockStateId, BlockPos, f64)>,
    ) -> Result<bool> {
        // Java mods patch it: they judge the damage (the game asks them after the moves). A
        // mace's or wind charge's impulse cuts the distance first, as
        // `cause_living_fall_damage` does; a mod block's own fallOn gets the damage after.
        let distance = match self.player.impulse.impact_position {
            Some(impact) => {
                let below = impact.y - self.player.position.y;
                let distance = if distance.is_nan() || below.is_nan() {
                    f64::NAN
                } else {
                    distance.min(below)
                };
                if distance <= 0.0 {
                    self.player.impulse.reset();
                } else {
                    self.player.impulse.try_reset();
                }
                distance
            }
            None => distance,
        };
        let safe = self.player.attribute("minecraft:safe_fall_distance")?;
        let damage_multiplier = self.player.attribute("minecraft:fall_damage_multiplier")?;
        self.fall_event(FallEvent::Routed {
            distance,
            multiplier,
            source,
            safe,
            damage_multiplier,
            contact,
        });
        Ok(false)
    }
}
