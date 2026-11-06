//! The game folder's Java mods (`mods-java/`): Fabric mods a game built with Java mods runs
//! (docs/java-bridge.md). The launcher lists them with their Fabric names, adds jars, sends them
//! to the trash, and reads from the game's last log which of them failed.
use std::io::Read;
use std::path::{Path, PathBuf};

/// The folder's name in the game folder, as the game reads it (`horizon_jvm::DIR`).
pub const DIR: &str = "mods-java";

/// One jar of `mods-java/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaMod {
    /// Its file name in the folder.
    pub file: String,
    /// `fabric.mod.json`'s id, name and version, when the jar has them.
    pub id: Option<String>,
    pub name: Option<String>,
    pub version: Option<String>,
    pub size: u64,
}

impl JavaMod {
    /// What to call it: its Fabric name, else its file name.
    pub fn title(&self) -> &str {
        self.name.as_deref().unwrap_or(&self.file)
    }
}

/// What the game's last log says went wrong with its Java mods.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Failure {
    /// The mod named, when the line names one ("[Java mods] <mod> failed in <where>: …").
    pub name: Option<String>,
    /// The line, without its time and thread.
    pub text: String,
}

/// `home`'s `mods-java/`.
pub fn dir(home: &Path) -> PathBuf {
    home.join(DIR)
}

/// The jars in `home`'s `mods-java/`, by file name.
pub fn list(home: &Path) -> Vec<JavaMod> {
    let mut mods: Vec<JavaMod> = std::fs::read_dir(dir(home))
        .map(|entries| {
            entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && is_jar(p))
                .map(|p| describe(&p))
                .collect()
        })
        .unwrap_or_default();
    mods.sort_by_key(|m| m.file.to_lowercase());
    mods
}

fn is_jar(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("jar"))
}

/// A jar's file name, size and Fabric facts.
fn describe(path: &Path) -> JavaMod {
    let meta = fabric_meta(path);
    let field = |key: &str| {
        meta.as_ref()
            .and_then(|m| m.get(key))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string)
    };
    JavaMod {
        file: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        id: field("id"),
        name: field("name"),
        version: field("version"),
        size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0),
    }
}

/// The jar's `fabric.mod.json`, if it has one that parses.
fn fabric_meta(path: &Path) -> Option<serde_json::Value> {
    let mut archive = zip::ZipArchive::new(std::fs::File::open(path).ok()?).ok()?;
    let entry = archive.by_name("fabric.mod.json").ok()?;
    let mut text = String::new();
    entry.take(1 << 20).read_to_string(&mut text).ok()?;
    // Some mods' files carry raw line breaks inside strings, which Fabric's reader accepts.
    let text: String = text
        .chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect();
    serde_json::from_str(&text).ok()
}

/// Copies the Fabric mod `jar` into `home`'s `mods-java/`. A jar without `fabric.mod.json` (not
/// a Fabric mod: a Forge or NeoForge one) is refused, and so is a file name already there.
pub fn add(home: &Path, jar: &Path) -> Result<JavaMod, String> {
    if !jar.is_file() || !is_jar(jar) {
        return Err(format!("{} is not a .jar file", jar.display()));
    }
    if fabric_meta(jar).is_none() {
        return Err(format!(
            "{} is not a Fabric mod (it has no fabric.mod.json): Horizon runs Fabric mods",
            jar.display()
        ));
    }
    let name = jar.file_name().ok_or("The jar has no file name")?;
    let folder = dir(home);
    std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", folder.display()))?;
    let target = folder.join(name);
    if target.exists() {
        return Err(format!("{} is already in {DIR}", name.to_string_lossy()));
    }
    std::fs::copy(jar, &target).map_err(|e| format!("{}: {e}", target.display()))?;
    Ok(describe(&target))
}

/// Sends `file` (a file name in `mods-java/`, or the mod's Fabric id or name) to the trash.
pub fn remove(home: &Path, which: &str) -> Result<JavaMod, String> {
    let found = list(home)
        .into_iter()
        .find(|m| {
            m.file == which
                || m.id.as_deref() == Some(which)
                || m.name
                    .as_deref()
                    .is_some_and(|n| n.eq_ignore_ascii_case(which))
        })
        .ok_or_else(|| format!("No Java mod {which} in {DIR}"))?;
    crate::desktop_seams::to_trash(&dir(home).join(&found.file))?;
    Ok(found)
}

