//! What the 1.20.1 mods draw on each player's HUD (phase 5i): AppleSkin's saturation and
//! exhaustion over the food bar, Farmer's Delight's nourishment and comfort, a mod's
//! `HudRenderCallback`.
//!
//! The runtime draws it every tick for each player (platform120's Hud120), with the player's
//! health, food, air and experience as Horizon has them, keeping only what the mods draw; it
//! goes to the player's game on the HUD channel the 26.3 bridge's HUD uses
//! (`java_screen::send_hud_part`, with that bridge's part when both run), when it changed. The
//! game draws it over its own HUD and says the HUD's size, which the next frame is laid out in.

use super::super::java_screen::{HUD_WINDOW, HudPart, forget_hud, send_hud_part};
use super::super::lan::LanHost;
use super::{Level, RUNTIME, uuid_string};
use horizon_client::command::args::GameMode;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;

/// A player's HUD, as the server knows it.
struct Hud {
    /// The GUI size their game draws the HUD at, as it last said.
    size: (i32, i32),
    /// The frame last sent: an unchanged one is not sent again.
    sent: Option<String>,
}

/// By the player's connection.
static HUDS: Mutex<Option<HashMap<u32, Hud>>> = Mutex::new(None);

fn huds<R>(f: impl FnOnce(&mut HashMap<u32, Hud>) -> R) -> R {
    let mut guard = HUDS.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

/// The GUI the HUD is drawn at before the player's game has said its own: a 1280x720 window
/// at GUI scale 3, which the game corrects once the first frame reaches it.
const FIRST_SIZE: (i32, i32) = (427, 240);

/// A player left.
pub(super) fn forget(conn: u32) {
    huds(|h| h.remove(&conn));
    forget_hud(conn);
}

/// The HUD's size from `input` (`{"window": -2, "kind": "size", "width", "height"}`), for
/// `conn`'s player: true when it was one.
pub(super) fn take_size(conn: u32, input: &Value) -> bool {
    let window = input.get("window").and_then(Value::as_i64);
    if window != Some(i64::from(HUD_WINDOW))
        || input.get("kind").and_then(Value::as_str) != Some("size")
    {
        return false;
    }
    let (Some(width), Some(height)) = (
        input.get("width").and_then(Value::as_i64),
        input.get("height").and_then(Value::as_i64),
    ) else {
        return false;
    };
    let size = (width.clamp(1, 8192) as i32, height.clamp(1, 8192) as i32);
    huds(|h| {
        let hud = h.entry(conn).or_insert(Hud { size, sent: None });
        hud.size = size;
    });
    true
}

/// What the runtime is told of a player (Hud120's `state`): Horizon's numbers.
fn state(
    dimension: &str,
    player: &mut horizon_client::motion::PlayerMotion,
    mode: GameMode,
    experience: (i32, f32),
) -> Value {
    json!({
        "dimension": dimension,
        "health": player.health.health(),
        "max_health": player.attribute("minecraft:max_health").unwrap_or(20.0),
        "absorption": player.health.absorption(),
        "food": player.health.food.food,
        "saturation": player.health.food.saturation,
        "exhaustion": player.health.food.exhaustion,
        "air": player.air,
        "xp_level": experience.0,
        "xp_progress": experience.1,
        // Gui.render: the hearts and food where the player can be hurt.
        "hurtable": matches!(mode, GameMode::Survival | GameMode::Adventure),
    })
}

impl super::super::Game<'_> {
    /// The mods' HUD of every player in the world, to their game where it changed.
    pub(in crate::game) fn tick_java120_hud(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let Some(mut host) = self.lan.take() else {
            return;
        };
        let players: Vec<(u32, String, String)> = host
            .hub
            .guests
            .iter()
            .filter(|g| g.placed)
            .map(|g| (g.conn, uuid_string(g.uuid), g.name.clone()))
            .collect();
        for (conn, uuid, name) in players {
            self.send_java120_hud(&mut host, conn, &uuid, &name);
        }
        self.lan = Some(host);
    }

    fn send_java120_hud(&mut self, host: &mut LanHost, conn: u32, uuid: &str, name: &str) {
        let size = huds(|h| h.get(&conn).map_or(FIRST_SIZE, |hud| hud.size));
        let drawn = self.as_guest(host, conn, |g| {
            let (dimension, mode, experience) = (
                g.dim.id().to_owned(),
                g.mode,
                (g.experience.level, g.experience.progress),
            );
            let player = g.player.as_mut()?;
            let state = state(&dimension, player, mode, experience).to_string();
            // Busy (a click is being run): this tick's frame is skipped, the next one comes.
            let Ok(mut guard) = RUNTIME.try_lock() else {
                return None;
            };
            let runtime = guard.as_mut()?;
            Some(runtime.hud_frame(&Level(g), uuid, name, &state, size.0, size.1))
        });
        let frame = match drawn.flatten() {
            Some(Ok(frame)) => frame,
            Some(Err(e)) => {
                failed_once(&e);
                return;
            }
            None => return,
        };
        let changed = huds(|h| {
            let hud = h.entry(conn).or_insert(Hud { size, sent: None });
            if hud.sent.as_deref() == Some(frame.as_str()) {
                return false;
            }
            hud.sent = Some(frame.clone());
            true
        });
        if changed {
            send_hud_part(host, conn, HudPart::Mods120, &frame);
        }
    }
}

/// A HUD that can't be drawn, said once: it would fail every tick.
fn failed_once(e: &str) {
    static SAID: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
        log::warn!("1.20.1 mods: their HUD can't be drawn: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hud_size_is_taken_from_its_window_only() {
        let conn = 0xF00D_0001;
        let hud = json!({"window": HUD_WINDOW, "kind": "size", "width": 320, "height": 180});
        assert!(take_size(conn, &hud));
        assert_eq!(huds(|h| h[&conn].size), (320, 180));
        let screen = json!({"window": -1, "kind": "size", "width": 100, "height": 50});
        assert!(!take_size(conn, &screen));
        assert_eq!(huds(|h| h[&conn].size), (320, 180));
        forget(conn);
        assert!(huds(|h| !h.contains_key(&conn)));
    }
}
