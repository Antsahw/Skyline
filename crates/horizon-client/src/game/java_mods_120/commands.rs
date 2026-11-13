//! The 1.20.1 mods' commands and messages (phase 5j). The mods' commands (`/waystones`,
//! `/supplementaries`) join every player's command tree, after vanilla's and the other mods';
//! a player's line for one runs in the runtime as that player, at their permission level, and
//! what it changes lands in Horizon as any call's changes do. What the mods tell a player (a
//! command's answer, a mod's notice: `ServerPlayer.sendSystemMessage`) reaches them in chat or
//! over the hotbar.

use super::super::lan::LanHost;
use super::{Level, RUNTIME, uuid_string, wait_for_runtime};
use horizon_protocol::packets::play::{self, Clientbound as C};
use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

/// The mods' commands (not vanilla's or Horizon's): their names, and their tree as a root of
/// the commands report. Read once, as the runtime's server has loaded its data.
struct Commands120 {
    names: HashSet<String>,
    tree: serde_json::Value,
}

static COMMANDS: OnceLock<Commands120> = OnceLock::new();

/// The hub's key for a command tree with the 1.20.1 mods' commands merged in.
const KEY_120: u64 = (1 << 63) | (1 << 62);

/// What the mods told players, until the server's next tick sends it: the player's id, the
/// message as 1.20.1's JSON text, whether it goes over the hotbar.
static TOLD: Mutex<Vec<(String, String, bool)>> = Mutex::new(Vec::new());

/// A message the mods sent a player (`World120::tell`).
pub(super) fn told(player: &str, json: &str, overlay: bool) {
    TOLD.lock().unwrap_or_else(|e| e.into_inner()).push((
        player.to_owned(),
        json.to_owned(),
        overlay,
    ));
}

/// Whether a command name is one Horizon has itself: vanilla's, or a server's own.
fn horizons(name: &str) -> bool {
    let d = horizon_client::command::vanilla::game_dispatcher();
    d.nodes[0]
        .children
        .iter()
        .any(|&c| d.nodes[c].name() == name)
        || horizon_client::lan::commands::DEDICATED_ONLY.contains(&name)
        || horizon_client::lan::commands::INTEGRATED_ONLY.contains(&name)
}

/// The mods' commands, once the runtime can say them.
fn commands() -> Option<&'static Commands120> {
    if let Some(commands) = COMMANDS.get() {
        return Some(commands);
    }
    let Ok(mut guard) = RUNTIME.try_lock() else {
        return None;
    };
    let runtime = guard.as_mut()?;
    let names: Vec<String> = match runtime.command_names() {
        Ok(names) => names.into_iter().filter(|n| !horizons(n)).collect(),
        Err(e) => {
            log::warn!("1.20.1 mods: their commands can't be read: {e}");
            Vec::new()
        }
    };
    let tree = if names.is_empty() {
        serde_json::json!({"type": "root", "children": {}})
    } else {
        match runtime
            .command_tree(&names)
            .map_err(|e| e.to_string())
            .and_then(|t| serde_json::from_str(&t).map_err(|e| e.to_string()))
        {
            Ok(tree) => tree,
            Err(e) => {
                log::warn!("1.20.1 mods: their command tree can't be read: {e}");
                return None;
            }
        }
    };
    if !names.is_empty() {
        log::info!("1.20.1 mods: commands /{}", names.join(", /"));
    }
    Some(COMMANDS.get_or_init(|| Commands120 {
        names: names.into_iter().collect(),
        tree,
    }))
}

