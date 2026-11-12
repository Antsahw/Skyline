//! Fabric 1.20.1 with unmodified mods, in a JVM of its own (phase 1 of
//! `docs/java-bridge-120.md`): the child gets a Fabric launcher profile's class path for 1.20.1
//! (`tools/java_bridge/fetch120.py` wrote it to `launch.json`) and the bridge's classes,
//! `remote.Child` connects back, and `Boot120` (`java/dev/horizonlab/bridge/r120/`) boots
//! Fabric Loader's own Minecraft provider (it remaps Mojang's jar to intermediary), the
//! registries, a headless client and every mod's entrypoints. [`start`] returns the running
//! runtime and its report (mods and how their entrypoints went, blocks, items, entities,
//! biomes, dimensions, timings).
//!
//! Mojang's files are fetched by the player's own machine and never committed or shipped.

use crate::child::{self, Child, Log};
use crate::peer::{Handler, Peer};
use crate::wire::Value;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The class `remote.Child` runs.
pub const RUNTIME: &str = "dev.horizonlab.bridge.r120.Boot120";

/// Which entrypoints the mods run: `client` (with a headless client) or `server`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Client,
    Server,
}

/// What [`start`] runs.
pub struct Fabric120 {
    /// The `java` executable ([`find_java`]).
    pub java: PathBuf,
    /// `fetch120.py`'s folder for 1.20.1: `launch.json` and what it lists.
    pub install: PathBuf,
    /// The bridge's classes: a jar or a folder with `dev/horizonlab/bridge/{remote,r120}`.
    pub bridge: PathBuf,
    /// The 1.20.1 game folder: `config/`, Fabric's remapped jar cache; the socket's folder.
    pub game_dir: PathBuf,
    /// The mods (default `<game_dir>/mods`).
    pub mods: Option<PathBuf>,
    /// The bridge's 1.20.1 platform mod (`tools/java_bridge/build120.sh`'s `platform120.jar`):
    /// what writes the content facts. Without it the runtime boots and reports, no more.
    pub platform: Option<PathBuf>,
    pub side: Side,
    /// The child's heap (`-Xmx`), e.g. "3G".
    pub max_heap: String,
    /// More JVM options.
    pub jvm_args: Vec<String>,
    /// How long the boot (or any later call) may take before the runtime is given up and
    /// stopped. A big pack boots in a minute or two; a mod stuck forever must not hang Horizon.
    pub timeout: Duration,
    pub log: Log,
}

/// `launch.json`.
#[derive(Debug, PartialEq)]
pub struct Launch {
    pub class_path: Vec<PathBuf>,
    pub game_jar: PathBuf,
    pub mojang_mappings: PathBuf,
    pub fabric_api: PathBuf,
    pub loader_version: String,
    pub java_major: i64,
}

/// Reads `install/launch.json`.
pub fn launch(install: &Path) -> Result<Launch, String> {
    let file = install.join("launch.json");
    let text = std::fs::read_to_string(&file).map_err(|e| {
        format!(
            "{}: {e} (run tools/java_bridge/fetch120.py first)",
            file.display()
        )
    })?;
    parse_launch(&text)
}

fn parse_launch(text: &str) -> Result<Launch, String> {
    let json: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("launch.json: {e}"))?;
    let path = |key: &str| {
        json[key]
            .as_str()
            .map(PathBuf::from)
            .ok_or_else(|| format!("launch.json has no {key}"))
    };
    let class_path: Vec<PathBuf> = json["class_path"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(PathBuf::from))
                .collect()
        })
        .unwrap_or_default();
    if class_path.is_empty() {
        return Err("launch.json has no class path".into());
    }
    Ok(Launch {
        class_path,
        game_jar: path("game_jar")?,
        mojang_mappings: path("mojang_mappings")?,
        fabric_api: path("fabric_api")?,
        loader_version: json["loader_version"].as_str().unwrap_or("").to_string(),
        java_major: json["java_major"].as_i64().unwrap_or(25),
    })
}

/// `Boot120`'s arguments.
pub fn runtime_args(config: &Fabric120, launch: &Launch) -> Vec<String> {
    let mods = config
        .mods
        .clone()
        .unwrap_or_else(|| config.game_dir.join("mods"));
    // Fabric API comes with the runtime unless the player's mods hold their own.
    let own_fabric_api = std::fs::read_dir(&mods)
        .map(|d| {
            d.filter_map(|e| e.ok())
                .any(|e| e.file_name().to_string_lossy().starts_with("fabric-api-"))
        })
        .unwrap_or(false);
    let mut args = vec![
        "--game-jar".to_string(),
        launch.game_jar.to_string_lossy().into_owned(),
        "--mappings".into(),
        launch.mojang_mappings.to_string_lossy().into_owned(),
        "--game-dir".into(),
        config.game_dir.to_string_lossy().into_owned(),
        "--mods".into(),
        mods.to_string_lossy().into_owned(),
        "--side".into(),
        match config.side {
            Side::Client => "client",
            Side::Server => "server",
        }
        .into(),
    ];
    let mut add = Vec::new();
    if !own_fabric_api {
        add.push(launch.fabric_api.clone());
    }
    add.extend(config.platform.iter().cloned());
    if !add.is_empty() {
        args.push("--add-mods".into());
        args.push(
            std::env::join_paths(&add)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
        );
    }
    args
}

