//! The bridge's jars built on the player's machine. A release build (`HORIZON_BRIDGE_PLATFORM=
//! source`: Mojang's jar can't be on a build machine, and nothing of Fabric's is fetched or read
//! there) carries their sources instead of the jars, compiled once with the Java the mods run on
//! (Mojang's runtime has `javac`), then kept in the bridge's work folder until their inputs change:
//! - the bridge's own classes (`java/dev/`, [`ensure_bridge`]), against Fabric Loader and its
//!   libraries, fetched on this machine;
//! - the platform mod (`java/platform/`, Fabric mod `horizon`, [`ensure`]), against the player's
//!   own game jar, its libraries, Fabric Loader and Fabric API.

use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

/// Bumped when the way the jar is made changes, so kept jars are made again.
const FORMAT: u32 = 1;

/// The platform jar compiled from `sources` (a zip of `java/platform/`) for `fabric`'s game,
/// in `work/platform/<key>/`: made now when not there yet. `bridge` is the bridge's jar
/// (platform classes extend its classes), `fabric_api` Fabric API's jar.
#[allow(clippy::too_many_arguments)]
pub fn ensure(
    sources: &[u8],
    home: &Path,
    work: &Path,
    bridge: &Path,
    fabric_libs: &[PathBuf],
    fabric_api: &Path,
    game_jar: &Path,
    game_libraries: &[PathBuf],
    log: &dyn Fn(String),
) -> Result<PathBuf, String> {
    let dir = work.join("platform");
    let modules = |at: &Path| -> Result<Vec<PathBuf>, String> {
        let mut class_path: Vec<PathBuf> = fabric_libs.to_vec();
        class_path.push(game_jar.to_path_buf());
        class_path.extend(game_libraries.iter().cloned());
        class_path.extend(fabric_api_modules(
            fabric_api,
            &at.join("fabric-api-modules"),
        )?);
        class_path.push(bridge.to_path_buf());
        Ok(class_path)
    };
    build(
        Built {
            what: "Horizon's platform mod",
            jar: "horizon-platform.jar",
        },
        sources,
        home,
        &dir,
        &[game_jar, bridge],
        &modules,
        log,
    )
}

/// The bridge's jar compiled from `sources` (a zip of `java/dev/` and its service file) against
/// Fabric Loader and its libraries (`fabric_libs`, fetched on this machine), in
/// `work/bridge/<key>/`.
pub fn ensure_bridge(
    sources: &[u8],
    home: &Path,
    work: &Path,
    fabric_libs: &[PathBuf],
    log: &dyn Fn(String),
) -> Result<PathBuf, String> {
    let inputs: Vec<&Path> = fabric_libs.iter().map(PathBuf::as_path).collect();
    build(
        Built {
            what: "the Java mod bridge",
            jar: "horizon-bridge.jar",
        },
        sources,
        home,
        &work.join("bridge"),
        &inputs,
        &|_| Ok(fabric_libs.to_vec()),
        log,
    )
}

/// What [`build`] makes: its name in messages, its jar's file name.
struct Built {
    what: &'static str,
    jar: &'static str,
}

