//! Forge 1.12.2 through Cleanroom in place of Fabric (phase 1 of `docs/java-bridge-112.md`):
//! the JVM gets Cleanroom's class path (its libraries, Mojang's 1.12.2 client jar) as
//! `tools/java_bridge/fetch112.py` laid it out (`launch.json`), and `Boot112.start` runs
//! Cleanroom's launcher on a thread of its own. That thread becomes 1.12's client thread: a
//! hidden window (SDL's offscreen driver) with a real GL context, every mod loaded, then the
//! platform mod (`java/platform112/mod`, built by `tools/java_bridge/build112.sh`) keeps it and
//! answers Horizon's commands. No world, no server yet: [`start`] returns the platform's
//! report (mods, blocks with their stored states, items with subtypes, fluids, entities,
//! biomes, dimensions, FML's numeric ids).
//!
//! Cleanroom is source-available (CleanroomMC License 1.0): it is downloaded by the player's
//! own machine from Cleanroom's maven, run as it is, never redistributed, and nothing here
//! comes from its source (only its installer's launch files and what it does when run).

use crate::{BRIDGE, Bridge, STARTING, boot, create_vm, describe, strings, write_jar};
use jni::JNIEnv;
use jni::objects::{JString, JValue};
use std::path::{Path, PathBuf};

/// The game-folder-relative place of the platform mod, handed to FML with `--mods` (FML
/// resolves it against the game folder), so the player's `mods/` holds only their own.
pub const PLATFORM_IN_GAME_DIR: &str = "horizon-java-112/horizon112-platform.jar";

/// What [`start`] runs.
pub struct Cleanroom {
    /// The Java home to load the JVM from (Java 25).
    pub home: PathBuf,
    /// The bridge's own files (its jar, the log config).
    pub work: PathBuf,
    /// The folder `fetch112.py` filled: `launch.json`, `libraries/`, `natives/`, `assets/`,
    /// and `horizon/horizon112-platform.jar` from `build112.sh`.
    pub install: PathBuf,
    /// The 1.12 game folder: `mods/`, `config/`; FML writes there.
    pub game_dir: PathBuf,
}

/// Cleanroom's launch, as `fetch112.py` wrote it from the installer's `version.json`.
#[derive(Debug, PartialEq)]
pub struct Launch {
    pub class_path: Vec<PathBuf>,
    pub main_class: String,
    pub tweakers: Vec<String>,
    pub natives: PathBuf,
    pub assets: PathBuf,
    pub asset_index: String,
}

/// Reads `install/launch.json`.
pub fn launch(install: &Path) -> Result<Launch, String> {
    let file = install.join("launch.json");
    let text = std::fs::read_to_string(&file).map_err(|e| {
        format!(
            "{}: {e} (run tools/java_bridge/fetch112.py first)",
            file.display()
        )
    })?;
    parse_launch(&text)
}

fn parse_launch(text: &str) -> Result<Launch, String> {
    let json: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("launch.json: {e}"))?;
    let string = |key: &str| {
        json[key]
            .as_str()
            .map(str::to_string)
            .ok_or_else(|| format!("launch.json has no {key}"))
    };
    let list = |key: &str| -> Vec<String> {
        json[key]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };
    let class_path: Vec<PathBuf> = list("class_path").into_iter().map(PathBuf::from).collect();
    if class_path.is_empty() {
        return Err("launch.json has no class path".into());
    }
    Ok(Launch {
        class_path,
        main_class: string("main_class")?,
        tweakers: list("tweakers"),
        natives: PathBuf::from(string("natives")?),
        assets: PathBuf::from(string("assets")?),
        asset_index: string("asset_index")?,
    })
}