/// The Java to run the child with: `HORIZON_JAVA_HOME`, `JAVA_HOME`, the system's default
/// (`/usr/lib/jvm/default` on Arch), then `java` on the PATH.
pub fn find_java() -> PathBuf {
    let exe = if cfg!(windows) { "java.exe" } else { "java" };
    for var in ["HORIZON_JAVA_HOME", "JAVA_HOME"] {
        if let Some(home) = std::env::var_os(var) {
            let java = PathBuf::from(home).join("bin").join(exe);
            if java.exists() {
                return java;
            }
        }
    }
    let default = Path::new("/usr/lib/jvm/default/bin/java");
    if default.exists() {
        return default.to_path_buf();
    }
    PathBuf::from(exe)
}

/// The Java version of a `java` executable, from its home's `release` file (symlinks
/// followed: `/usr/lib/jvm/default/bin/java` on Arch); `None` when there is none.
pub fn java_major(java: &Path) -> Option<i64> {
    let java = std::fs::canonicalize(java).ok()?;
    let text = std::fs::read_to_string(java.parent()?.parent()?.join("release")).ok()?;
    let line = text.lines().find(|l| l.starts_with("JAVA_VERSION="))?;
    let version = line["JAVA_VERSION=".len()..].trim_matches('"');
    let major = version.split(['.', '+', '-']).next()?;
    // "1.8.0_202" is Java 8.
    match major.parse::<i64>().ok()? {
        1 => version.split('.').nth(1)?.parse().ok(),
        n => Some(n),
    }
}

/// What 1.20.1's libraries need from a newer Java to run quietly, as launchers pass it:
/// LWJGL loads native libraries (restricted since 22), JOML and others use `sun.misc.Unsafe`'s
/// memory methods (they warn from 24). Older Javas reject these options, so only for theirs.
pub fn new_java_options(major: Option<i64>) -> Vec<String> {
    let mut options = Vec::new();
    let major = major.unwrap_or(0);
    if major >= 22 {
        options.push("--enable-native-access=ALL-UNNAMED".to_string());
    }
    if major >= 24 {
        options.push("--sun-misc-unsafe-memory-access=allow".to_string());
    }
    options
}

/// A running 1.20.1 runtime.
pub struct Runtime120 {
    pub child: Child,
    pub(crate) log: Log,
}

/// Horizon's side of the conversation in phase 1: the runtime's log lines; no world yet, so
/// a call into Horizon is refused (and fails only the mod code that made it).
struct Phase1 {
    log: Log,
}

impl Handler for Phase1 {
    fn call(&mut self, _: &mut Peer, method: &str, _: Vec<Value>) -> Result<Vec<Value>, String> {
        Err(format!("Horizon has no {method} yet (phase 1: no world)"))
    }

    fn event(&mut self, _: &mut Peer, method: &str, args: Vec<Value>) {
        if method == "log"
            && let Some(line) = args.first().and_then(Value::as_str)
        {
            (self.log)(&format!("[fabric-1.20.1] {line}"));
        }
    }
}

/// Starts the 1.20.1 runtime and boots it. Returns it and its report (JSON).
pub fn start(config: &Fabric120) -> Result<(Runtime120, String), String> {
    let launch = launch(&config.install)?;
    let mut class_path = vec![config.bridge.clone()];
    class_path.extend(launch.class_path.iter().cloned());
    let mut jvm_args = vec![format!("-Xmx{}", config.max_heap)];
    jvm_args.extend(new_java_options(java_major(&config.java)));
    jvm_args.extend(config.jvm_args.iter().cloned());
    let child_config = child::Config {
        name: "fabric-1.20.1".into(),
        java: config.java.clone(),
        jvm_args,
        class_path,
        runtime: RUNTIME.into(),
        runtime_args: runtime_args(config, &launch),
        dir: config.game_dir.clone(),
        tcp: false,
        connect_timeout: Duration::from_secs(60),
        read_timeout: Some(config.timeout),
        log: config.log.clone(),
    };
    let child = child::spawn(&child_config)?;
    let mut runtime = Runtime120 {
        child,
        log: config.log.clone(),
    };
    let report = runtime.call("start")?;
    Ok((runtime, report))
}

impl Runtime120 {
    /// Runs a command in the runtime (`report`, `ping`) and returns its answer.
    pub fn call(&mut self, command: &str) -> Result<String, String> {
        self.call_with(command, &[])
    }

