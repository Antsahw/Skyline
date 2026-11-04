//! NeoForge in place of Fabric (prototype, `docs/java-bridge-neoforge.md`): the JVM gets
//! FancyModLoader's launcher class path from a folder NeoForge's own installer ran in, and
//! `NeoBoot.start` runs FML's launcher on a thread of its own. FML builds its class loader
//! (Mixin, coremods, access transformers), the platform mod's mixin on vanilla's server main
//! runs vanilla's bootstrap and NeoForge's mod loading, then hands back. Only the dedicated
//! server's half for now: no headless client, no mirror, no content.
//!
//! NeoForge and FML are LGPL: the bridge runs their jars as they are and reads none of their
//! source (CONTRIBUTING.md). Everything here comes from the installer's own launch files
//! (`unix_args.txt`), NeoForge's documentation and what the loader does when run.

use crate::{BRIDGE, Bridge, STARTING, boot, create_vm, describe, strings, write_jar};
use jni::JNIEnv;
use jni::objects::{JObjectArray, JString, JValue};
use std::path::{Path, PathBuf};

/// What [`start`] runs.
pub struct NeoForge {
    /// The Java home to load the JVM from.
    pub home: PathBuf,
    /// The bridge's own files (its jar).
    pub work: PathBuf,
    /// A folder NeoForge's installer ran in (`--installServer`): `libraries/` holds FML,
    /// NeoForge and the game jar it patched from the player's own.
    pub install: PathBuf,
    /// The game folder FML reads `mods/` and writes `config/` in.
    pub game_dir: PathBuf,
}

/// FML's launch: the class path, its main class and arguments, as the installer wrote them
/// (`libraries/net/neoforged/neoforge/<version>/unix_args.txt`; paths relative to the folder).
#[derive(Debug, PartialEq)]
pub struct Launch {
    pub class_path: Vec<PathBuf>,
    pub main_class: String,
    pub args: Vec<String>,
}

/// Reads the installer's `unix_args.txt` in `install`.
pub fn launch(install: &Path) -> Result<Launch, String> {
    let versions = install.join("libraries/net/neoforged/neoforge");
    let mut found = Vec::new();
    for entry in std::fs::read_dir(&versions).map_err(|e| format!("{}: {e}", versions.display()))? {
        let file = entry
            .map_err(|e| e.to_string())?
            .path()
            .join("unix_args.txt");
        if file.is_file() {
            found.push(file);
        }
    }
    found.sort();
    let file = found
        .pop()
        .ok_or_else(|| format!("no NeoForge server install in {}", install.display()))?;
    let text = std::fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
    parse_launch(&text, install)
}

fn parse_launch(text: &str, install: &Path) -> Result<Launch, String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let cp = words
        .iter()
        .position(|w| *w == "-classpath" || *w == "-cp")
        .and_then(|i| words.get(i + 1))
        .ok_or("unix_args.txt has no class path")?;
    let class_path = cp
        .split(':')
        .filter(|p| !p.is_empty())
        .map(|p| install.join(p))
        .collect();
    let main = words
        .iter()
        .position(|w| w.starts_with("net.neoforged.fml.startup."))
        .ok_or("unix_args.txt names no FML launcher")?;
    Ok(Launch {
        class_path,
        main_class: words[main].to_string(),
        args: words[main + 1..].iter().map(|w| w.to_string()).collect(),
    })
}

/// Starts the JVM with FML's class path and loads the NeoForge mods in `game_dir/mods`.
/// Returns the running bridge (`mods()`: `id version` of each mod NeoForge loaded).
pub fn start(config: &NeoForge) -> Result<&'static Bridge, String> {
    let _starting = STARTING.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(bridge) = BRIDGE.get() {
        return Ok(bridge);
    }
    let launch = launch(&config.install)?;
    let bridge_jar = write_jar(&config.work, "horizon-bridge.jar", crate::BRIDGE_JAR)?;
    let mut class_path = vec![bridge_jar];
    class_path.extend(launch.class_path.iter().cloned());
    let log_config = write_jar(
        &config.work,
        "log4j2-horizon.xml",
        crate::LOG4J_CONFIG.as_bytes(),
    )?;
    let libraries = config.install.join("libraries");
    let options = [
        // What the installer's args file gives java besides the class path.
        "--add-opens=java.base/java.lang.invoke=ALL-UNNAMED".to_string(),
        "--add-exports=jdk.naming.dns/com.sun.jndi.dns=java.naming".to_string(),
        format!("-DlibraryDirectory={}", libraries.display()),
    ];
    let vm = create_vm(&config.home, &class_path, None, &log_config, &options)?;
    let mut env = vm
        .attach_current_thread_permanently()
        .map_err(|e| format!("attach: {e}"))?;
    let java_version = boot(&mut env, &[]).map_err(|e| describe(&mut env, e))?;
    let mut args = launch.args.clone();
    args.push("--gameDir".into());
    args.push(config.game_dir.to_string_lossy().into_owned());
    let mods = neo_boot(&mut env, &launch.main_class, &args).map_err(|e| describe(&mut env, e))?;
    Ok(BRIDGE.get_or_init(|| Bridge {
        vm,
        java_version,
        home: config.home.clone(),
        mods,
        fabric: false,
        class_archive: None,
    }))
}

/// Runs `NeoBoot.start`: FML's launcher until the mods are loaded.
fn neo_boot(
    env: &mut JNIEnv,
    main_class: &str,
    args: &[String],
) -> jni::errors::Result<Vec<String>> {
    let main_class = env.new_string(main_class)?;
    let args = strings(env, args)?;
    let loaded = env
        .call_static_method(
            "dev/horizonlab/bridge/neo/NeoBoot",
            "start",
            "(Ljava/lang/String;[Ljava/lang/String;)[Ljava/lang/String;",
            &[JValue::Object(&main_class), JValue::Object(&args)],
        )?
        .l()?;
    let loaded = JObjectArray::from(loaded);
    let count = env.get_array_length(&loaded)?;
    let mut out = Vec::with_capacity(count as usize);
    for i in 0..count {
        let item = JString::from(env.get_object_array_element(&loaded, i)?);
        out.push(env.get_string(&item)?.into());
        env.delete_local_ref(item)?;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_installers_args_file() {
        let text = "--add-opens java.base/java.lang.invoke=ALL-UNNAMED -DlibraryDirectory=libraries \
            -classpath libraries/a/fml.jar:libraries/b/bus.jar net.neoforged.fml.startup.Server \
            --fml.neoForgeVersion 26.3.0.101 --fml.mcVersion 26.3\n";
        let launch = parse_launch(text, Path::new("/srv/nf")).unwrap();
        assert_eq!(
            launch.class_path,
            vec![
                PathBuf::from("/srv/nf/libraries/a/fml.jar"),
                PathBuf::from("/srv/nf/libraries/b/bus.jar")
            ]
        );
        assert_eq!(launch.main_class, "net.neoforged.fml.startup.Server");
        assert_eq!(
            launch.args,
            [
                "--fml.neoForgeVersion",
                "26.3.0.101",
                "--fml.mcVersion",
                "26.3"
            ]
        );
    }
}
