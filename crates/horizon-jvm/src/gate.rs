//! Who may run Fabric mods in this game (docs/java-bridge.md, "Ownership").
//!
//! Fabric mods run on the player's own game jar, and their first start downloads Fabric Loader,
//! its libraries and Fabric API from Fabric's servers. That happens only when **the launcher
//! started the game for an account whose ownership of Minecraft: Java Edition was confirmed just
//! now**, and the game confirms it again itself:
//! - past its owner gate (`account::launch`, which asks the Minecraft services with the account's
//!   token), the launcher writes an **owner record** for this one start: the owner's profile UUID
//!   and Minecraft access token, in a fresh file only the player can read, in the launcher's data
//!   folder (`<launcher>/java-mods-owner/`), and passes its path in [`OWNER_VAR`] beside [`ARG`]
//!   (`launcher/src/java_variant.rs`, `switch_command`). It is the session file the launcher
//!   already hands the game (`--session-file`), written for the owner;
//! - the game reads the record once and deletes it ([`read_record`]): it must be fresh (written
//!   in the last [`RECORD_LIFE`]), the player's only, and name an account of the launcher's
//!   `accounts.json` beside its folder;
//! - then the game asks the Minecraft services with that token, as the launcher does: the profile
//!   must be the record's UUID and the entitlements must hold Minecraft: Java Edition
//!   ([`services_check`]). Only a token Mojang issued to an owner passes; no file or variable
//!   written by hand does, and offline (no answer) runs no mods either.
//!
//! [`ARG`] without a record that passes (the game started by hand, by another launcher, or with
//! the variables set by hand) runs no mods: the game says the launcher is needed and plays without
//! them, having downloaded nothing. The game itself never downloads anything of Mojang's (Java,
//! jars, assets), whoever starts it.
//!
//! Developers' harnesses (regress, `launch.sh`, `lan_e2e`, test servers) opt in with
//! `HORIZON_JAVA_MODS=1`, which exists only in builds with the `dev` feature
//! (`horizon-client/java-mods-dev`); release builds (`java-mods-release.yml`,
//! `package_java_variant.sh`) don't have it.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// The game argument the launcher passes for an instance whose Fabric switch is on.
pub const ARG: &str = "--java-mods";
/// The owner record's path, set by the launcher beside [`ARG`] past its owner gate.
pub const OWNER_VAR: &str = "HORIZON_JAVA_MODS_OWNER";
/// A developer's explicit opt-in (`=1`): the bridge's harnesses and test servers. Only in builds
/// with the `dev` feature.
#[cfg(feature = "dev")]
pub const DEV_VAR: &str = "HORIZON_JAVA_MODS";
/// Developers' builds only: the Minecraft services' address (a stand-in for the harnesses).
#[cfg(feature = "dev")]
pub const SERVICES_VAR: &str = "HORIZON_JAVA_MODS_SERVICES";
/// The Minecraft services the owner record's token is checked with.
pub const SERVICES: &str = "https://api.minecraftservices.com";
/// How old an owner record may be: the launcher writes it as it starts the game.
pub const RECORD_LIFE: Duration = Duration::from_secs(120);

/// What the player sees when [`ARG`] came without a record that passes.
pub const NEEDS_LAUNCHER: &str = "Fabric 26.3 mods run only when Horizon's launcher starts the \
     game for a signed-in account that owns Minecraft: Java Edition. Start this instance from \
     the launcher with Fabric 26.3 mods on. Playing without them; nothing was downloaded.";

/// Why [`ARG`] came without a record that passes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// No [`OWNER_VAR`] (the game started by hand).
    NoRecord,
    /// [`OWNER_VAR`] names no fresh, private record the launcher wrote.
    BadRecord,
    /// The record's account isn't one of the launcher's.
    NotInLauncher,
    /// The Minecraft services said the token isn't an owner's (or isn't valid).
    NotOwner,
    /// The Minecraft services gave no answer (offline).
    NoAnswer,
}

