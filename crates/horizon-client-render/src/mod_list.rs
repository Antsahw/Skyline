//! Which of the installed Horizon mods are on (`docs/mods.md`), kept in
//! `horizon-mods.txt` beside `options.txt` as `mod.<id>:true` lines. Every
//! mod is off until the player turns it on in Options > Mods. A native mod is
//! built in, so on until turned off (`mod.<id>:false`); the launcher builds
//! only the native mods whose line is `true`.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub const FILE: &str = "horizon-mods.txt";
/// The folder holding `.hmod` files, beside `options.txt`.
pub const DIR: &str = "horizon-mods";
/// Where the launcher installs native mods (`<id>.hnmod`), beside [`DIR`]. They run only in a
/// game the launcher built with them.
pub const NATIVE_DIR: &str = "horizon-native-mods";
/// Why a native mod installed in [`NATIVE_DIR`] doesn't run in this game.
pub const NOT_BUILT: &str = "Installed, not built into this game: build the game with it in the launcher (Settings > Native mods), then play the instance that build is in.";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ModList {
    /// Ids of the mods the player turned on.
    pub enabled: BTreeSet<String>,
    /// Ids of the native mods the player turned off: they rest
    /// (`native_on`) until the launcher's next build leaves them out.
    pub disabled: BTreeSet<String>,
    /// The keys the player chose for mods' keys, mod id → key name → key
    /// (`key.<id>.<name>:key.keyboard.x` lines).
    pub keys: BTreeMap<String, BTreeMap<String, String>>,
    /// What the Mods page shows, filled by the game from the mods it found.
    pub installed: Vec<Installed>,
    /// The Java mods of the world's server with a settings screen, `(id, name)`: the Mods
    /// page lists them, and the server opens one's screen (game/java_screen.rs).
    pub java: Vec<(String, String)>,
    /// The Java mods installed for each Java runtime (Fabric 26.3, Fabric 1.20.1), a section
    /// of the Mods page each: what the player put in each runtime's folder.
    pub runtimes: Vec<RuntimeMods>,
}

/// One Java runtime's installed mods, as the Mods page lists them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeMods {
    /// The section's heading, a lang key (`horizon.mods.runtime.fabric_1_20_1`).
    pub title: String,
    pub mods: Vec<RuntimeMod>,
}

/// A Java mod as its jar names it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RuntimeMod {
    pub id: String,
    /// `Name 1.0`.
    pub title: String,
    /// Its fabric.mod.json's description.
    pub description: String,
    /// Its fabric.mod.json's icon, [`RUNTIME_ICON`] pixels square, RGBA.
    pub icon: Option<std::sync::Arc<Vec<u8>>>,
}

/// A recipe of a Java mod's own type (a cooking pot's, a machine's): Horizon has no such type,
/// so it is only shown, by recipe viewers (native JEI). Item ids as the runtime has them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ForeignRecipe {
    /// `farmersdelight:cooking/beef_stew`.
    pub id: String,
    /// Its type, `farmersdelight:cooking`.
    pub kind: String,
    /// Each input slot's items, any one of which fits.
    pub inputs: Vec<Vec<String>>,
    /// Its results: item id and count.
    pub outputs: Vec<(String, u32)>,
    /// The item of the block it's made in, when the runtime could tell (`cooking_pot`).
    pub station: Option<String>,
    /// Its time in ticks, when it keeps one.
    pub ticks: u32,
    /// The experience it gives.
    pub experience: f32,
    /// What its result comes in (a cooking pot's bowl).
    pub container: Option<String>,
}

static FOREIGN_RECIPES: std::sync::RwLock<Option<std::sync::Arc<Vec<ForeignRecipe>>>> =
    std::sync::RwLock::new(None);

/// The Java mods' recipes of their own types, once a runtime has listed them; a new `Arc` each
/// time they change, so a viewer can tell (`Arc::ptr_eq`).
pub fn foreign_recipes() -> Option<std::sync::Arc<Vec<ForeignRecipe>>> {
    FOREIGN_RECIPES
        .read()
        .unwrap_or_else(|e| e.into_inner())
        .clone()
}

