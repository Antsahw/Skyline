//! Java mods' entities among Horizon's creatures: the mods' spawns in fresh chunks, and
//! Horizon's projectiles hitting the mods' entities (the mirror takes the hurt).
#![cfg(feature = "java-mods")]
use super::*;

impl Game<'_> {
    /// The mods' creatures of a fresh chunk's biome spawn too (HorizonSpawner.generated).
    pub(super) fn java_fresh_chunk(&mut self, key: (i32, i32)) {
        if horizon_jvm::events::ready() {
            horizon_jvm::world::fresh_chunk(self.dim.id(), key.0, key.1);
        }
    }

    /// Projectile.onHitEntity on one of the mods' entities: the mirror takes the hurt. True
    /// when that ends the hit (an arrow); a snowball or a fireball goes on as on any entity.
    pub(super) fn java_projectile_hit(&mut self, p: &Projectile, hit: &Hit) -> bool {
        if let Hit::Entity {
            who: Who::Mob(id), ..
        } = *hit
            && self.is_java_neighbour(id)
        {
            let mut shot = p.clone();
            let amount = match p.kind {
                ProjectileKind::Arrow { .. } | ProjectileKind::Trident => {
                    self.shot_damage(&mut shot, None)
                }
                ProjectileKind::SmallFireball => 5.0,
                ProjectileKind::LargeFireball { .. } => 6.0,
                ProjectileKind::WitherSkull { .. } => 8.0,
                ProjectileKind::ShulkerBullet { .. } => 4.0,
                ProjectileKind::LlamaSpit => 1.0,
                ProjectileKind::Thrown(t) => t.damage,
                _ => 0.0,
            };
            let hurt = self.java_projectile_hits_entity(p, id, amount);
            if p.arrow.is_some() {
                // AbstractArrow.onHitEntity: spent (or on through, piercing) when it hurt,
                // else turned back.
                let outcome = if hurt {
                    EntityHit::Hurt
                } else {
                    EntityHit::Refused
                };
                self.finish_arrow_hit(shot, outcome);
                return true;
            }
            // The rest goes on as on any entity: a snowball breaks, a fireball explodes.
        }
        false
    }
}