impl Refused {
    /// The detail after [`NEEDS_LAUNCHER`], for the log.
    pub fn detail(self) -> &'static str {
        match self {
            Refused::NoRecord => "no owner record from the launcher",
            Refused::BadRecord => "the owner record is missing, too old or readable by others",
            Refused::NotInLauncher => "the owner record's account is not signed in to the launcher",
            Refused::NotOwner => "the Minecraft services did not confirm the account owns the game",
            Refused::NoAnswer => {
                "the Minecraft services could not be reached to confirm ownership (offline?)"
            }
        }
    }
}

/// Whether Fabric mods run in this game, and why.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OptIn {
    /// Not asked for: the plain game.
    Off,
    /// The launcher started it for an owner, confirmed by the Minecraft services.
    Launcher,
    /// A developer's harness (`HORIZON_JAVA_MODS=1`, `dev` builds only).
    #[cfg(feature = "dev")]
    Developer,
    /// [`ARG`] without an owner the game could confirm: no mods, [`NEEDS_LAUNCHER`].
    NoOwner(Refused),
}

impl OptIn {
    /// Whether the mods run (and their files may be fetched).
    pub fn runs(self) -> bool {
        match self {
            OptIn::Launcher => true,
            #[cfg(feature = "dev")]
            OptIn::Developer => true,
            OptIn::Off | OptIn::NoOwner(_) => false,
        }
    }
}

/// The owner record the launcher wrote: who owns the game and the token to prove it with.
#[derive(Clone, PartialEq, Eq)]
pub struct Record {
    /// The profile UUID without dashes, lowercase.
    pub uuid: String,
    pub access_token: String,
}

impl std::fmt::Debug for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Record({}, <token hidden>)", self.uuid)
    }
}

/// What the Minecraft services said about a record's token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Services {
    /// The token's profile is the record's UUID, and it owns Minecraft: Java Edition.
    Owns,
    /// Refused, another profile, or no Minecraft: Java Edition.
    DoesNot,
    /// No answer (offline, a server error).
    NoAnswer(String),
}

/// Reads the owner record at `path` once and deletes it: it must be a fresh ([`RECORD_LIFE`])
/// file readable by its owner only, in a folder whose parent holds the launcher's `accounts.json`
/// listing its account.
pub fn read_record(path: &Path) -> Result<Record, Refused> {
    let meta = std::fs::symlink_metadata(path).map_err(|_| Refused::BadRecord)?;
    // A regular file only (no link to another file, no pipe to block on).
    if !meta.is_file() {
        return Err(Refused::BadRecord);
    }
    let text = std::fs::read_to_string(path);
    let _ = std::fs::remove_file(path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(Refused::BadRecord);
        }
    }
    let age = meta
        .modified()
        .ok()
        .and_then(|m| SystemTime::now().duration_since(m).ok());
    // A time in the future (or none) is no proof of freshness either.
    if age.is_none_or(|age| age > RECORD_LIFE) {
        return Err(Refused::BadRecord);
    }
    let json: serde_json::Value = text
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .ok_or(Refused::BadRecord)?;
    let field = |k: &str| {
        json.get(k)
            .and_then(serde_json::Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let (Some(uuid), Some(access_token)) = (field("uuid"), field("access_token")) else {
        return Err(Refused::BadRecord);
    };
    let uuid = uuid.replace('-', "").to_lowercase();
    if !launcher_has(&launcher_accounts(path), &uuid) {
        return Err(Refused::NotInLauncher);
    }
    Ok(Record { uuid, access_token })
}

/// The launcher's `accounts.json` for a record at `path` (`<launcher>/java-mods-owner/<file>`).
fn launcher_accounts(path: &Path) -> PathBuf {
    path.parent()
        .and_then(Path::parent)
        .unwrap_or(Path::new(""))
        .join("accounts.json")
}

/// Whether the launcher's `accounts.json` at `file` lists the account `uuid`.
fn launcher_has(file: &Path, uuid: &str) -> bool {
    let Some(json) = std::fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
    else {
        return false;
    };
    json.get("accounts")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|list| {
            list.iter().any(|a| {
                a.get("uuid")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|u| u.replace('-', "").eq_ignore_ascii_case(uuid))
            })
        })
}

