//! What the 1.20.1 mods' code plays and shows in the world (phases 5f, 5g): sounds
//! (`ServerLevel.playSeededSound`, a client's `playLocalSound`) and particles
//! (`ServerLevel.sendParticles`, a client's `addParticle`, from the blocks' `animateTick`). The runtime hands
//! them over as they happen, during any call ([`World120::play_sound`],
//! [`World120::send_particles`]); they wait here, and the server's next tick sends them to the
//! players near, as Horizon's own native mods' sounds and particles go.
//!
//! [`World120::play_sound`]: horizon_jvm_remote::mirror120::World120::play_sound
//! [`World120::send_particles`]: horizon_jvm_remote::mirror120::World120::send_particles

use super::super::Game;
use super::super::creatures::NativeSound;
use super::super::lan_particles::QueuedParticle;
use horizon_client::particles::ParticleOptions;
use horizon_client_core::particle_type::ParticleType;
use horizon_jvm_remote::mirror120::{PlayedSound, SentParticles};
use std::sync::Mutex;

/// At most this many wait (a mod that plays a sound every tick from many blocks shouldn't grow
/// this without bound while the server is between ticks).
const MOST: usize = 1024;

static SOUNDS: Mutex<Vec<PlayedSound>> = Mutex::new(Vec::new());
static PARTICLES: Mutex<Vec<SentParticles>> = Mutex::new(Vec::new());
/// Vanilla mobs the runtime added: (dimension, 1.20.1 SNBT).
static ENTITIES: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// A vanilla mob the runtime added, for the next tick.
pub(super) fn added(dimension: &str, snbt: &str) {
    let mut all = ENTITIES.lock().unwrap_or_else(|e| e.into_inner());
    if all.len() < MOST {
        all.push((dimension.to_owned(), snbt.to_owned()));
    }
}

/// A sound the runtime played, for the next tick.
pub(super) fn heard(sound: &PlayedSound) {
    let mut sounds = SOUNDS.lock().unwrap_or_else(|e| e.into_inner());
    if sounds.len() < MOST {
        sounds.push(sound.clone());
    }
}

/// Particles the runtime sent, for the next tick.
pub(super) fn seen(particles: &SentParticles) {
    let mut all = PARTICLES.lock().unwrap_or_else(|e| e.into_inner());
    if all.len() < MOST {
        all.push(particles.clone());
    }
}

/// Vanilla's string of particle options (`minecraft:block minecraft:stone`, `minecraft:dust
/// 1.0 0.0 0.0 1.0`) as Horizon's, and the block state of a block particle (1.20.1's name of
/// it, for the caller to make Horizon's). A mod's own type with no parameters goes by its name
/// (its sprites are in its content); one with parameters, or a vanilla type Horizon has no
/// client particle for, is `None`.
fn options(text: &str) -> Option<(ParticleOptions, Option<String>)> {
    let mut parts = text.split(' ');
    let name = parts.next()?;
    let rest: Vec<&str> = parts.collect();
    let Some(ty) = ParticleType::from_name(name) else {
        let modded = !name.starts_with("minecraft:") && rest.is_empty();
        return modded.then(|| (ParticleOptions::named(name), None));
    };
    let mut options = ParticleOptions::simple(ty);
    let mut state = None;
    match ty {
        ParticleType::Block
        | ParticleType::BlockMarker
        | ParticleType::FallingDust
        | ParticleType::DustPillar
        | ParticleType::BlockCrumble => state = Some(rest.join(" ")),
        ParticleType::Dust => {
            let f: Vec<f32> = rest.iter().filter_map(|v| v.parse().ok()).collect();
            if let [r, g, b, scale] = f[..] {
                options.color = [r, g, b, 1.0];
                options.scale = scale;
            }
        }
        _ => {}
    }
    Some((options, state))
}

/// Which blocks are picked: a random of its own, so Horizon's world random is untouched.
static PICK: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(0x9E37_79B9_7F4A_7C15);