/// The tree a hub's players have, with `ours` merged in: a command of the same name already
/// there (vanilla's, a native or 26.3 mod's) stays, as it runs first. `None` when every one of
/// ours is there already.
fn merged(theirs: &serde_json::Value, ours: &serde_json::Value) -> Option<serde_json::Value> {
    let ours = ours["children"].as_object()?;
    if ours.is_empty() {
        return None;
    }
    let mut tree = theirs.clone();
    if tree.is_null() {
        tree = serde_json::json!({"type": "root", "children": {}});
    }
    let children = tree["children"].as_object_mut()?;
    if ours.keys().all(|name| children.contains_key(name)) {
        return None;
    }
    for (name, node) in ours {
        children.entry(name.clone()).or_insert_with(|| node.clone());
    }
    Some(tree)
}

impl super::super::Game<'_> {
    /// Each tick: the mods' commands into the players' command trees (once, and again when
    /// the hub's tree is set anew), and what the mods told players to them.
    pub(in crate::game) fn tick_java120_commands(&mut self) {
        if self.remote.is_some() {
            return;
        }
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        if let Some(commands) = commands()
            && host.hub.mod_commands_key() != Some(KEY_120)
            && let Some(tree) = merged(host.hub.mod_commands(), &commands.tree)
        {
            host.hub.set_mod_commands(KEY_120, tree);
        }
        send_told(host);
    }

    /// `line` (no slash) from `conn`'s player, when a 1.20.1 mod owns the command: run in
    /// the runtime as them. False when none does.
    pub(in crate::game) fn guest_java120_command(
        &mut self,
        host: &mut LanHost,
        conn: u32,
        line: &str,
    ) -> bool {
        let name = line.split_whitespace().next().unwrap_or_default();
        if !commands().is_some_and(|c| c.names.contains(name)) {
            return false;
        }
        let Some(guest) = host.hub.guest(conn).filter(|g| g.placed) else {
            return false;
        };
        let (uuid, player, permission) = (
            uuid_string(guest.uuid),
            guest.name.clone(),
            guest.permission,
        );
        self.as_guest(host, conn, |g| {
            let dimension = g.dim.id().to_owned();
            let ran = {
                let mut guard = wait_for_runtime()?;
                let runtime = guard.as_mut()?;
                runtime.command(
                    &Level(g),
                    &uuid,
                    &player,
                    &dimension,
                    i32::from(permission),
                    line,
                )
            };
            match ran {
                Ok(done) => {
                    g.apply_java120(&dimension, &done.set, &done.block_entities, &done.spawned)
                }
                Err(e) => log::warn!("1.20.1 mods: /{line} failed: {e}"),
            }
            Some(())
        });
        send_told(host);
        true
    }
}

/// What the mods told players, to those of them on this server.
fn send_told(host: &mut LanHost) {
    let told = std::mem::take(&mut *TOLD.lock().unwrap_or_else(|e| e.into_inner()));
    for (player, json, overlay) in told {
        let Ok(text) = serde_json::from_str::<serde_json::Value>(&json) else {
            continue;
        };
        let Some(conn) = host
            .hub
            .guests
            .iter()
            .find(|g| g.placed && uuid_string(g.uuid) == player)
            .map(|g| g.conn)
        else {
            continue;
        };
        host.hub.send_to(
            conn,
            &C::SystemChat(play::SystemChat {
                content: horizon_client::lan::data::component_nbt(&text),
                overlay,
            }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_mods_commands_join_the_tree_without_taking_a_name() {
        let theirs = json!({"type": "root", "children": {
            "home": {"type": "literal", "executable": true},
        }});
        let ours = json!({"type": "root", "children": {
            "home": {"type": "literal", "children": {}},
            "waystones": {"type": "literal", "executable": true},
        }});
        let tree = merged(&theirs, &ours).expect("waystones is new");
        // The command already there stays as it was; the new one joins.
        assert_eq!(tree["children"]["home"]["executable"], true);
        assert_eq!(tree["children"]["waystones"]["executable"], true);
        // Once merged, nothing more to do.
        assert!(merged(&tree, &ours).is_none());
        // A hub with no mods' tree yet takes ours whole.
        assert_eq!(
            merged(&serde_json::Value::Null, &ours).unwrap()["children"],
            ours["children"]
        );
    }
}