/// `sources` (a zip: `.java` files, other entries copied into the jar as they are) compiled
/// against `class_path(dir)` into `dir/<key>/<jar>`, the key over the sources and `inputs`
/// (path, size, time).
fn build(
    built: Built,
    sources: &[u8],
    home: &Path,
    dir: &Path,
    inputs: &[&Path],
    class_path: &dyn Fn(&Path) -> Result<Vec<PathBuf>, String>,
    log: &dyn Fn(String),
) -> Result<PathBuf, String> {
    let mut key = Sha256::new();
    key.update(format!("format {FORMAT}\n"));
    key.update(sources);
    for input in inputs {
        let meta = std::fs::metadata(input).map_err(|e| format!("{}: {e}", input.display()))?;
        let modified = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs());
        key.update(format!("{} {} {modified}\n", input.display(), meta.len()));
    }
    let key: String = key.finalize()[..8]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let dir = dir.join(&key);
    let jar = dir.join(built.jar);
    if jar.is_file() {
        return Ok(jar);
    }
    let javac = javac(home).ok_or_else(|| {
        format!(
            "the Java in {} has no compiler (javac), which builds {} once: \
             use a JDK 25 (Mojang's Java for Minecraft has it)",
            home.display(),
            built.what
        )
    })?;
    let started = std::time::Instant::now();
    log(format!(
        "Java mods: building {} on this machine (once)",
        built.what
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let src = dir.join("src");
    let classes = dir.join("classes");
    std::fs::create_dir_all(&classes).map_err(|e| format!("{}: {e}", classes.display()))?;
    let mut files = Vec::new();
    let mut resources = Vec::new();
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(sources))
        .map_err(|e| format!("{}'s sources: {e}", built.what))?;
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = entry.enclosed_name() else {
            return Err(format!(
                "unsafe name {:?} in {}'s sources",
                entry.name(),
                built.what
            ));
        };
        if entry.is_dir() {
            continue;
        }
        let path = src.join(&name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        std::fs::write(&path, &bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        if path.extension().is_some_and(|e| e == "java") {
            files.push(path);
        } else {
            resources.push((name.to_string_lossy().replace('\\', "/"), bytes));
        }
    }
    files.sort();
    let class_path = class_path(&dir)?;
    // An argument file: the class path and the sources are longer than a Windows command line.
    // Plain paths (no `\\?\`): javac's file manager takes no verbatim one.
    let spelled = |p: &Path| {
        crate::plain_path(p)
            .display()
            .to_string()
            .replace('\\', "/")
    };
    let quote = |p: &Path| format!("\"{}\"", spelled(p));
    let separator = if cfg!(windows) { ";" } else { ":" };
    let mut args = String::from("--release 21 -encoding UTF-8 -nowarn -proc:none\n");
    let joined: Vec<String> = class_path.iter().map(|p| spelled(p)).collect();
    args += &format!(
        "-cp \"{}\"\n-d {}\n",
        joined.join(separator),
        quote(&classes)
    );
    for file in &files {
        args += &quote(file);
        args.push('\n');
    }
    let argfile = dir.join("javac.args");
    // javac reads this file as UTF-8, whatever the system, but its launcher reads the command
    // line in the system's code page: only the file's own path needs care (a user name like Ivér).
    std::fs::write(&argfile, args).map_err(|e| format!("{}: {e}", argfile.display()))?;
    // javac's launcher finds its Java home from its own path, read through the code page too.
    let output = std::process::Command::new(crate::option_path(&javac)?)
        .arg(format!("@{}", crate::option_path(&argfile)?.display()))
        .output()
        .map_err(|e| format!("running {}: {e}", javac.display()))?;
    if !output.status.success() {
        let text = String::from_utf8_lossy(&output.stderr);
        let first: Vec<&str> = text.lines().take(12).collect();
        return Err(format!(
            "javac failed on {}:\n{}",
            built.what,
            first.join("\n")
        ));
    }
    let partial = dir.join(format!("{}.partial", built.jar));
    {
        let file =
            std::fs::File::create(&partial).map_err(|e| format!("{}: {e}", partial.display()))?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        let mut class_files = Vec::new();
        collect(&classes, &mut class_files);
        class_files.sort();
        for file in class_files {
            let name = file
                .strip_prefix(&classes)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/");
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            zip.write_all(&std::fs::read(&file).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        }
        for (name, bytes) in &resources {
            zip.start_file(name.as_str(), options)
                .map_err(|e| e.to_string())?;
            zip.write_all(bytes).map_err(|e| e.to_string())?;
        }
        zip.finish().map_err(|e| e.to_string())?;
    }
    std::fs::rename(&partial, &jar).map_err(|e| format!("{}: {e}", jar.display()))?;
    let _ = std::fs::remove_dir_all(&src);
    let _ = std::fs::remove_dir_all(&classes);
    log(format!(
        "Java mods: {} built in {} ms ({} sources)",
        built.what,
        started.elapsed().as_millis(),
        files.len()
    ));
    Ok(jar)
}

/// `javac` of a Java home, if it has one.
fn javac(home: &Path) -> Option<PathBuf> {
    let name = if cfg!(windows) { "javac.exe" } else { "javac" };
    let path = home.join("bin").join(name);
    path.is_file().then_some(path)
}

/// Fabric API's module jars, unpacked from its jar-in-jar (javac can't read jars in jars).
fn fabric_api_modules(jar: &Path, dir: &Path) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let file = std::fs::File::open(jar).map_err(|e| format!("{}: {e}", jar.display()))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("{}: {e}", jar.display()))?;
    let mut modules = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive.by_index(i).map_err(|e| e.to_string())?;
        let Some(file) = entry
            .name()
            .strip_prefix("META-INF/jars/")
            .map(str::to_string)
        else {
            continue;
        };
        if file.is_empty() || file.contains('/') || file.contains("..") {
            continue;
        }
        let path = dir.join(file);
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        modules.push(path);
    }
    modules.sort();
    Ok(modules)
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else {
            out.push(path);
        }
    }
}