/// The launcher's arguments: what a launcher passes Minecraft 1.12.2 (offline), FML's client
/// tweaker (the client: Forge strips `@SideOnly(CLIENT)` code on a server, and the mods'
/// screens and models are needed), and the platform mod.
pub fn args(launch: &Launch, game_dir: &Path) -> Vec<String> {
    let mut args = vec![
        "--username".to_string(),
        "Horizon".into(),
        "--version".into(),
        "1.12.2".into(),
        "--gameDir".into(),
        game_dir.to_string_lossy().into_owned(),
        "--assetsDir".into(),
        launch.assets.to_string_lossy().into_owned(),
        "--assetIndex".into(),
        launch.asset_index.clone(),
        "--accessToken".into(),
        "0".into(),
        "--userType".into(),
        "legacy".into(),
    ];
    for tweaker in &launch.tweakers {
        args.push("--tweakClass".into());
        args.push(tweaker.clone());
    }
    args.push("--mods".into());
    args.push(PLATFORM_IN_GAME_DIR.into());
    args
}

/// The game folder's files the hidden boot needs: the platform mod, and FML's splash screen
/// off (it is a second GL thread, and it waits for a click when mods are missing).
fn prepare_game_dir(config: &Cleanroom) -> Result<(), String> {
    let platform = config.install.join("horizon/horizon112-platform.jar");
    let bytes = std::fs::read(&platform).map_err(|e| {
        format!(
            "{}: {e} (run tools/java_bridge/build112.sh)",
            platform.display()
        )
    })?;
    let target = config.game_dir.join(PLATFORM_IN_GAME_DIR);
    let dir = target.parent().unwrap_or(&config.game_dir);
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    if std::fs::read(&target).ok().as_deref() != Some(&bytes[..]) {
        std::fs::write(&target, &bytes).map_err(|e| format!("{}: {e}", target.display()))?;
    }
    let config_dir = config.game_dir.join("config");
    std::fs::create_dir_all(&config_dir).map_err(|e| format!("{}: {e}", config_dir.display()))?;
    let splash = config_dir.join("splash.properties");
    let text = std::fs::read_to_string(&splash).unwrap_or_default();
    if !text.lines().any(|l| l.trim() == "enabled=false") {
        let mut lines: Vec<&str> = text
            .lines()
            .filter(|l| !l.trim_start().starts_with("enabled="))
            .collect();
        lines.push("enabled=false");
        std::fs::write(&splash, lines.join("\n") + "\n")
            .map_err(|e| format!("{}: {e}", splash.display()))?;
    }
    Ok(())
}

/// Starts the JVM with Cleanroom's class path and boots Forge 1.12.2 hidden with the mods in
/// `game_dir/mods`. Returns the running bridge and the platform's report (JSON).
pub fn start(config: &Cleanroom) -> Result<(&'static Bridge, String), String> {
    let _starting = STARTING.lock().unwrap_or_else(|e| e.into_inner());
    if BRIDGE.get().is_some() {
        return Err("the JVM already runs (one per process): 1.12 needs a world of its own".into());
    }
    let launch = launch(&config.install)?;
    prepare_game_dir(config)?;
    // The hidden window: SDL draws offscreen (EGL), so no window shows and none is needed
    // (servers, CI); OpenAL opens its null device (the game plays its own sound). Set before
    // the JVM starts, while nothing else reads them; a player's own values win.
    for (var, value) in [
        ("SDL_VIDEO_DRIVER", "offscreen"),
        ("ALSOFT_DRIVERS", "null"),
    ] {
        if std::env::var_os(var).is_none() {
            // SAFETY: before the JVM exists; the game reads neither variable.
            unsafe { std::env::set_var(var, value) };
        }
    }
    let bridge_jar = write_jar(&config.work, "horizon-bridge.jar", crate::BRIDGE_JAR)?;
    let mut class_path = vec![bridge_jar];
    class_path.extend(launch.class_path.iter().cloned());
    let log_config = write_jar(
        &config.work,
        "log4j2-horizon.xml",
        crate::LOG4J_CONFIG.as_bytes(),
    )?;
    let natives = crate::option_path(&launch.natives);
    let options = [
        format!("-Djava.library.path={}", natives.display()),
        format!("-Dorg.lwjgl.librarypath={}", natives.display()),
    ];
    let vm = create_vm(&config.home, &class_path, None, &log_config, &options)?;
    let mut env = vm
        .attach_current_thread_permanently()
        .map_err(|e| format!("attach: {e}"))?;
    let java_version = boot(&mut env, &[]).map_err(|e| describe(&mut env, e))?;
    let report = boot112(
        &mut env,
        &launch.main_class,
        &args(&launch, &config.game_dir),
    )
    .map_err(|e| describe(&mut env, e))?;
    let mods = report_mods(&report);
    let bridge = BRIDGE.get_or_init(|| Bridge {
        vm,
        java_version,
        home: config.home.clone(),
        mods,
        fabric: false,
        class_archive: None,
    });
    Ok((bridge, report))
}

