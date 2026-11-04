//! Which of a folder's jars Horizon runs, and the game folder's `mods-java/` made from them.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

/// Mods that replace Minecraft's renderer: Horizon has its own, so they can't work in it.
const RENDERERS: [&str; 5] = ["sodium", "iris", "canvas", "nvidium", "vulkanmod"];
/// Horizon brings its own Fabric API, made for it: two copies would clash.
const FABRIC_API: &str = "fabric-api";
/// The list of jars this program put in `mods-java/`: it never touches any others.
const MANAGED: &str = ".horizon-play-jars";

/// What a jar's `fabric.mod.json` says.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModInfo {
    pub file: String,
    pub id: Option<String>,
    pub requires: Vec<String>,
}

/// Why a jar was left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Left {
    FabricApi,
    NotFabric,
    Renderer,
    Needs(String),
}

impl std::fmt::Display for Left {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Left::FabricApi => write!(f, "Horizon brings its own Fabric API"),
            Left::NotFabric => write!(f, "not a Fabric mod"),
            Left::Renderer => write!(f, "it replaces the renderer, and Horizon has its own"),
            Left::Needs(id) => write!(f, "it needs {id}, which was left out"),
        }
    }
}

/// The jars to run, and the others with why. A mod needing a left-out renderer goes too (Fabric
/// would refuse to start otherwise); a mod needing Fabric API stays, the game has that.
pub fn choose(mods: &[ModInfo]) -> (Vec<String>, BTreeMap<String, Left>) {
    let mut left = BTreeMap::new();
    let mut gone: BTreeSet<&str> = BTreeSet::new();
    for m in mods {
        match m.id.as_deref() {
            None => {
                left.insert(m.file.clone(), Left::NotFabric);
            }
            Some(FABRIC_API) => {
                left.insert(m.file.clone(), Left::FabricApi);
            }
            Some(id) if RENDERERS.contains(&id) => {
                left.insert(m.file.clone(), Left::Renderer);
                gone.insert(id);
            }
            Some(_) => {}
        }
    }
    loop {
        let mut changed = false;
        for m in mods {
            if left.contains_key(&m.file) {
                continue;
            }
            if let Some(need) = m.requires.iter().find(|r| gone.contains(r.as_str())) {
                left.insert(m.file.clone(), Left::Needs(need.clone()));
                if let Some(id) = &m.id {
                    gone.insert(id);
                }
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    let keep = mods
        .iter()
        .filter(|m| !left.contains_key(&m.file))
        .map(|m| m.file.clone())
        .collect();
    (keep, left)
}

/// A jar's Fabric id and required mods; no id when it has no readable `fabric.mod.json`.
pub fn read_info(jar: &Path) -> ModInfo {
    let file = jar
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    let json: Option<serde_json::Value> = std::fs::File::open(jar)
        .ok()
        .and_then(|f| zip::ZipArchive::new(f).ok())
        .and_then(|mut z| {
            let mut text = String::new();
            z.by_name("fabric.mod.json")
                .ok()?
                .read_to_string(&mut text)
                .ok()?;
            // Fabric's reader takes raw line breaks inside strings, and mods use them.
            let text = text.replace(['\n', '\r', '\t'], " ");
            // Unreadable here is no reason to refuse it: Fabric decides.
            Some(serde_json::from_str(&text).unwrap_or_else(|_| serde_json::json!({"id": "?"})))
        });
    let Some(json) = json else {
        return ModInfo {
            file,
            id: None,
            requires: Vec::new(),
        };
    };
    ModInfo {
        file,
        id: json["id"].as_str().map(str::to_string),
        requires: json["depends"]
            .as_object()
            .map(|d| d.keys().cloned().collect())
            .unwrap_or_default(),
    }
}

/// The `.jar` files directly in `dir`.
pub fn jars_in(dir: &Path) -> Vec<PathBuf> {
    let mut jars: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("jar")))
        .collect();
    jars.sort();
    jars
}

/// Makes `to` (`mods-java/`) hold the chosen jars of `from`: a hard link, which needs no
/// privilege and no space, else a copy. Unchanged jars stay as they are, so the game's content
/// cache (made from their names, sizes and times) stays good. Returns how many it runs.
pub fn sync(from: &Path, to: &Path, say: &dyn Fn(String)) -> Result<usize, String> {
    let infos: Vec<ModInfo> = jars_in(from).iter().map(|j| read_info(j)).collect();
    let (keep, left) = choose(&infos);
    for (file, why) in &left {
        say(format!("Leaving out {file}: {why}"));
    }
    std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let list = to.join(MANAGED);
    for old in std::fs::read_to_string(&list).unwrap_or_default().lines() {
        if !old.is_empty() && !keep.iter().any(|k| k == old) {
            let _ = std::fs::remove_file(to.join(old));
        }
    }
    for file in &keep {
        let (source, target) = (from.join(file), to.join(file));
        if same_file_stamp(&source, &target) {
            continue;
        }
        let _ = std::fs::remove_file(&target);
        if std::fs::hard_link(&source, &target).is_err() {
            std::fs::copy(&source, &target).map_err(|e| format!("{file}: {e}"))?;
        }
    }
    std::fs::write(&list, keep.join("\n")).map_err(|e| format!("{}: {e}", list.display()))?;
    Ok(keep.len())
}

fn same_file_stamp(a: &Path, b: &Path) -> bool {
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(a), Ok(b)) => a.len() == b.len() && a.modified().ok() == b.modified().ok(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info(file: &str, id: Option<&str>, requires: &[&str]) -> ModInfo {
        ModInfo {
            file: file.into(),
            id: id.map(str::to_string),
            requires: requires.iter().map(|r| r.to_string()).collect(),
        }
    }

    #[test]
    fn leaves_out_renderers_and_what_needs_them() {
        let (keep, left) = choose(&[
            info("api.jar", Some("fabric-api"), &[]),
            info("delight.jar", Some("farmersdelight"), &["fabric-api"]),
            info("sodium.jar", Some("sodium"), &[]),
            info("extra.jar", Some("sodium-extra"), &["sodium"]),
            info("extra-addon.jar", Some("addon"), &["sodium-extra"]),
            info("forge.jar", None, &[]),
        ]);
        assert_eq!(keep, ["delight.jar"]);
        assert_eq!(left["api.jar"], Left::FabricApi);
        assert_eq!(left["sodium.jar"], Left::Renderer);
        assert_eq!(left["extra.jar"], Left::Needs("sodium".into()));
        assert_eq!(left["extra-addon.jar"], Left::Needs("sodium-extra".into()));
        assert_eq!(left["forge.jar"], Left::NotFabric);
    }

    #[test]
    fn reads_line_breaks_in_strings_and_keeps_what_it_cant_read() {
        let dir = std::env::temp_dir().join(format!("horizon-play-json-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let jar = |name: &str, json: &str| {
            let path = dir.join(name);
            let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
            z.start_file::<_, ()>("fabric.mod.json", Default::default())
                .unwrap();
            std::io::Write::write_all(&mut z, json.as_bytes()).unwrap();
            z.finish().unwrap();
            path
        };
        let lines = jar(
            "lines.jar",
            "{\"id\":\"etf\",\"description\":\"a
b\",\"depends\":{\"sodium\":\"*\"}}",
        );
        let info = read_info(&lines);
        assert_eq!(info.id.as_deref(), Some("etf"));
        assert_eq!(info.requires, ["sodium"]);
        let broken = read_info(&jar("broken.jar", "{not json"));
        let (keep, _) = choose(&[broken]);
        assert_eq!(keep, ["broken.jar"]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn sync_keeps_the_players_own_jars_and_drops_its_old_ones() {
        let dir = std::env::temp_dir().join(format!("horizon-play-sync-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (from, to) = (dir.join("mods"), dir.join("mods-java"));
        std::fs::create_dir_all(&from).unwrap();
        std::fs::create_dir_all(&to).unwrap();
        let write_jar = |path: &Path, id: &str| {
            let mut z = zip::ZipWriter::new(std::fs::File::create(path).unwrap());
            z.start_file::<_, ()>("fabric.mod.json", Default::default())
                .unwrap();
            std::io::Write::write_all(&mut z, format!("{{\"id\":\"{id}\"}}").as_bytes()).unwrap();
            z.finish().unwrap();
        };
        write_jar(&from.join("a.jar"), "a");
        write_jar(&from.join("b.jar"), "b");
        std::fs::write(to.join("own.jar"), b"the player's").unwrap();
        assert_eq!(sync(&from, &to, &|_| {}).unwrap(), 2);
        assert!(to.join("a.jar").is_file() && to.join("b.jar").is_file());
        std::fs::remove_file(from.join("a.jar")).unwrap();
        assert_eq!(sync(&from, &to, &|_| {}).unwrap(), 1);
        assert!(!to.join("a.jar").exists());
        assert!(to.join("b.jar").is_file() && to.join("own.jar").is_file());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