/// A number below `bound` (`RandomSource.nextInt`'s use here).
fn below(bound: i32) -> i32 {
    use std::sync::atomic::Ordering;
    // xorshift64*.
    let mut x = PICK.load(Ordering::Relaxed);
    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    PICK.store(x, Ordering::Relaxed);
    ((x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 33) % bound as u64) as i32
}

impl Game<'_> {
    /// The server half, each tick: the mods' blocks around each player run their client code
    /// (`Block.animateTick`), picked as `ClientLevel.animateTick` picks them -- 667 random
    /// blocks within 16 of the player and 667 within 32, each `x + nextInt(r) - nextInt(r)` --
    /// so a lit stove smokes as often as it would in a 1.20.1 client.
    fn animate_java120_blocks(&mut self) {
        let Some(host) = self.lan.as_ref() else {
            return;
        };
        let players: Vec<[i32; 3]> = host
            .hub
            .guests
            .iter()
            .filter(|g| host.hub.here(g) && !g.spectator())
            .map(|g| g.position.map(|c| c.floor() as i32))
            .collect();
        if players.is_empty() {
            return;
        }
        let registry = self.pack.assets.registry;
        let mut positions = Vec::new();
        for [px, py, pz] in players {
            for _ in 0..667 {
                for r in [16, 32] {
                    let x = px + below(r) - below(r);
                    let y = py + below(r) - below(r);
                    let z = pz + below(r) - below(r);
                    if !self.world.loaded(x, z) {
                        continue;
                    }
                    let theirs = registry
                        .state(self.world.block(x, y, z))
                        .is_some_and(|s| !s.name.starts_with("minecraft:"));
                    if theirs {
                        positions.extend([x, y, z]);
                    }
                }
            }
        }
        if positions.is_empty() {
            return;
        }
        let dimension = self.dim.id();
        let Ok(mut guard) = super::RUNTIME.try_lock() else {
            return;
        };
        let Some(runtime) = guard.as_mut() else {
            return;
        };
        if let Err(e) = runtime.animate_ticks(&super::Level(self), dimension, &positions) {
            super::tick_failed_once(&e);
        }
    }

    /// The server half, each tick: what the mods' code played and showed in this dimension
    /// goes to the players near (the other dimensions' waits for theirs).
    pub(in crate::game) fn tick_java120_effects(&mut self) {
        if self.remote.is_some() {
            return;
        }
        self.animate_java120_blocks();
        let dimension = self.dim.id();
        let sounds: Vec<PlayedSound> = {
            let mut all = SOUNDS.lock().unwrap_or_else(|e| e.into_inner());
            let (here, elsewhere) = std::mem::take(&mut *all)
                .into_iter()
                .partition(|s| s.dimension == dimension);
            *all = elsewhere;
            here
        };
        for s in sounds {
            self.creatures.lan_sounds.push(NativeSound {
                dimension,
                sound: s.sound,
                at: s.pos,
                source: s.source,
                volume: s.volume,
                pitch: s.pitch,
            });
        }
        let particles: Vec<SentParticles> = {
            let mut all = PARTICLES.lock().unwrap_or_else(|e| e.into_inner());
            let (here, elsewhere) = std::mem::take(&mut *all)
                .into_iter()
                .partition(|p| p.dimension == dimension);
            *all = elsewhere;
            here
        };
        let entities: Vec<String> = {
            let mut all = ENTITIES.lock().unwrap_or_else(|e| e.into_inner());
            let (here, elsewhere): (Vec<_>, Vec<_>) = std::mem::take(&mut *all)
                .into_iter()
                .partition(|(d, _)| d == dimension);
            *all = elsewhere;
            here.into_iter().map(|(_, snbt)| snbt).collect()
        };
        for snbt in entities {
            self.load_mob_120(&snbt);
        }
        let registry = self.pack.assets.registry;
        for p in particles {
            let Some((options, state)) = options(&p.options) else {
                continue;
            };
            let state = match state {
                Some(name) => {
                    let name = super::worldgen::state_from_120(&name);
                    match horizon_world::worldgen::foreign::parse_state_string(registry, &name) {
                        Some(id) => Some(id),
                        None => continue,
                    }
                }
                None => None,
            };
            self.lan_send_particles(QueuedParticle {
                dimension,
                options,
                state,
                at: p.pos,
                spread: p.spread,
                speed: [p.speed; 3],
                count: p.count,
                randomization: 0,
            });
        }
    }

    /// A mob as 1.20.1 saved it, upgraded by vanilla's own fixes to this version and kept as
    /// Horizon's own.
    fn load_mob_120(&mut self, snbt: &str) {
        use horizon_world::datafix::{self, schema::Ty};
        let mut tag = match horizon_world::nbt::parse_snbt(snbt) {
            Ok((tag, _)) => tag,
            Err(e) => {
                log::warn!("1.20.1 mods: a mob they made was not read: {e}");
                return;
            }
        };
        datafix::update(
            Ty::EntityTree,
            &mut tag,
            super::DATA_VERSION_120,
            horizon_core::version::DATA_VERSION,
        );
        if self.load_mob(tag, None).is_none() {
            log::warn!("1.20.1 mods: a mob they made could not be kept: {snbt}");
        }
    }
}

