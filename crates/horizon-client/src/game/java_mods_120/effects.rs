//! What the 1.20.1 mods' code plays and shows in the world (phase 5f): sounds
//! (`ServerLevel.playSeededSound`) and particles (`ServerLevel.sendParticles`). The runtime hands
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

impl Game<'_> {
    /// The server half, each tick: what the mods' code played and showed in this dimension
    /// goes to the players near (the other dimensions' waits for theirs).
    pub(in crate::game) fn tick_java120_effects(&mut self) {
        if self.remote.is_some() {
            return;
        }
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