/// Asks the Minecraft services at `base` about `record`'s token, as the launcher's owner gate does
/// (`account::owns_game`, `account::profile`).
pub fn services_check(base: &str, record: &Record) -> Services {
    services_check_with(crate::libs::agent(), base, record)
}

/// [`services_check`] through `agent`.
fn services_check_with(agent: &ureq::Agent, base: &str, record: &Record) -> Services {
    let get = |path: &str| -> Result<Option<serde_json::Value>, String> {
        match agent
            .get(&format!("{base}{path}"))
            .set("Authorization", &format!("Bearer {}", record.access_token))
            .timeout(Duration::from_secs(20))
            .call()
        {
            Ok(r) => {
                let mut body = Vec::new();
                r.into_reader()
                    .take(1 << 20)
                    .read_to_end(&mut body)
                    .map_err(|e| e.to_string())?;
                serde_json::from_slice(&body)
                    .map(Some)
                    .map_err(|e| e.to_string())
            }
            // Refused (401, 403, 404: no profile): not an owner's token.
            Err(ureq::Error::Status(code, _)) if (400..500).contains(&code) => Ok(None),
            Err(e) => Err(e.to_string()),
        }
    };
    let profile = match get("/minecraft/profile") {
        Ok(Some(v)) => v,
        Ok(None) => return Services::DoesNot,
        Err(e) => return Services::NoAnswer(e),
    };
    let id = profile
        .get("id")
        .and_then(serde_json::Value::as_str)
        .map(|id| id.replace('-', "").to_lowercase());
    if id.as_deref() != Some(record.uuid.as_str()) {
        return Services::DoesNot;
    }
    match get("/entitlements/mcstore") {
        Ok(Some(v)) if owns(&v) => Services::Owns,
        Ok(_) => Services::DoesNot,
        Err(e) => Services::NoAnswer(e),
    }
}

/// Whether the entitlements answer holds Minecraft: Java Edition (Game Pass included).
fn owns(entitlements: &serde_json::Value) -> bool {
    entitlements
        .get("items")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|items| {
            items.iter().any(|i| {
                matches!(
                    i.get("name").and_then(serde_json::Value::as_str),
                    Some("game_minecraft" | "product_minecraft")
                )
            })
        })
}

/// The decision for a game started with `args` and environment `var`; `services` asks the
/// Minecraft services about a record that passed [`read_record`].
pub fn decide<S: AsRef<str>>(
    args: impl IntoIterator<Item = S>,
    var: impl Fn(&str) -> Option<String>,
    services: impl FnOnce(&Record) -> Services,
) -> OptIn {
    #[cfg(feature = "dev")]
    if var(DEV_VAR).is_some_and(|v| v == "1") {
        return OptIn::Developer;
    }
    if !args.into_iter().any(|a| a.as_ref() == ARG) {
        return OptIn::Off;
    }
    let Some(path) = var(OWNER_VAR).filter(|v| !v.is_empty()) else {
        return OptIn::NoOwner(Refused::NoRecord);
    };
    let record = match read_record(Path::new(&path)) {
        Ok(record) => record,
        Err(why) => return OptIn::NoOwner(why),
    };
    match services(&record) {
        Services::Owns => OptIn::Launcher,
        Services::DoesNot => OptIn::NoOwner(Refused::NotOwner),
        Services::NoAnswer(e) => {
            log::warn!("Java mods: the Minecraft services gave no answer: {e}");
            OptIn::NoOwner(Refused::NoAnswer)
        }
    }
}

/// Asks the Minecraft services about `record`: Mojang's. A developer's build may name a
/// stand-in (`SERVICES_VAR`, on localhost: asked with no proxy between).
fn ask_services(record: &Record) -> Services {
    #[cfg(feature = "dev")]
    if let Some(base) = std::env::var(SERVICES_VAR).ok().filter(|v| !v.is_empty()) {
        return services_check_with(&ureq::AgentBuilder::new().build(), &base, record);
    }
    services_check(SERVICES, record)
}

/// [`decide`] for this process.
pub fn this_process() -> OptIn {
    decide(std::env::args(), |k| std::env::var(k).ok(), ask_services)
}

#[cfg(test)]
mod tests;