impl Game<'_> {
    /// The 1.20.1 mods' effects due this tick on the player acting (phase 5k: `due` as the
    /// effect tick found them, `(id, ticks, amplifier)`), run by the mods' own code on the
    /// runtime's player with Horizon's numbers; what they changed (Comfort's healing,
    /// Nourishment's exhaustion taken back) comes back to Horizon's player.
    pub(in crate::game) fn java120_effect_ticks(&mut self, due: &[(&'static str, i32, i32)]) {
        let uuid = self.java120_actor();
        let name = self.acting_player_name().to_owned();
        let dimension = self.dim.id().to_owned();
        let (mode, experience) = (self.mode, (self.experience.level, self.experience.progress));
        let all: Vec<serde_json::Value> = self
            .potions
            .effects
            .list
            .iter()
            .map(|e| {
                serde_json::json!({
                    "id": horizon_client_core::potions::namespaced(e.id),
                    "duration": e.duration,
                    "amplifier": e.amplifier,
                })
            })
            .collect();
        let Some(player) = self.player.as_mut() else {
            return;
        };
        let mut state = super::hud::state(&dimension, player, mode, experience);
        state["effects"] = serde_json::Value::Array(all);
        state["tick"] = due
            .iter()
            .map(|(id, ticks, amplifier)| {
                serde_json::json!({"id": id, "ticks": ticks, "amplifier": amplifier})
            })
            .collect();
        let after = {
            let Some(mut guard) = super::wait_for_runtime() else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            runtime.effect_ticks(&super::Level(self), &uuid, &name, &state.to_string())
        };
        let after: serde_json::Value = match after.map(|a| serde_json::from_str(&a)) {
            Ok(Ok(after)) => after,
            Ok(Err(e)) => return log::warn!("1.20.1 mods: their effects' answer: {e}"),
            Err(e) => return log::warn!("1.20.1 mods: their effects failed: {e}"),
        };
        let Some(player) = self.player.as_mut() else {
            return;
        };
        let number = |key: &str| after[key].as_f64();
        if let Some(health) = number("health").map(|h| h as f32)
            && health != player.health.health()
        {
            let _ = player.set_health(health, false);
        }
        if let Some(absorption) = number("absorption").map(|a| a as f32)
            && absorption != player.health.absorption()
        {
            let _ = player.set_health(absorption, true);
        }
        let food = &mut player.health.food;
        if let Some(level) = number("food") {
            food.food = level as i32;
        }
        if let Some(saturation) = number("saturation") {
            food.saturation = saturation as f32;
        }
        if let Some(exhaustion) = number("exhaustion") {
            food.exhaustion = exhaustion as f32;
        }
        if let Some(air) = number("air") {
            player.air = air as i32;
        }
    }
}