/// Runs a platform command on 1.12's client thread (`report`, `ping`).
pub fn call(bridge: &Bridge, command: &str) -> Result<String, String> {
    let mut env = bridge
        .vm()
        .attach_current_thread()
        .map_err(|e| format!("attach: {e}"))?;
    let result = (|| {
        let command = env.new_string(command)?;
        let out = env
            .call_static_method(
                "dev/horizonlab/bridge/c112/Boot112",
                "call",
                "(Ljava/lang/String;)Ljava/lang/String;",
                &[JValue::Object(&command)],
            )?
            .l()?;
        let out = JString::from(out);
        let text: String = env.get_string(&out)?.into();
        Ok(text)
    })();
    result.map_err(|e| describe(&mut env, e))
}

fn boot112(env: &mut JNIEnv, main_class: &str, args: &[String]) -> jni::errors::Result<String> {
    let main_class = env.new_string(main_class)?;
    let args = strings(env, args)?;
    let report = env
        .call_static_method(
            "dev/horizonlab/bridge/c112/Boot112",
            "start",
            "(Ljava/lang/String;[Ljava/lang/String;)Ljava/lang/String;",
            &[JValue::Object(&main_class), JValue::Object(&args)],
        )?
        .l()?;
    let report = JString::from(report);
    let text: String = env.get_string(&report)?.into();
    Ok(text)
}

/// `id version` of each mod in the report, as [`Bridge::mods`] lists Fabric's.
fn report_mods(report: &str) -> Vec<String> {
    let Ok(json) = serde_json::from_str::<serde_json::Value>(report) else {
        return Vec::new();
    };
    json["mods"]
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(id, v)| format!("{id} {}", v.as_str().unwrap_or("")))
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_launch_json_and_builds_the_arguments() {
        let text = r#"{"cleanroom": "0.7.14-alpha", "main_class": "top.outlands.foundation.boot.Foundation",
            "tweakers": ["net.minecraftforge.fml.common.launcher.FMLTweaker"],
            "class_path": ["/c/libraries/a.jar", "/c/versions/1.12.2.jar"],
            "natives": "/c/natives", "assets": "/c/assets", "asset_index": "1.12"}"#;
        let launch = parse_launch(text).unwrap();
        assert_eq!(launch.class_path.len(), 2);
        assert_eq!(launch.main_class, "top.outlands.foundation.boot.Foundation");
        let args = args(&launch, Path::new("/g"));
        let after = |flag: &str| {
            let i = args.iter().position(|a| a == flag).unwrap();
            args[i + 1].as_str()
        };
        assert_eq!(after("--gameDir"), "/g");
        assert_eq!(after("--assetIndex"), "1.12");
        assert_eq!(
            after("--tweakClass"),
            "net.minecraftforge.fml.common.launcher.FMLTweaker"
        );
        assert_eq!(after("--mods"), PLATFORM_IN_GAME_DIR);
        assert!(parse_launch(r#"{"main_class": "x", "class_path": []}"#).is_err());
    }

    #[test]
    fn mods_come_from_the_report() {
        let mods =
            report_mods(r#"{"start_ms": 1, "mods": {"forge": "14.23.5.2864", "hello112": "1.0"}}"#);
        assert_eq!(mods, ["forge 14.23.5.2864", "hello112 1.0"]);
    }
}
