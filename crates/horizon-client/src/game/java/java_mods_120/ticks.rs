//! A server tick, as the 1.20.1 mods hear it (phase 5): Fabric API's `ServerTickEvents` (and,
//! once, the server starting and its levels loading), fired in the runtime each time Horizon's
//! server ticks a level. A mod set nobody of which listens to ticks costs a call every five
//! seconds, to notice a listener registered later.

use super::{Level, runtime};
use std::sync::atomic::{AtomicI64, Ordering};

/// The game time at which to ask again, when the last answer was that nobody listens.
static QUIET_UNTIL: [AtomicI64; 2] = [AtomicI64::new(i64::MIN), AtomicI64::new(i64::MIN)];

/// How long a mod set with no tick listeners is left alone (5 seconds).
const QUIET_TICKS: i64 = 100;

impl super::super::Game<'_> {
    /// One tick of the level running now, for the 1.20.1 mods' tick events. A runtime busy
    /// with something else misses this one (a tick is every 50 ms; nobody waits on it).
    pub(in crate::game) fn tick_java120_events(&mut self) {
        if self.remote.is_some() {
            return;
        }
        for loader in super::loaders() {
            let _route = super::route(loader);
            self.tick_java120_events_of(&QUIET_UNTIL[loader.index()]);
        }
    }

    /// The tick in the runtime calls go to now; `quiet` is till when its mods don't listen.
    fn tick_java120_events_of(&mut self, quiet: &AtomicI64) {
        let now = self.world.game_time;
        if now < quiet.load(Ordering::Relaxed) {
            return;
        }
        let dimension = self.dim.id().to_owned();
        let ticked = {
            let Ok(mut guard) = runtime().try_lock() else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            runtime.server_tick(&Level(self), &dimension, now)
        };
        match ticked {
            Ok(ticked) => {
                if !ticked.listening {
                    quiet.store(now + QUIET_TICKS, Ordering::Relaxed);
                }
                self.apply_java120(
                    &dimension,
                    &ticked.set,
                    &ticked.block_entities,
                    &ticked.spawned,
                );
            }
            Err(e) => {
                log::warn!("1.20.1 mods: a server tick failed: {e}");
                quiet.store(now + QUIET_TICKS, Ordering::Relaxed);
            }
        }
    }
}