    /// Runs a command with arguments (`facts` and the file to write) and returns its answer.
    pub fn call_with(&mut self, command: &str, args: &[Value]) -> Result<String, String> {
        let mut handler = Phase1 {
            log: self.log.clone(),
        };
        let values = self.call_values(command, args, &mut handler)?;
        values
            .first()
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("{command}: no text in the answer"))
    }

    /// A call that asks Horizon nothing back, its answers as they come.
    pub(crate) fn call_answers(
        &mut self,
        command: &str,
        args: &[Value],
    ) -> Result<Vec<Value>, String> {
        let mut handler = Phase1 {
            log: self.log.clone(),
        };
        self.call_values(command, args, &mut handler)
    }

    /// A call with Horizon's side answered by `handler`; its errors carry the runtime's last lines.
    pub(crate) fn call_values(
        &mut self,
        command: &str,
        args: &[Value],
        handler: &mut dyn Handler,
    ) -> Result<Vec<Value>, String> {
        self.child.call(command, args, handler).map_err(|e| {
            let lines = self.child.last_lines();
            format!(
                "{command}: {e}; the runtime's last lines:\n{}",
                lines.join("\n")
            )
        })
    }

    /// Says goodbye and waits for the process to end.
    pub fn stop(mut self) {
        self.child.stop(Duration::from_secs(10));
    }
}

/// `id version` of each mod in a report, sorted.
pub fn report_mods(report: &str) -> Vec<String> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(report) else {
        return Vec::new();
    };
    json["mods"]
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(id, v)| format!("{id} {}", v["version"].as_str().unwrap_or("")))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(game: &Path) -> Fabric120 {
        Fabric120 {
            java: "java".into(),
            install: "/c".into(),
            bridge: "/b".into(),
            game_dir: game.to_path_buf(),
            mods: None,
            platform: None,
            side: Side::Client,
            max_heap: "2G".into(),
            jvm_args: vec![],
            timeout: Duration::from_secs(600),
            log: std::sync::Arc::new(|_| {}),
        }
    }

    const LAUNCH: &str = r#"{"minecraft": "1.20.1", "class_path": ["/c/fabric/fabric-loader-0.20.1.jar", "/c/versions/1.20.1.jar"],
        "game_jar": "/c/versions/1.20.1.jar", "mojang_mappings": "/c/versions/client.txt",
        "fabric_api": "/c/fabric/fabric-api-0.92.12+1.20.1.jar", "loader_version": "0.20.1", "java_major": 25}"#;

    #[test]
    fn fabric_api_comes_along_unless_the_mods_hold_their_own() {
        let game = std::env::temp_dir().join(format!("boot120-args-{}", std::process::id()));
        let mods = game.join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        let launch = parse_launch(LAUNCH).unwrap();
        let args = runtime_args(&config(&game), &launch);
        let after = |args: &[String], flag: &str| {
            args.iter()
                .position(|a| a == flag)
                .map(|i| args[i + 1].clone())
        };
        assert_eq!(
            after(&args, "--add-mods").as_deref(),
            Some("/c/fabric/fabric-api-0.92.12+1.20.1.jar")
        );
        assert_eq!(
            after(&args, "--mods"),
            Some(mods.to_string_lossy().into_owned())
        );
        assert_eq!(after(&args, "--side").as_deref(), Some("client"));
        std::fs::write(mods.join("fabric-api-0.92.2+1.20.1.jar"), b"").unwrap();
        let args = runtime_args(&config(&game), &launch);
        assert_eq!(after(&args, "--add-mods"), None);
        std::fs::remove_dir_all(&game).unwrap();
        assert!(parse_launch(r#"{"game_jar": "x", "class_path": []}"#).is_err());
    }

    #[test]
    fn newer_javas_get_the_options_launchers_pass_and_older_ones_none() {
        assert!(new_java_options(Some(17)).is_empty());
        assert!(new_java_options(None).is_empty());
        assert_eq!(
            new_java_options(Some(22)),
            ["--enable-native-access=ALL-UNNAMED"]
        );
        assert_eq!(
            new_java_options(Some(25)),
            [
                "--enable-native-access=ALL-UNNAMED",
                "--sun-misc-unsafe-memory-access=allow"
            ]
        );
    }

    #[test]
    fn java_versions_come_from_the_release_file() {
        let home = std::env::temp_dir().join(format!("boot120-java-{}", std::process::id()));
        std::fs::create_dir_all(home.join("bin")).unwrap();
        std::fs::write(home.join("bin/java"), b"").unwrap();
        for (release, major) in [
            ("25.0.1", 25),
            ("17.0.15", 17),
            ("1.8.0_202", 8),
            ("21", 21),
        ] {
            std::fs::write(
                home.join("release"),
                format!("IMPLEMENTOR=\"x\"\nJAVA_VERSION=\"{release}\"\n"),
            )
            .unwrap();
            assert_eq!(java_major(&home.join("bin/java")), Some(major), "{release}");
        }
        std::fs::remove_dir_all(&home).unwrap();
        assert_eq!(java_major(Path::new("/nonexistent/bin/java")), None);
    }

    #[test]
    fn mods_come_from_the_report() {
        let mods = report_mods(
            r#"{"mods": {"fabricloader": {"version": "0.20.1"}, "lithium": {"version": "0.11.4", "name": "Lithium"}}}"#,
        );
        assert_eq!(mods, ["fabricloader 0.20.1", "lithium 0.11.4"]);
    }
}
