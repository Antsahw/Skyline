//! A modpack's resource packs: the instance's `resourcepacks/` folder seen by the game, and the
//! pack's selection (its `options.txt`) turned on in Horizon's own options.
//!
//! The instance's selection is applied once, and again only when it changes (a pack update), so
//! what the player turns on or off in Horizon's pack screen stays.
use std::path::Path;

/// The options a modpack selects its resource packs with (`Options.resourcePacks`, and the packs
/// of another version it allows).
const KEYS: [&str; 2] = ["resourcePacks", "incompatibleResourcePacks"];
/// What was last applied from the instance, in the game folder.
const RECORD: &str = ".horizon-play-packs";

/// Makes the instance's packs the game's and applies its selection. `link` links a folder.
pub fn carry(
    instance: &Path,
    home: &Path,
    link: &dyn Fn(&Path, &Path) -> std::io::Result<()>,
    say: &dyn Fn(String),
) {
    let source = instance.join("resourcepacks");
    let _ = std::fs::create_dir_all(&source);
    let target = home.join("resourcepacks");
    match std::fs::symlink_metadata(&target) {
        Err(_) => {
            if let Err(e) = link(&source, &target) {
                say(format!(
                    "The modpack's resource packs couldn't be linked ({e})"
                ));
            }
        }
        // An earlier start's own folder (the game makes it; a player may have copied packs in):
        // each of the instance's packs it lacks goes in beside them.
        Ok(meta) if meta.is_dir() => {
            for entry in std::fs::read_dir(&source).into_iter().flatten().flatten() {
                let to = target.join(entry.file_name());
                if std::fs::symlink_metadata(&to).is_ok() {
                    continue;
                }
                let from = entry.path();
                let done = if from.is_dir() {
                    link(&from, &to)
                } else {
                    std::fs::hard_link(&from, &to)
                        .or_else(|_| std::fs::copy(&from, &to).map(|_| ()))
                };
                if let Err(e) = done {
                    say(format!("{}: {e}", from.display()));
                }
            }
        }
        Ok(_) => {}
    }
    let Ok(theirs) = std::fs::read_to_string(instance.join("options.txt")) else {
        return;
    };
    let options = home.join("options.txt");
    let ours = std::fs::read_to_string(&options).ok();
    let last = std::fs::read_to_string(home.join(RECORD)).ok();
    if let Some((text, record)) = select(&theirs, ours.as_deref(), last.as_deref()) {
        match std::fs::write(&options, text) {
            Ok(()) => {
                let _ = std::fs::write(home.join(RECORD), record);
                say("The modpack's resource packs are turned on".into());
            }
            Err(e) => say(format!("{}: {e}", options.display())),
        }
    }
}

/// The value of `key` in an options.txt.
fn value<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .find_map(|l| l.strip_prefix(key)?.strip_prefix(':'))
}

/// Horizon's options.txt with the instance's selection in it, and the record of that selection;
/// `None` when the instance selects nothing or its selection was applied already.
fn select(theirs: &str, ours: Option<&str>, last: Option<&str>) -> Option<(String, String)> {
    value(theirs, KEYS[0])?;
    let record: String = KEYS
        .iter()
        .filter_map(|k| Some(format!("{k}:{}\n", value(theirs, k)?)))
        .collect();
    if last == Some(record.as_str()) {
        return None;
    }
    let mut text = match ours {
        Some(ours) => ours
            .lines()
            .filter(|l| {
                !KEYS
                    .iter()
                    .any(|k| l.strip_prefix(k).is_some_and(|r| r.starts_with(':')))
            })
            .map(|l| format!("{l}\n"))
            .collect(),
        // A first start: the instance's options version, so the game updates nothing else.
        None => value(theirs, "version")
            .map(|v| format!("version:{v}\n"))
            .unwrap_or_default(),
    };
    text.push_str(&record);
    Some((text, record))
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRISM: &str = "version:5023\nfov:0.0\nresourcePacks:[\"vanilla\",\"fabric\",\"file/Look.zip\"]\n\
                         incompatibleResourcePacks:[]\nmaxFps:120\n";

    #[test]
    fn a_first_start_takes_the_packs_selection_and_nothing_else() {
        let (text, record) = select(PRISM, None, None).unwrap();
        assert_eq!(
            text,
            "version:5023\nresourcePacks:[\"vanilla\",\"fabric\",\"file/Look.zip\"]\n\
             incompatibleResourcePacks:[]\n"
        );
        assert!(text.ends_with(&record));
    }

    #[test]
    fn the_selection_replaces_horizons_once_and_the_players_choice_then_stays() {
        let ours = "version:5023\nresourcePacks:[\"vanilla\"]\nfov:0.5\n";
        let (text, record) = select(PRISM, Some(ours), None).unwrap();
        assert!(text.starts_with("version:5023\nfov:0.5\n"));
        assert_eq!(value(&text, "resourcePacks"), value(PRISM, "resourcePacks"));
        // The player turned the pack off in Horizon: the next start leaves that.
        assert_eq!(select(PRISM, Some(ours), Some(&record)), None);
        // The modpack's update changes its selection: applied again.
        let updated = PRISM.replace("file/Look.zip", "file/Look 2.zip");
        assert!(select(&updated, Some(ours), Some(&record)).is_some());
    }

    #[test]
    fn an_instance_that_selects_nothing_changes_nothing() {
        assert_eq!(select("version:5023\nfov:0.0\n", None, None), None);
    }

    #[test]
    fn the_instances_folder_and_its_packs_reach_the_game() {
        let dir = std::env::temp_dir().join(format!("horizon-play-packs-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (instance, home, later) = (dir.join("mc"), dir.join("mc/horizon"), dir.join("later"));
        std::fs::create_dir_all(instance.join("resourcepacks/Folder Pack")).unwrap();
        std::fs::write(instance.join("resourcepacks/Look.zip"), b"zip").unwrap();
        std::fs::write(instance.join("options.txt"), PRISM).unwrap();
        std::fs::create_dir_all(&home).unwrap();
        let link = |t: &Path, l: &Path| copy_dir(t, l);
        carry(&instance, &home, &link, &|_| {});
        assert!(home.join("resourcepacks/Look.zip").is_file());
        let options = std::fs::read_to_string(home.join("options.txt")).unwrap();
        assert_eq!(
            value(&options, "resourcePacks"),
            value(PRISM, "resourcePacks")
        );
        // A game folder whose own resourcepacks/ is a folder: the instance's packs go into it.
        std::fs::create_dir_all(later.join("resourcepacks")).unwrap();
        std::fs::write(later.join("resourcepacks/mine.zip"), b"mine").unwrap();
        carry(&instance, &later, &link, &|_| {});
        assert!(later.join("resourcepacks/mine.zip").is_file());
        assert!(later.join("resourcepacks/Look.zip").is_file());
        assert!(later.join("resourcepacks/Folder Pack").is_dir());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn copy_dir(from: &Path, to: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(to)?;
        for entry in std::fs::read_dir(from)? {
            let entry = entry?;
            if entry.path().is_dir() {
                copy_dir(&entry.path(), &to.join(entry.file_name()))?;
            } else {
                std::fs::copy(entry.path(), to.join(entry.file_name()))?;
            }
        }
        Ok(())
    }
}
