//! Java mods' own particle types, drawn with their sprites (game/java_entities.rs).
use super::*;

impl ParticleEngine {
    /// Whether a mod's particle type has sprites here (else it can't be drawn).
    pub fn has_mod(&self, id: &str) -> bool {
        self.mod_sets.contains_key(id)
    }
    /// A Java mod's own particle type (`ns:name`): its provider is Java's, so it is drawn as a
    /// generic `SingleQuadParticle` with the type's sprites (animated by age when it has
    /// several), moving as sent, slowing as most do. Limited like `add`.
    pub fn add_mod(&mut self, id: &str, p: [f64; 3], v: [f64; 3], override_limiter: bool) {
        let Some(&(set, _)) = self.mod_sets.get(id) else {
            return;
        };
        let c = self.camera;
        let d2 = (p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2) + (p[2] - c[2]).powi(2);
        if !override_limiter && (d2 > 1024.0 || self.calculate_particle_level(false) == 2) {
            return;
        }
        let r = &mut self.random;
        let mut q = Particle::blank(ParticleType::Poof, p);
        q.velocity = v;
        q.size = 0.1 * (r.next_float() * 0.5 + 0.5) * 2.0;
        q.lifetime = 10 + r.next_int_bound(20).unwrap_or(0);
        q.friction = 0.96;
        q.random = LegacyRandom::new(r.next_long());
        let sprites = &self.set_sprites[set as usize];
        if sprites.len() > 1 {
            q.set = Some(set);
            q.behaviour = Behaviour::Animated;
            q.sprite = Sprite::Index(sprites[0]);
        } else {
            q.sprite = Sprite::Index(sprites[0]);
        }
        *self.made.entry("mod").or_default() += 1;
        self.pending.push(q);
    }
}
