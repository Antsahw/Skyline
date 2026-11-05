//! Who may run Fabric mods in this game (docs/java-bridge.md, "Ownership").
//!
//! Fabric mods run on the player's own game jar, and their first start downloads Fabric Loader,
//! its libraries and Fabric API from Fabric's servers. That happens only in two cases:
//! - **the launcher started the game** for an account that owns Minecraft: Java Edition: it
//!   passes [`ARG`] and [`OWNER_VAR`] only on a command that passed its owner gate
//!   (`launcher/src/java_variant.rs`, `switch_command`);
//! - **a developer opted in** with [`DEV_VAR`]`=1` (the bridge's harnesses: regress,
//!   `launch.sh`, `lan_e2e`, servers they start), as Horizon's own harnesses run the plain game.
//!
//! [`ARG`] alone (the game started by hand, or by another launcher) runs no mods: the game says
//! the launcher is needed and plays without them, having downloaded nothing. The game itself
//! never downloads anything of Mojang's (Java, jars, assets), whoever starts it.

/// The game argument the launcher passes for an instance whose Fabric switch is on.
pub const ARG: &str = "--java-mods";
/// Set by the launcher beside [`ARG`], on a command that passed its owner gate.
pub const OWNER_VAR: &str = "HORIZON_JAVA_MODS_OWNER";
/// A developer's explicit opt-in (`=1`): the bridge's harnesses and test servers.
pub const DEV_VAR: &str = "HORIZON_JAVA_MODS";

/// What the player sees when [`ARG`] came without the launcher.
pub const NEEDS_LAUNCHER: &str = "Fabric 26.3 mods run only when Horizon's launcher starts the \
     game for a signed-in account that owns Minecraft: Java Edition. Start this instance from \
     the launcher with Fabric 26.3 mods on. Playing without them; nothing was downloaded.";

/// Whether Fabric mods run in this game, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptIn {
    /// Not asked for: the plain game.
    Off,
    /// The launcher started it past its owner gate.
    Launcher,
    /// A developer's harness ([`DEV_VAR`]).
    Developer,
    /// [`ARG`] without the launcher's owner gate: no mods, [`NEEDS_LAUNCHER`].
    NoOwner,
}

impl OptIn {
    /// Whether the mods run (and their files may be fetched).
    pub fn runs(self) -> bool {
        matches!(self, OptIn::Launcher | OptIn::Developer)
    }
}

/// The decision for a game started with `args` and environment `var`.
pub fn decide<S: AsRef<str>>(
    args: impl IntoIterator<Item = S>,
    var: impl Fn(&str) -> Option<String>,
) -> OptIn {
    if var(DEV_VAR).is_some_and(|v| v == "1") {
        return OptIn::Developer;
    }
    if !args.into_iter().any(|a| a.as_ref() == ARG) {
        return OptIn::Off;
    }
    if var(OWNER_VAR).is_some_and(|v| !v.is_empty()) {
        OptIn::Launcher
    } else {
        OptIn::NoOwner
    }
}

/// [`decide`] for this process.
pub fn this_process() -> OptIn {
    decide(std::env::args(), |k| std::env::var(k).ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str], vars: &[(&str, &str)]) -> OptIn {
        decide(args.iter().copied(), |k| {
            vars.iter()
                .find(|(n, _)| *n == k)
                .map(|(_, v)| v.to_string())
        })
    }

    #[test]
    fn started_by_hand_runs_no_mods() {
        // The game started directly with the switch's argument: no launcher, no owner.
        let direct = run(&["horizon-client", "--resources", "r", "--java-mods"], &[]);
        assert_eq!(direct, OptIn::NoOwner);
        assert!(!direct.runs());
        // An empty marker is no marker.
        assert_eq!(run(&["--java-mods"], &[(OWNER_VAR, "")]), OptIn::NoOwner);
        // The developer variable set to anything but 1 opts nothing in.
        assert_eq!(run(&["--java-mods"], &[(DEV_VAR, "0")]), OptIn::NoOwner);
    }

    #[test]
    fn the_launcher_past_its_gate_runs_them() {
        let launcher = run(&["--resources", "r", "--java-mods"], &[(OWNER_VAR, "1")]);
        assert_eq!(launcher, OptIn::Launcher);
        assert!(launcher.runs());
        // The marker without the argument (switch off) runs nothing.
        assert_eq!(run(&["--resources", "r"], &[(OWNER_VAR, "1")]), OptIn::Off);
    }

    #[test]
    fn developers_opt_in_explicitly() {
        assert_eq!(run(&["--world", "W"], &[(DEV_VAR, "1")]), OptIn::Developer);
        assert!(run(&[], &[(DEV_VAR, "1")]).runs());
        assert_eq!(run(&["--world", "W"], &[]), OptIn::Off);
        assert!(!OptIn::Off.runs());
    }
}
