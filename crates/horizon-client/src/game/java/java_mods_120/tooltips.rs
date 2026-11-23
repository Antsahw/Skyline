//! What the 1.20.1 mods add to item tooltips (phase 5n): a mod item's own lines
//! (`Item.appendHoverText`: Farmer's Delight's meals list their effects) and what tooltip mods
//! append to any stack (Fabric's `ItemTooltipCallback`). Horizon builds the rest of the tooltip
//! itself.
//!
//! The runtime is the server's, and a tooltip shows in a player's game, which may be another
//! machine. So the server tells each player's game once that it answers tooltips ([`CHANNEL`]);
//! the game asks for a stack the first time its tooltip shows (an input on the Java screens'
//! channel, `kind: tooltip`), and again a second later while it's still shown (a battery's
//! charge changes); the server answers from the runtime, and the game keeps the answer per
//! stack and puts its lines in where 1.20.1's `getTooltipLines` does: a mod item's own under
//! the name, the tooltip mods' at the end.

use super::super::Game;
use super::super::java_screen::INPUT;
use super::breaking::stack_to_120;
use super::{runtime, wait_for_runtime};
use horizon_client::items::ItemStack;
use horizon_client::text::Text;
use horizon_protocol::packets::configuration::CustomPayload;
use horizon_protocol::packets::play::{Clientbound as C, Serverbound as S};
use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Server to game: `{"on": true}` once, then `{"stack", "advanced", "hover", "after"}` for each
/// stack asked about (the lines as JSON components).
pub(in crate::game) const CHANNEL: &str = "horizon:java120_tooltip";

/// How long an answer stands before the game asks again, while the tooltip still shows.
const FRESH: Duration = Duration::from_secs(1);

/// The game: its server answers tooltips.
static ON: AtomicBool = AtomicBool::new(false);

/// What a stack's tooltip gets from the mods, as last answered.
struct Answer {
    at: Instant,
    hover: Vec<Text>,
    after: Vec<Text>,
    /// A mod item's tooltip image: its size and what it draws at the origin.
    image: Option<Image>,
}

/// A mod item's tooltip image (`getTooltipImage`): drawn under the lines, as 1.20.1 draws a
/// `ClientTooltipComponent`.
#[derive(Clone)]
pub(in crate::game) struct Image {
    pub size: [f32; 2],
    pub ops: Vec<Value>,
}

fn image(value: &Value) -> Option<Image> {
    let (w, h) = (value["w"].as_f64()?, value["h"].as_f64()?);
    let ops = value["ops"].as_array()?.clone();
    (w > 0.0 && h > 0.0).then_some(Image {
        size: [w as f32, h as f32],
        ops,
    })
}