/// Sets the Java mods' recipes of their own types (`foreign_recipes`).
pub fn set_foreign_recipes(recipes: Vec<ForeignRecipe>) {
    *FOREIGN_RECIPES.write().unwrap_or_else(|e| e.into_inner()) =
        Some(std::sync::Arc::new(recipes));
}

/// The side of a Java mod's icon as the Mods screen keeps it: the pack icons' slots' size.
pub const RUNTIME_ICON: u32 = crate::hud::PACK_ICON_SIZE;

/// `rgba`, `w` by `h`, scaled to [`RUNTIME_ICON`] square by nearest pixel (mod icons are mostly
/// pixel art).
pub fn runtime_icon(w: u32, h: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    if w == 0 || h == 0 || rgba.len() < (w * h * 4) as usize {
        return None;
    }
    let n = RUNTIME_ICON;
    let mut out = vec![0; (n * n * 4) as usize];
    for y in 0..n {
        for x in 0..n {
            let (sx, sy) = (x * w / n, y * h / n);
            let from = ((sy * w + sx) * 4) as usize;
            let to = ((y * n + x) * 4) as usize;
            out[to..to + 4].copy_from_slice(&rgba[from..from + 4]);
        }
    }
    Some(out)
}

/// One installed mod as the Mods page shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Installed {
    pub id: String,
    /// `Name 1.0` (the file name when the manifest is unreadable).
    pub title: String,
    pub description: String,
    /// Why it is not running, when it failed.
    pub problem: Option<String>,
    /// Loaded with `--dev-mod`: always on.
    pub dev: bool,
    /// A native mod (`game/native_hooks.rs`): built into the game, on until
    /// the player turns it off, and its settings page.
    pub native: Option<NativePage>,
}

/// A native mod's settings page: its keys as Key Binds rows, then its
/// buttons. The keys the player chooses are kept in `ModList::keys`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NativePage {
    /// Each key's name, label and default key (`key.keyboard.z`).
    pub keys: Vec<[String; 3]>,
    pub buttons: Vec<String>,
}

