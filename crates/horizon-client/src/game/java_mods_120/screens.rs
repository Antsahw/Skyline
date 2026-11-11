//! A 1.20.1 mod's screen on a player's game (phase 4): a click that opened a mod's menu (a
//! Farmer's Delight cooking pot) shows the mod's own screen there, and what the player does on
//! it goes back to it.
//!
//! The screen is drawn in the runtime, by the mod's own screen class, into operations
//! (platform120's Graphics120) of the same kind the 26.3 bridge sends, so the player's game
//! shows it with the same code (game/java_screen.rs): a client-only screen
//! ([`CLIENT_WINDOW`]) on [`CHANNEL`]. Its input comes back on [`INPUT`] and runs as the
//! screen's own handlers take it; a slot click lands on the server's menu, which in the
//! runtime is the very menu the screen draws. The player's 36 inventory slots go to the
//! runtime with every frame and input, and the ones an input changed come back.
//!
//! Frames go out every tick while a screen is open, and only when they changed.

use super::super::java_screen::{CHANNEL, CLIENT_WINDOW};
use super::super::lan::LanHost;
use super::{Level, RUNTIME, stack_from_120, tracing};
use horizon_protocol::packets::play::Clientbound as C;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::sync::Mutex;

/// A 1.20.1 mod's screen open on a player's game.
struct Open {
    /// The player, as the runtime knows them (`UUID.toString()`).
    uuid: String,
    /// The GUI size their game lays the screen out in, as it last said.
    size: (i32, i32),
    /// Where their mouse is, in that GUI.
    mouse: (i32, i32),
    /// The mouse button held down (GLFW's), for a move to be a drag.
    held: Option<i32>,
    /// The frame last sent: an unchanged one is not sent again.
    sent: Option<String>,
}

/// The screens open now, by the player's connection.
static OPEN: Mutex<Option<HashMap<u32, Open>>> = Mutex::new(None);

fn open<R>(f: impl FnOnce(&mut HashMap<u32, Open>) -> R) -> R {
    let mut guard = OPEN.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(HashMap::new))
}

/// The GUI a screen is laid out in before the player's game has said its own: a 1280x720
/// window at GUI scale 3, which the next frame corrects.
const FIRST_SIZE: (i32, i32) = (427, 240);

/// GLFW's modifier bits (1 shift, 2 control, 4 alt) from the SDL ones the game sends.
fn glfw_mods(sdl: i64) -> i32 {
    let mut mods = 0;
    if sdl & 0x3 != 0 {
        mods |= 1;
    }
    if sdl & 0xC0 != 0 {
        mods |= 2;
    }
    if sdl & 0x300 != 0 {
        mods |= 4;
    }
    mods
}

/// GLFW's mouse button (0 left, 1 right, 2 middle) from the game's (1 left, 3 right, 2 middle).
fn glfw_button(button: i64) -> i32 {
    match button {
        3 => 1,
        2 => 2,
        _ => 0,
    }
}

/// GLFW's key code, which 1.20.1 screens read, from the SDL scancode the game sends (26.3's).
/// -1 (GLFW's unknown key) for one with no GLFW code.
fn glfw_key(sdl: i64) -> i32 {
    let sdl = sdl as i32;
    match sdl {
        4..=29 => 65 + (sdl - 4),    // A..Z
        30..=38 => 49 + (sdl - 30),  // 1..9
        39 => 48,                    // 0
        40 => 257,                   // Enter
        41 => 256,                   // Escape
        42 => 259,                   // Backspace
        43 => 258,                   // Tab
        44 => 32,                    // Space
        45 => 45,                    // -
        46 => 61,                    // =
        47 => 91,                    // [
        48 => 93,                    // ]
        49 => 92,                    // backslash
        51 => 59,                    // ;
        52 => 39,                    // '
        53 => 96,                    // `
        54 => 44,                    // ,
        55 => 46,                    // .
        56 => 47,                    // /
        57 => 280,                   // Caps Lock
        58..=69 => 290 + (sdl - 58), // F1..F12
        73 => 260,                   // Insert
        74 => 268,                   // Home
        75 => 266,                   // Page Up
        76 => 261,                   // Delete
        77 => 269,                   // End
        78 => 267,                   // Page Down
        79 => 262,                   // Right
        80 => 263,                   // Left
        81 => 264,                   // Down
        82 => 265,                   // Up
        83 => 282,                   // Num Lock
        84 => 331,                   // keypad /
        85 => 332,                   // keypad *
        86 => 333,                   // keypad -
        87 => 334,                   // keypad +
        88 => 335,                   // keypad Enter
        89..=97 => 321 + (sdl - 89), // keypad 1..9
        98 => 320,                   // keypad 0
        99 => 330,                   // keypad .
        _ => -1,
    }
}