/// `ops` moved by `origin`: their GUI pose's translation, or one where they have none.
pub(in crate::game) fn moved(ops: &[Value], origin: [f32; 2]) -> Vec<Value> {
    ops.iter()
        .map(|o| {
            let mut o = o.clone();
            let mut pose = match o.get("pose").and_then(Value::as_array) {
                Some(p) if p.len() == 6 => p.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect(),
                _ => vec![1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            };
            pose[4] += f64::from(origin[0]);
            pose[5] += f64::from(origin[1]);
            if let Some(map) = o.as_object_mut() {
                map.insert("pose".into(), json!(pose));
            }
            o
        })
        .collect()
}

#[derive(Default)]
struct Client {
    /// By the stack as 1.20.1 saves it, and the advanced flag.
    answers: HashMap<(String, bool), Answer>,
    /// Asked and not answered yet.
    asked: BTreeSet<(String, bool)>,
    /// To ask, at the game's next tick.
    queue: Vec<(String, bool)>,
}

static CLIENT: Mutex<Option<Client>> = Mutex::new(None);

fn client<R>(f: impl FnOnce(&mut Client) -> R) -> R {
    let mut guard = CLIENT.lock().unwrap_or_else(|e| e.into_inner());
    f(guard.get_or_insert_with(Client::default))
}

/// The server: the players told that it answers tooltips.
static TOLD: Mutex<BTreeSet<u32>> = Mutex::new(BTreeSet::new());

/// A stack as the runtime reads it: [`stack_to_120`], which carries the stack's other data from
/// 1.20.1 (`minecraft:custom_data`: a battery's charge) back in its `tag`.
fn stack_key(stack: &ItemStack) -> String {
    stack_to_120(stack)
}

/// The item id in a 1.20.1 stack's SNBT (`{Count:1b,id:"farmersdelight:stove"}`).
fn stack_id(snbt: &str) -> &str {
    snbt.split("id:\"")
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_default()
}

fn texts(lines: &Value) -> Vec<Text> {
    lines
        .as_array()
        .into_iter()
        .flatten()
        .map(horizon_client::scoreboard::comp_text)
        .collect()
}

impl Game<'_> {
    /// The game, building a tooltip: the mods' lines for `stack`, put in as 1.20.1 puts them
    /// (`lines[0]` is the name). Asks for them when there's no answer yet or it's a second old.
    pub(in crate::game) fn java120_tooltip(
        &self,
        stack: &ItemStack,
        advanced: bool,
        lines: &mut Vec<(Text, u32)>,
    ) {
        if !ON.load(Ordering::Relaxed) || lines.is_empty() {
            return;
        }
        let key = (stack_key(stack), advanced);
        client(|c| {
            let stale = c.answers.get(&key).is_none_or(|a| a.at.elapsed() > FRESH);
            if stale && !c.asked.contains(&key) && !c.queue.contains(&key) {
                c.queue.push(key.clone());
            }
            if let Some(answer) = c.answers.get(&key) {
                let hover = answer.hover.iter().map(|t| (t.clone(), u32::MAX));
                lines.splice(1..1, hover);
                lines.extend(answer.after.iter().map(|t| (t.clone(), u32::MAX)));
            }
        });
    }

    /// The game: the tooltip pictures the mods give `stack` (a mod item's image, AppleSkin's
    /// food on vanilla food), when its answer has them (asked for with its lines, by
    /// [`Self::java120_tooltip`]).
    pub(in crate::game) fn java120_tooltip_image(&self, stack: &ItemStack) -> Option<Image> {
        if !ON.load(Ordering::Relaxed) {
            return None;
        }
        let key = (stack_key(stack), false);
        let advanced = (stack_key(stack), true);
        client(|c| {
            c.answers
                .get(&key)
                .or_else(|| c.answers.get(&advanced))
                .and_then(|a| a.image.clone())
        })
    }

    /// The game, each tick: the stacks to ask about, to the server.
    pub(in crate::game) fn ask_java120_tooltips(&mut self) {
        if !ON.load(Ordering::Relaxed) {
            return;
        }
        let queue = client(|c| {
            let queue = std::mem::take(&mut c.queue);
            c.asked.extend(queue.iter().cloned());
            queue
        });
        for (stack, advanced) in queue {
            let input =
                json!({"window": -4, "kind": "tooltip", "stack": stack, "advanced": advanced});
            self.send(S::CustomPayload(CustomPayload {
                channel: INPUT.into(),
                data: input.to_string().into_bytes(),
            }));
        }
    }

    /// The game: the server's word on tooltips.
    pub(in crate::game) fn remote_java120_tooltip(&mut self, data: &[u8]) {
        let Ok(message) = serde_json::from_slice::<Value>(data) else {
            return;
        };
        if message.get("on").is_some() {
            ON.store(true, Ordering::Relaxed);
            return;
        }
        let (Some(stack), advanced) = (
            message["stack"].as_str(),
            message["advanced"].as_bool().unwrap_or(false),
        ) else {
            return;
        };
        let key = (stack.to_owned(), advanced);
        client(|c| {
            c.asked.remove(&key);
            // Kept small: a player looks at a few hundred stacks at most.
            if c.answers.len() > 512 {
                c.answers.retain(|_, a| a.at.elapsed() < FRESH * 30);
            }
            c.answers.insert(
                key,
                Answer {
                    at: Instant::now(),
                    hover: texts(&message["hover"]),
                    after: texts(&message["after"]),
                    image: image(&message["image"]),
                },
            );
        });
    }

    /// The server, each tick: each player's game told, once, that it answers tooltips.
    pub(in crate::game) fn tell_java120_tooltips(&mut self) {
        let none_up = super::loaders().into_iter().all(|loader| {
            let _route = super::route(loader);
            runtime().try_lock().map(|r| r.is_none()).unwrap_or(false)
        });
        if self.remote.is_some() || none_up {
            return;
        }
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        let mut told = TOLD.lock().unwrap_or_else(|e| e.into_inner());
        let waiting: Vec<u32> = host
            .hub
            .guests
            .iter()
            .filter(|g| g.placed && !told.contains(&g.conn))
            .map(|g| g.conn)
            .collect();
        for conn in waiting {
            told.insert(conn);
            host.hub.send_to(
                conn,
                &C::CustomPayload(CustomPayload {
                    channel: CHANNEL.into(),
                    data: json!({"on": true}).to_string().into_bytes(),
                }),
            );
        }
    }

    /// The server: `conn`'s game asks what the mods add to a stack's tooltip.
    pub(in crate::game) fn java120_tooltip_request(
        &mut self,
        conn: u32,
        input: &Value,
    ) {
        let Some(stack) = input["stack"].as_str() else {
            return;
        };
        let advanced = input["advanced"].as_bool().unwrap_or(false);
        // The item's mod's runtime (its id, in the stack's NBT: `{id:"ns:path",...}`).
        let _route = super::route_id(stack_id(stack));
        let answer = {
            let Some(mut guard) = wait_for_runtime() else {
                return;
            };
            let Some(runtime) = guard.as_mut() else {
                return;
            };
            runtime.tooltip(stack, advanced)
        };
        let (hover, after, image) = match answer {
            Ok((hover, after, image)) => (
                serde_json::from_str::<Value>(&hover).unwrap_or(json!([])),
                serde_json::from_str::<Value>(&after).unwrap_or(json!([])),
                serde_json::from_str::<Value>(&image).unwrap_or(Value::Null),
            ),
            Err(e) => {
                log::warn!("1.20.1 mods: a tooltip failed: {e}");
                (json!([]), json!([]), Value::Null)
            }
        };
        let reply = json!({
            "stack": stack, "advanced": advanced, "hover": hover, "after": after, "image": image,
        });
        self.hub_host().hub.send_to(
            conn,
            &C::CustomPayload(CustomPayload {
                channel: CHANNEL.into(),
                data: reply.to_string().into_bytes(),
            }),
        );
    }
}

