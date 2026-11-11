//! Java mods' commands: the tree sent to players, and a player's line for a mod's command.
use super::*;
use serde_json::Value;

/// A vanilla command name: mods may not take it.
pub(super) fn vanilla_command(name: &str) -> bool {
    let d = horizon_client::command::vanilla::game_dispatcher();
    d.nodes[0]
        .children
        .iter()
        .any(|&c| d.nodes[c].name() == name)
}

/// The commands Java mods add (not vanilla's): their names, and their tree as a root of
/// the commands report, merged into every player's command tree.
pub(super) struct JavaCommands {
    names: std::collections::HashSet<String>,
    tree: Value,
}

/// The Java mods' commands, once the bridge has loaded the server's data (they don't
/// change after).
pub(super) fn java_commands() -> Option<&'static JavaCommands> {
    {
        static COMMANDS: std::sync::OnceLock<JavaCommands> = std::sync::OnceLock::new();
        if let Some(commands) = COMMANDS.get() {
            return Some(commands);
        }
        if !horizon_jvm::events::enabled() {
            return None;
        }
        let names: Vec<String> = horizon_jvm::events::command_names()
            .map_err(|e| log::warn!("Java mods: no commands: {e}"))
            .ok()?
            .into_iter()
            // Vanilla's, the dedicated server's and single player's own included: those
            // stay Horizon's.
            .filter(|n| {
                !vanilla_command(n)
                    && !horizon_client::lan::commands::DEDICATED_ONLY.contains(&n.as_str())
                    && !horizon_client::lan::commands::INTEGRATED_ONLY.contains(&n.as_str())
            })
            .collect();
        let tree = horizon_jvm::events::command_tree(&names)
            .map_err(|e| log::warn!("Java mods: no command tree: {e}"))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())?;
        if !names.is_empty() {
            log::info!("Java mods: commands /{}", names.join(", /"));
        }
        Some(COMMANDS.get_or_init(|| JavaCommands {
            names: names.into_iter().collect(),
            tree,
        }))
    }
}

impl Game<'_> {
    /// Players' command trees list the Java mods' commands after vanilla's and the native
    /// mods' (`register_native_commands`), once they load (a while after the world opens) and
    /// again whenever the hub's tree is set anew.
    pub(super) fn send_java_commands(&mut self) {
        /// The hub's key for a tree with the Java mods' commands merged in.
        const JAVA_KEY: u64 = 1 << 63;
        let Some(java) = java_commands() else {
            return;
        };
        let Some(host) = self.lan.as_mut() else {
            return;
        };
        if host.hub.mod_commands_key() == Some(JAVA_KEY) {
            return;
        }
        let mut tree = host.hub.mod_commands().clone();
        if tree.is_null() {
            tree = java.tree.clone();
        } else if let (Some(ours), Some(theirs)) = (
            tree["children"].as_object_mut(),
            java.tree["children"].as_object(),
        ) {
            // A native mod's command of the same name wins, as it runs first.
            for (name, node) in theirs {
                ours.entry(name.clone()).or_insert_with(|| node.clone());
            }
        }
        host.hub.set_mod_commands(JAVA_KEY, tree);
    }

    /// `/line` from the player with this id, when a Java mod owns the command: run in the
    /// JVM as that player, its answers sent back. False when no Java mod owns it.
    pub(super) fn java_command(
        &mut self,
        host: Option<&mut LanHost>,
        uuid: &str,
        line: &str,
    ) -> bool {
        if !java_mods_on() {
            return false;
        }
        let name = line.split_whitespace().next().unwrap_or_default();
        let Some(java) = java_commands() else {
            return false;
        };
        // A command naming a mod's entity type (`/summon mod:x`, `@e[type=mod:x]`): the
        // mods' server runs it (vanilla's command); Horizon knows no such type.
        let types = java_entity_types();
        let names_type = line
            .split(|c: char| c.is_whitespace() || matches!(c, '[' | ']' | ',' | '=' | '!' | '{'))
            .any(|word| types.contains(word));
        if !java.names.contains(name) && !names_type {
            // `@e`/`@n` may mean mods' entities too: the mods run the command first, for
            // their entities only and quietly; Horizon then runs it for its own.
            if !types.is_empty() && (line.contains("@e") || line.contains("@n")) {
                let ran =
                    self.with_java_world(|| horizon_jvm::events::run_for_entities(uuid, line));
                if let Err(e) = ran {
                    log::warn!("Java mods: /{line} for their entities failed: {e}");
                }
                self.apply_java_actions(host);
            }
            return false;
        }
        {
            match self.with_java_world(|| horizon_jvm::events::run_command(uuid, line)) {
                Ok(true) => {}
                Ok(false) => return false,
                Err(e) => {
                    log::warn!("Java mods: /{line} failed: {e}");
                    return false;
                }
            }
            self.apply_java_actions(host);
            true
        }
    }

    /// `/line` typed by the window's own player in-frame: true when a Java mod owns it.
    pub(in crate::game) fn local_java_command(&mut self, line: &str) -> bool {
        if !self.hears_players() {
            return false;
        }
        let Some(me) = self.java_players(None).first().map(|p| p.id.clone()) else {
            return false;
        };
        self.java_command(None, &me, line)
    }

    /// `/line` typed by a connection's player: true when a Java mod owns it.
    pub(in crate::game) fn guest_java_command(
        &mut self,
        host: &mut LanHost,
        conn: u32,
        line: &str,
    ) -> bool {
        if !self.hears_players() {
            return false;
        }
        let Some(g) = host.hub.guest(conn).filter(|g| g.placed) else {
            return false;
        };
        let id = uuid_string(g.uuid);
        self.java_command(Some(host), &id, line)
    }
}