/// The game's input for its client-only screen as the runtime's screen takes it (see
/// `Runtime120::screen_input`), with what the screen keeps between inputs (where the mouse is,
/// which button is down) brought up to date. None for input that needs no call (the screen's
/// size, a move with no button down: the next frame carries those).
fn for_runtime(input: &Value, screen: &mut Open) -> Option<Value> {
    let num = |key: &str| input.get(key).and_then(Value::as_f64).unwrap_or(0.0);
    let kind = input
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let mods = glfw_mods(input.get("mods").and_then(Value::as_i64).unwrap_or(0));
    match kind {
        "size" => {
            let (w, h) = (num("width") as i32, num("height") as i32);
            if w > 0 && h > 0 {
                screen.size = (w, h);
            }
            None
        }
        "move" | "drag" => {
            let was = screen.mouse;
            screen.mouse = (num("x") as i32, num("y") as i32);
            let button = screen.held?;
            Some(
                json!({"kind": "drag", "x": num("x"), "y": num("y"), "button": button,
                        "dx": f64::from(screen.mouse.0 - was.0), "dy": f64::from(screen.mouse.1 - was.1),
                        "mods": mods}),
            )
        }
        "press" | "release" => {
            let button = glfw_button(input.get("button").and_then(Value::as_i64).unwrap_or(1));
            screen.mouse = (num("x") as i32, num("y") as i32);
            screen.held = (kind == "press").then_some(button);
            Some(
                json!({"kind": kind, "x": num("x"), "y": num("y"), "button": button, "mods": mods}),
            )
        }
        "scroll" => Some(json!({"kind": "scroll", "x": num("x"), "y": num("y"), "v": num("v")})),
        "key" => {
            let scancode = input.get("key").and_then(Value::as_i64).unwrap_or(0);
            Some(
                json!({"kind": "key", "key": glfw_key(scancode), "scancode": scancode, "mods": mods}),
            )
        }
        "char" => Some(json!({"kind": "char", "char": num("char"), "mods": mods})),
        "close" => Some(json!({"kind": "close"})),
        _ => None,
    }
}

/// Said once: a screen that failed to draw or to take input. It closes, and a mod whose screen
/// fails once fails every time.
fn failed_once(what: &str, e: &str) {
    static SAID: Mutex<Option<std::collections::HashSet<String>>> = Mutex::new(None);
    let mut said = SAID.lock().unwrap_or_else(|e| e.into_inner());
    if said
        .get_or_insert_with(Default::default)
        .insert(what.to_owned())
    {
        log::warn!("1.20.1 mods: a mod's screen {what} failed, and closes: {e}");
    }
}

/// A player's main inventory as the runtime takes it.
fn inventory_json(inventory: &[Option<horizon_client::items::ItemStack>]) -> String {
    let slots: Vec<Option<(&str, i32, i32)>> = inventory
        .iter()
        .take(36)
        .map(|slot| {
            slot.as_ref()
                .map(|s| (&*s.item, s.count as i32, s.damage as i32))
        })
        .collect();
    horizon_jvm_remote::mirror120::inventory_json(&slots)
}