/// A player left: told again when they come back.
pub(super) fn forget(conn: u32) {
    TOLD.lock().unwrap_or_else(|e| e.into_inner()).remove(&conn);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stacks_own_data_goes_back_under_its_tag() {
        let mut stack = ItemStack::new("techreborn:red_cell_battery", 1);
        let custom = horizon_client_core::scoreboard::json_to_tag(&json!({"energy": 1200}));
        stack
            .data_mut()
            .set_other("minecraft:custom_data", Some(custom));
        let key = stack_key(&stack);
        assert_eq!(
            key, r#"{id:"techreborn:red_cell_battery",Count:1b,tag:{energy:1200}}"#,
            "its charge under tag"
        );
        horizon_core::nbt::parse_snbt(&key).expect("SNBT the runtime reads");
        // Horizon's own NBT (here its damage) beside it.
        stack.damage = 3;
        let key = stack_key(&stack);
        assert!(key.contains("tag:{Damage:3,energy:1200}"), "{key}");
        horizon_core::nbt::parse_snbt(&key).expect("SNBT the runtime reads");
        // Nested compounds keep their braces (a cooking pot's meal under BlockEntityTag).
        let mut pot = ItemStack::new("farmersdelight:cooking_pot", 1);
        let custom = horizon_core::nbt::parse_snbt(
            r#"{BlockEntityTag:{Inventory:{Items:[{Slot:6b,id:"farmersdelight:beef_stew",Count:2b}]}}}"#,
        )
        .expect("snbt")
        .0;
        pot.data_mut()
            .set_other("minecraft:custom_data", Some(custom));
        let key = stack_key(&pot);
        horizon_core::nbt::parse_snbt(&key).expect("SNBT the runtime reads");
        assert!(key.contains("Count:2b"), "{key}");
        // A stack with nothing of its own is as before.
        let plain = ItemStack::new("minecraft:bread", 3);
        assert_eq!(stack_key(&plain), stack_to_120(&plain));
    }
}