impl ModList {
    pub fn load(dir: &Path) -> Self {
        fs::read_to_string(dir.join(FILE))
            .map(|t| Self::parse(&t))
            .unwrap_or_default()
    }
    pub fn parse(text: &str) -> Self {
        let ids = |value: &str| -> BTreeSet<String> {
            text.lines()
                .filter_map(|l| l.split_once(':'))
                .filter(|(_, v)| v.trim() == value)
                .filter_map(|(k, _)| k.trim().strip_prefix("mod."))
                .map(str::to_owned)
                .collect()
        };
        let (enabled, disabled) = (ids("true"), ids("false"));
        let mut keys: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
        for (k, v) in text.lines().filter_map(|l| l.split_once(':')) {
            // Mod ids have no dots; a key's name may.
            if let Some((id, name)) = k
                .trim()
                .strip_prefix("key.")
                .and_then(|r| r.split_once('.'))
            {
                let entry = keys.entry(id.to_owned()).or_default();
                entry.insert(name.to_owned(), v.trim().to_owned());
            }
        }
        Self {
            enabled,
            disabled,
            keys,
            installed: Vec::new(),
            java: Vec::new(),
            runtimes: Vec::new(),
        }
    }
    pub fn serialize(&self) -> String {
        let on = self.enabled.iter().map(|id| format!("mod.{id}:true\n"));
        let off = self.disabled.iter().map(|id| format!("mod.{id}:false\n"));
        let keys = self.keys.iter().flat_map(|(id, keys)| {
            keys.iter()
                .map(move |(name, key)| format!("key.{id}.{name}:{key}\n"))
        });
        on.chain(off).chain(keys).collect()
    }
    /// Mod `id`'s chosen keys, for `Mods::set_bindings`.
    pub fn keys_of(&self, id: &str) -> Vec<(&str, &str)> {
        self.keys.get(id).map_or_else(Vec::new, |k| {
            k.iter().map(|(n, k)| (n.as_str(), k.as_str())).collect()
        })
    }
    pub fn save(&self, dir: &Path) {
        let path = dir.join(FILE);
        if self.enabled.is_empty()
            && self.disabled.is_empty()
            && self.keys.is_empty()
            && !path.exists()
        {
            return;
        }
        let _ = fs::write(path, self.serialize());
    }
    pub fn is_on(&self, i: usize) -> bool {
        self.installed.get(i).is_some_and(|m| {
            m.dev
                || if m.native.is_some() {
                    !self.disabled.contains(&m.id)
                } else {
                    self.enabled.contains(&m.id)
                }
        })
    }
    /// Turns installed mod `i` on or off (a dev mod stays on). A native mod's
    /// line says `false` when off, so that the launcher's next build leaves
    /// it out, and `true` when on again.
    /// Turn the installed mod `id` on (nothing when it isn't installed).
    pub fn turn_on(&mut self, id: &str) {
        let Some(m) = self.installed.iter().find(|m| m.id == id && !m.dev) else {
            return;
        };
        if m.native.is_some() {
            self.disabled.remove(&m.id);
        }
        self.enabled.insert(m.id.clone());
    }
    pub fn toggle(&mut self, i: usize) {
        let Some(m) = self.installed.get(i).filter(|m| !m.dev) else {
            return;
        };
        if m.native.is_some() {
            if self.disabled.remove(&m.id) {
                self.enabled.insert(m.id.clone());
            } else {
                self.enabled.remove(&m.id);
                self.disabled.insert(m.id.clone());
            }
        } else if !self.enabled.remove(&m.id) {
            self.enabled.insert(m.id.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_ignores_the_rest() {
        let list = ModList::parse("mod.coords_hud:true\nmod.other:false\nzoom:true\n");
        assert_eq!(list.enabled.iter().collect::<Vec<_>>(), ["coords_hud"]);
        assert_eq!(ModList::parse(&list.serialize()), list);
        assert_eq!(ModList::default().serialize(), "");
    }

    #[test]
    fn keeps_the_keys_players_chose() {
        let text = "mod.zoom:true\nkey.zoom.zoom.in:key.mouse.4\nkey.zoom.out:key.keyboard.x\n";
        let list = ModList::parse(text);
        assert_eq!(
            list.keys_of("zoom"),
            [("out", "key.keyboard.x"), ("zoom.in", "key.mouse.4")]
        );
        assert!(list.keys_of("other").is_empty());
        assert_eq!(ModList::parse(&list.serialize()), list);
    }

    #[test]
    fn toggles_installed_and_native_mods_but_not_dev_ones() {
        let installed = |id: &str, dev| Installed {
            id: id.into(),
            title: id.into(),
            description: String::new(),
            problem: None,
            dev,
            native: None,
        };
        let mut list = ModList {
            installed: vec![installed("a", false), installed("b", true)],
            ..Default::default()
        };
        assert!(!list.is_on(0) && list.is_on(1));
        list.toggle(0);
        list.toggle(1);
        assert!(list.is_on(0) && list.is_on(1));
        assert_eq!(list.serialize(), "mod.a:true\n");
        list.toggle(0);
        assert!(!list.is_on(0));
        list.installed.push(Installed {
            native: Some(NativePage::default()),
            ..installed("c", false)
        });
        // On until turned off; off, the launcher leaves it out.
        assert!(list.is_on(2));
        list.toggle(2);
        assert!(!list.is_on(2));
        assert_eq!(list.serialize(), "mod.c:false\n");
        assert_eq!(ModList::parse(&list.serialize()).disabled, list.disabled);
        list.toggle(2);
        assert!(list.is_on(2));
        assert_eq!(list.serialize(), "mod.c:true\n");
    }
}