fn send(host: &LanHost, conn: u32, frame: String) {
    host.hub.send_to(
        conn,
        &C::CustomPayload(horizon_protocol::packets::configuration::CustomPayload {
            channel: CHANNEL.into(),
            data: frame.into_bytes(),
        }),
    );
}

/// Tells the player's game its screen closed.
fn send_close(host: &LanHost, conn: u32) {
    send(
        host,
        conn,
        json!({"window": CLIENT_WINDOW, "close": true}).to_string(),
    );
}

impl super::super::Game<'_> {
    /// The click the player acting now made opened a mod's menu: its screen opens on their
    /// game with the next tick's frame.
    pub(in crate::game) fn java120_opened(&mut self, uuid: &str, kind: &str) {
        let Some(conn) = self.lan_acting else {
            return;
        };
        trace120!("{kind} opened for {uuid}: its screen goes to connection {conn}");
        open(|screens| {
            let size = screens.get(&conn).map_or(FIRST_SIZE, |s| s.size);
            screens.insert(
                conn,
                Open {
                    uuid: uuid.to_owned(),
                    size,
                    mouse: (-1, -1),
                    held: None,
                    sent: None,
                },
            )
        });
    }

    /// A frame of every open mod screen, to its player's game, where it changed.
    pub(in crate::game) fn tick_java120_screens(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let conns: Vec<u32> = open(|screens| screens.keys().copied().collect());
        if conns.is_empty() {
            return;
        }
        let Some(mut host) = self.lan.take() else {
            return;
        };
        for conn in conns {
            self.send_java120_frame(&mut host, conn);
        }
        self.lan = Some(host);
    }

    /// One frame of `conn`'s open screen, drawn now and sent if it changed; a screen whose
    /// menu closed (walked away from, its block broken) closes on their game.
    fn send_java120_frame(&mut self, host: &mut LanHost, conn: u32) {
        let Some((uuid, size, mouse)) = open(|screens| {
            screens
                .get(&conn)
                .map(|s| (s.uuid.clone(), s.size, s.mouse))
        }) else {
            return;
        };
        let drawn = self.as_guest(host, conn, |g| {
            let inventory = inventory_json(&g.inventory);
            let at = g
                .player
                .as_ref()
                .map_or([0.0; 3], |p| [p.position.x, p.position.y, p.position.z]);
            // Busy (a click is being run): this tick's frame is skipped, the next one comes.
            let Ok(mut guard) = RUNTIME.try_lock() else {
                return None;
            };
            let runtime = guard.as_mut()?;
            Some(runtime.screen_frame(
                &Level(g),
                &uuid,
                &inventory,
                at,
                size.0,
                size.1,
                mouse.0,
                mouse.1,
            ))
        });
        let frame = match drawn {
            // The player left.
            None => {
                open(|screens| screens.remove(&conn));
                return;
            }
            Some(None) => return,
            Some(Some(Ok(frame))) => frame,
            Some(Some(Err(e))) => {
                failed_once("drawing", &e);
                None
            }
        };
        let Some(frame) = frame else {
            trace120!("the screen of connection {conn} closed");
            open(|screens| screens.remove(&conn));
            send_close(host, conn);
            return;
        };
        let changed = open(|screens| {
            let Some(screen) = screens.get_mut(&conn) else {
                return false;
            };
            if screen.sent.as_deref() == Some(frame.as_str()) {
                return false;
            }
            screen.sent = Some(frame.clone());
            true
        });
        if changed {
            send(
                host,
                conn,
                format!(r#"{{"window":{CLIENT_WINDOW},"ops":{frame}}}"#),
            );
        }
    }

    /// Input from a player's game for its client-only screen, when that screen is a 1.20.1
    /// mod's: run by the screen, and its effects (a stack moved into the pot, out of the
    /// inventory) landed here. False when the player has no such screen open, for the 26.3
    /// bridge to take it.
    pub(in crate::game) fn java120_screen_input(
        &mut self,
        host: &mut LanHost,
        conn: u32,
        data: &[u8],
    ) -> bool {
        let Ok(input) = serde_json::from_slice::<Value>(data) else {
            return false;
        };
        let window = input.get("window").and_then(Value::as_i64).unwrap_or(-1) as i32;
        if window != CLIENT_WINDOW {
            return false;
        }
        let Some((uuid, call)) = open(|screens| {
            let screen = screens.get_mut(&conn)?;
            Some((screen.uuid.clone(), for_runtime(&input, screen)))
        }) else {
            return false;
        };
        let Some(call) = call else {
            // The size or the mouse: the next frame shows them. A new size now, so the screen
            // isn't drawn at the wrong one for a tick.
            if input.get("kind").and_then(Value::as_str) == Some("size") {
                self.send_java120_frame(host, conn);
            }
            return true;
        };
        let call = call.to_string();
        let done = self.as_guest(host, conn, |g| {
            let inventory = inventory_json(&g.inventory);
            let done = {
                let mut guard = super::wait_for_runtime()?;
                let runtime = guard.as_mut()?;
                runtime.screen_input(&Level(g), &uuid, &inventory, &call)
            };
            let done = match done {
                Ok(done) => done,
                Err(e) => {
                    failed_once("taking input", &e);
                    return Some(false);
                }
            };
            let dimension = g.dim.id().to_owned();
            g.apply_java120(&dimension, &done.set, &done.block_entities, &done.spawned);
            for (slot, stack) in &done.slots {
                let Some(place) = g.inventory.get_mut(*slot) else {
                    continue;
                };
                match stack {
                    None => *place = None,
                    // A stack that does not read stays as Horizon had it, rather than vanish.
                    Some(snbt) => {
                        if let Some(stack) = stack_from_120(snbt) {
                            *place = Some(stack);
                        }
                    }
                }
            }
            Some(done.open)
        });
        match done.flatten() {
            Some(true) => self.send_java120_frame(host, conn),
            Some(false) => {
                open(|screens| screens.remove(&conn));
                send_close(host, conn);
            }
            // The runtime was busy: the input is lost, as a dropped packet would be.
            None => {}
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_reach_a_1_20_1_screen_as_glfw_codes() {
        // GLFW's codes, from glfw3.h.
        assert_eq!(glfw_key(41), 256, "Escape");
        assert_eq!(glfw_key(8), 69, "E, the inventory key");
        assert_eq!(glfw_key(30), 49, "1, the first hotbar key");
        assert_eq!(glfw_key(39), 48, "0");
        assert_eq!(glfw_key(42), 259, "Backspace");
        assert_eq!(glfw_key(58), 290, "F1");
        assert_eq!(glfw_key(82), 265, "Up");
        assert_eq!(glfw_key(98), 320, "keypad 0");
    }

    #[test]
    fn a_move_with_a_button_down_is_a_drag() {
        let mut screen = Open {
            uuid: String::new(),
            size: FIRST_SIZE,
            mouse: (-1, -1),
            held: None,
            sent: None,
        };
        let press = for_runtime(
            &json!({"kind": "press", "x": 10, "y": 20, "button": 3, "mods": 3}),
            &mut screen,
        )
        .expect("a press is the screen's");
        assert_eq!(press["button"], 1, "the right button is GLFW's 1");
        assert_eq!(press["mods"], 1, "either shift is GLFW's shift");
        let drag = for_runtime(&json!({"kind": "move", "x": 14, "y": 23}), &mut screen)
            .expect("a move with the button down");
        assert_eq!(
            (&drag["kind"], &drag["dx"], &drag["dy"]),
            (&json!("drag"), &json!(4.0), &json!(3.0))
        );
        for_runtime(
            &json!({"kind": "release", "x": 14, "y": 23, "button": 3}),
            &mut screen,
        );
        assert!(
            for_runtime(&json!({"kind": "move", "x": 1, "y": 1}), &mut screen).is_none(),
            "a move with no button down is only where the mouse is"
        );
        assert_eq!(screen.mouse, (1, 1));
    }
}
