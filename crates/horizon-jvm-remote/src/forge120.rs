//! Forge 1.20.1 with unmodified mods, in a JVM of its own beside the Fabric one
//! (`docs/java-bridge-120-forge.md`, phase 4). Forge's client is started as its installer's
//! launch arguments start it (`tools/java_bridge/fetch120forge.py` wrote them to
//! `forge/launch.json`), with the bridge's platform (`tools/java_bridge/build120forge.sh`'s
//! `platform120-forge.jar`) among the mods. Once every mod has loaded, the platform connects
//! back (`Child.serveInGame`) and answers Horizon's calls as the Fabric runtime does, so
//! [`start`] gives the same [`Runtime120`].
//!
//! Forge is run as it ships, never read or shipped: the player's machine fetches it.

use crate::boot120::Runtime120;
use crate::child::{self, Log};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// What `ForgeMod120` says it runs.
pub const RUNTIME: &str = "dev.horizonlab.platform120.ForgeRuntime120";

/// A Java Forge 1.20.1 runs on (its ModLauncher reads classes up to Java 21's):
/// `HORIZON_FORGE_JAVA_HOME`, else an installed 21 or 17, else the one the Fabric runtime uses.
pub fn find_java() -> PathBuf {
    if let Some(home) = std::env::var_os("HORIZON_FORGE_JAVA_HOME") {
        return PathBuf::from(home).join("bin").join("java");
    }
    for version in ["21", "17"] {
        for dir in [
            format!("/usr/lib/jvm/java-{version}-openjdk"),
            format!("/usr/lib/jvm/java-{version}-openjdk-amd64"),
            format!("/usr/lib/jvm/jdk-{version}"),
        ] {
            let java = Path::new(&dir).join("bin").join("java");
            if java.exists() {
                return java;
            }
        }
    }
    crate::boot120::find_java()
}

/// What [`start`] runs.
pub struct Forge120 {
    /// A Java Forge 1.20.1 runs on: 17 to 21.
    pub java: PathBuf,
    /// `fetch120.py`'s folder for 1.20.1, with `fetch120forge.py`'s `forge/` in it.
    pub install: PathBuf,
    /// The Forge game folder: `mods/`, `config/`; the socket's folder.
    pub game_dir: PathBuf,
    /// The bridge's platform as a Forge mod; copied into `<game_dir>/mods`.
    pub platform: PathBuf,
    /// The mods' jars; copied into `<game_dir>/mods`, in place of the jars there before.
    pub mods: Vec<PathBuf>,
    /// The child's heap (`-Xmx`).
    pub max_heap: String,
    pub jvm_args: Vec<String>,
    /// How long the boot (Forge's whole client start) and any later call may take.
    pub timeout: Duration,
    pub log: Log,
}

/// `forge/launch.json`.
#[derive(Debug, PartialEq)]
pub struct Launch {
    pub main_class: String,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
    pub class_path: Vec<PathBuf>,
    pub version_id: String,
}

/// Reads `install/forge/launch.json`.
pub fn launch(install: &Path) -> Result<Launch, String> {
    let file = install.join("forge").join("launch.json");
    let text = std::fs::read_to_string(&file).map_err(|e| {
        format!(
            "{}: {e} (run tools/java_bridge/fetch120forge.py first)",
            file.display()
        )
    })?;
    parse_launch(&text)
}

fn parse_launch(text: &str) -> Result<Launch, String> {
    let json: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("forge launch.json: {e}"))?;
    let strings = |key: &str| -> Vec<String> {
        json[key]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    let text = |key: &str| {
        json[key]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("forge launch.json has no {key}"))
    };
    let class_path: Vec<PathBuf> = strings("class_path")
        .into_iter()
        .map(PathBuf::from)
        .collect();
    if class_path.is_empty() {
        return Err("forge launch.json has no class path".into());
    }
    Ok(Launch {
        main_class: text("main_class")?,
        jvm_args: strings("jvm_args"),
        game_args: strings("game_args"),
        class_path,
        version_id: text("version_id")?,
    })
}

/// The game's arguments after Forge's own: the folders, and an offline player as a launcher
/// passes one. No assets: Horizon draws and sounds the game, so an empty index is enough.
pub fn game_args(launch: &Launch, game_dir: &Path) -> Vec<String> {
    let mut args = launch.game_args.clone();
    let dir = game_dir.display().to_string();
    let assets = game_dir.join("assets").display().to_string();
    for (k, v) in [
        ("--gameDir", dir.as_str()),
        ("--assetsDir", assets.as_str()),
        ("--assetIndex", "horizon"),
        ("--version", launch.version_id.as_str()),
        ("--accessToken", "0"),
        ("--username", "Horizon"),
        ("--uuid", "00000000000000000000000000000000"),
        ("--userType", "legacy"),
    ] {
        args.push(k.into());
        args.push(v.into());
    }
    args
}