/// What went wrong with the Java mods in the game's last run (`logs/latest.log` in `home`):
/// the mods that failed while it ran, as the game told the players, and a start that failed.
pub fn failures(home: &Path) -> Vec<Failure> {
    let log = std::fs::read_to_string(home.join("logs").join("latest.log")).unwrap_or_default();
    failures_in(&log)
}

fn failures_in(log: &str) -> Vec<Failure> {
    let mut out: Vec<Failure> = Vec::new();
    for line in log.lines() {
        // "[12:00:00] [Server thread/INFO]: ..." → the message.
        let message = line.split_once("]: ").map_or(line, |(_, m)| m).trim();
        let failure = if let Some(rest) = message.split_once("[Java mods] ").map(|(_, r)| r) {
            let Some((name, _)) = rest.split_once(" failed in ") else {
                continue;
            };
            Failure {
                name: Some(name.trim().to_string()),
                text: rest.trim().to_string(),
            }
        } else if message.starts_with("Java mods: not started")
            || message.starts_with("Java mods: Fabric can't start")
            || message.contains("Incompatible mods found")
            || message.contains("Mod resolution failed")
        {
            Failure {
                name: None,
                text: message.to_string(),
            }
        } else {
            continue;
        };
        if !out.contains(&failure) {
            out.push(failure);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jar(path: &Path, meta: Option<&str>) {
        let mut zip = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        if let Some(meta) = meta {
            zip.start_file("fabric.mod.json", options).unwrap();
            std::io::Write::write_all(&mut zip, meta.as_bytes()).unwrap();
        }
        zip.start_file("a/B.class", options).unwrap();
        zip.finish().unwrap();
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hz-java-mods-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn adds_lists_and_removes_fabric_jars() {
        let root = scratch("add");
        let home = root.join("home");
        let fd = root.join("FarmersDelight.jar");
        jar(
            &fd,
            Some(r#"{"id":"farmersdelight","name":"Farmer's Delight","version":"3.6"}"#),
        );
        let forge = root.join("forge-only.jar");
        jar(&forge, None);
        let added = add(&home, &fd).unwrap();
        assert_eq!(added.title(), "Farmer's Delight");
        assert!(add(&home, &fd).unwrap_err().contains("already"));
        assert!(add(&home, &forge).unwrap_err().contains("not a Fabric mod"));
        let listed = list(&home);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id.as_deref(), Some("farmersdelight"));
        // SAFETY of the test: the trash is a scratch folder (desktop_seams).
        unsafe { std::env::set_var(crate::desktop_seams::TRASH_ENV, root.join("trash")) };
        remove(&home, "farmersdelight").unwrap();
        assert!(list(&home).is_empty());
        assert!(root.join("trash/FarmersDelight.jar").is_file());
        assert!(remove(&home, "farmersdelight").is_err());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn reads_failures_from_the_log() {
        let log = "\
[10:00:00] [Server thread/INFO]: System chat: [Java mods] Farmer's Delight failed in the HUD: NullPointerException: x (the game goes on; the log has more)
[10:00:01] [Server thread/INFO]: System chat: [Java mods] Farmer's Delight failed in the HUD: NullPointerException: x (the game goes on; the log has more)
[10:00:02] [Render thread/WARN]: Java mods: not started: no Java 25
[10:00:03] [Server thread/INFO]: System chat: hello
";
        let found = failures_in(log);
        assert_eq!(found.len(), 2);
        // The owner gate's refusal stops the game the same way (`not_started`): the tab says why.
        let refused = "[10:00:00] [main/WARN]: Java mods: off: Fabric 26.3 mods run only when Horizon's launcher starts the game (no owner record from the launcher)
[10:00:00] [main/ERROR]: Java mods: not started: Fabric 26.3 mods run only when Horizon's launcher starts the game (no owner record from the launcher)
";
        let gate = failures_in(refused);
        assert_eq!(gate.len(), 1, "{gate:?}");
        assert!(gate[0].text.contains("no owner record"));
        assert_eq!(found[0].name.as_deref(), Some("Farmer's Delight"));
        assert!(found[1].text.starts_with("Java mods: not started"));
    }
}