/// The game folder as Forge's client needs it: the platform among the mods, an empty asset
/// index, and Forge's early window off (the window Horizon hides comes later; the early one
/// would flash up first).
pub fn prepare(game_dir: &Path, platform: &Path, jars: &[PathBuf]) -> Result<(), String> {
    let io = |p: &Path, e: std::io::Error| format!("{}: {e}", p.display());
    let mods = game_dir.join("mods");
    std::fs::create_dir_all(&mods).map_err(|e| io(&mods, e))?;
    // The mods folder is the game's own copy: what was there from another set of mods goes.
    for old in std::fs::read_dir(&mods)
        .map_err(|e| io(&mods, e))?
        .flatten()
    {
        if old.path().extension().is_some_and(|e| e == "jar") {
            let _ = std::fs::remove_file(old.path());
        }
    }
    for jar in jars {
        let to = mods.join(jar.file_name().ok_or("a jar without a name")?);
        std::fs::copy(jar, &to).map_err(|e| io(&to, e))?;
    }
    let to = mods.join("horizon-platform120-forge.jar");
    std::fs::copy(platform, &to).map_err(|e| io(&to, e))?;
    let index = game_dir.join("assets/indexes/horizon.json");
    if !index.exists() {
        std::fs::create_dir_all(index.parent().unwrap_or(game_dir)).map_err(|e| io(&index, e))?;
        std::fs::write(&index, "{\"objects\":{}}").map_err(|e| io(&index, e))?;
    }
    let fml = game_dir.join("config/fml.toml");
    if !fml.exists() {
        std::fs::create_dir_all(fml.parent().unwrap_or(game_dir)).map_err(|e| io(&fml, e))?;
        std::fs::write(&fml, "earlyWindowControl = false\n").map_err(|e| io(&fml, e))?;
    }
    Ok(())
}

/// Starts Forge's client with the mods and waits for the platform to connect. Returns the
/// runtime and its report (JSON: the mods Forge loaded).
pub fn start(config: &Forge120) -> Result<(Runtime120, String), String> {
    let launch = launch(&config.install)?;
    prepare(&config.game_dir, &config.platform, &config.mods)?;
    let mut jvm_args = vec![format!("-Xmx{}", config.max_heap)];
    jvm_args.extend(launch.jvm_args.iter().cloned());
    jvm_args.extend(config.jvm_args.iter().cloned());
    let child_config = child::Config {
        name: "forge-1.20.1".into(),
        java: config.java.clone(),
        jvm_args,
        class_path: launch.class_path.clone(),
        runtime: RUNTIME.into(),
        main: Some((
            launch.main_class.clone(),
            game_args(&launch, &config.game_dir),
        )),
        runtime_args: Vec::new(),
        dir: config.game_dir.clone(),
        tcp: false,
        // The whole client start, every mod's loading, comes before the platform connects.
        connect_timeout: config.timeout,
        read_timeout: Some(config.timeout),
        log: config.log.clone(),
    };
    let child = child::spawn(&child_config)?;
    let mut runtime = Runtime120 {
        child,
        log: config.log.clone(),
        loader: crate::boot120::Loader::Forge,
    };
    let report = runtime.call("start")?;
    Ok((runtime, report))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_launch_is_read_and_the_game_gets_its_folders() {
        let launch = parse_launch(
            r#"{"main_class": "cpw.mods.bootstraplauncher.BootstrapLauncher",
                "jvm_args": ["-p", "/l/a.jar"], "game_args": ["--launchTarget", "forgeclient"],
                "class_path": ["/l/b.jar"], "version_id": "1.20.1-forge-47.4.28"}"#,
        )
        .unwrap();
        assert_eq!(
            launch.main_class,
            "cpw.mods.bootstraplauncher.BootstrapLauncher"
        );
        let args = game_args(&launch, Path::new("/g"));
        assert_eq!(&args[..2], ["--launchTarget", "forgeclient"]);
        let at = |k: &str| args[args.iter().position(|a| a == k).unwrap() + 1].clone();
        assert_eq!(at("--gameDir"), "/g");
        assert_eq!(at("--version"), "1.20.1-forge-47.4.28");
        assert!(parse_launch(r#"{"main_class": "m", "version_id": "v"}"#).is_err());
    }
}
