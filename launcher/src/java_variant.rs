//! Fabric 26.3 mods (beta): the game build with the Java mod bridge (docs/java-bridge.md), per
//! instance. Off by default: an instance plays the plain build, has no Java mods tab and never
//! starts Java, even with jars in its `mods-java/`. On: the launcher installs the version's
//! Java mods build (game-build.yml's `<asset>-java`) in the version's `java/` folder, beside the
//! plain one, where it plays with the version's prepared files, and starts it with
//! [`OPT_IN_ARG`]. Java 25 is the game's to fetch, the first time it starts the mods.
use crate::backend::{self, Release, CLIENT};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The folder in a version's folder holding its Java mods build.
pub const DIR: &str = "java";
/// What tells the game to run the Fabric mods (`game/java_mods.rs`, `opted_in`).
pub const OPT_IN_ARG: &str = "--java-mods";
/// Written once the build is all there.
const COMPLETE: &str = ".installed";

/// game-build.yml's Java mods builds: (`std::env::consts::OS`, `ARCH`, the asset). None for macOS yet.
pub const ASSETS: [(&str, &str, &str); 2] = [
    ("linux", "x86_64", "horizon-linux-x86_64-java.tar.gz"),
    ("windows", "x86_64", "horizon-windows-x64-java.zip"),
];

/// This system's Java mods build, if Horizon makes one.
pub fn asset() -> Option<&'static str> {
    asset_for(std::env::consts::OS, std::env::consts::ARCH)
}

fn asset_for(os: &str, arch: &str) -> Option<&'static str> {
    ASSETS
        .iter()
        .find(|(o, a, _)| *o == os && *a == arch)
        .map(|(_, _, asset)| *asset)
}

/// Whether the switch can be offered here.
pub fn available() -> bool {
    asset().is_some()
}

/// Installed version `tag`'s Java mods build folder.
pub fn dir(tag: &str) -> PathBuf {
    backend::version_dir(tag).join(DIR)
}

/// Whether installed version `tag` has its Java mods build.
pub fn is_installed(tag: &str) -> bool {
    complete(&dir(tag))
}

fn complete(dir: &Path) -> bool {
    dir.join(COMPLETE).is_file() && dir.join(CLIENT).is_file()
}

/// Installs `release`'s Java mods build as installed version `tag`'s, checked against the
/// release's signed checksums as the plain build is.
pub fn install(release: &Release, tag: &str, progress: impl Fn(f32)) -> Result<(), String> {
    let asset = asset().ok_or_else(no_build_here)?;
    let version = backend::version_dir(tag);
    if !version.join(CLIENT).is_file() {
        return Err(format!("{tag} is not installed."));
    }
    let published = release.nightly.as_deref().unwrap_or(&release.tag);
    let sha256 = backend::release_digest(release, asset).map_err(|e| missing(tag, &e))?;
    let url = crate::signed::asset_url(published, asset);
    let partial = version.join(".java.partial");
    let _ = fs::remove_dir_all(&partial);
    fs::create_dir_all(&partial).map_err(|e| e.to_string())?;
    let archive = partial.join("download");
    backend::download(&url, 0, &sha256, &archive, &progress).map_err(|e| missing(tag, &e))?;
    let unpacked = partial.join("unpacked");
    backend::unpack(&archive, &url, &unpacked)?;
    // One top folder (horizon-linux-x86_64-java/...), as the plain build's archive.
    let entries: Vec<_> = fs::read_dir(&unpacked)
        .map_err(|e| e.to_string())?
        .flatten()
        .collect();
    let top = match entries.as_slice() {
        [only] if only.path().is_dir() => only.path(),
        _ => unpacked.clone(),
    };
    place(&top, &dir(tag))?;
    let _ = fs::remove_dir_all(&partial);
    progress(1.0);
    Ok(())
}

/// A 404 or an asset the signed checksums don't list: the release has no Java mods build.
fn missing(tag: &str, why: &str) -> String {
    if why.ends_with("answered 404") || why.contains(asset().unwrap_or("-java")) {
        format!(
            "its release has none to download yet ({why}). A build of your own: \
             horizon-launcher java-mods use-build <horizon-client> --version {tag}"
        )
    } else {
        why.to_string()
    }
}

fn no_build_here() -> String {
    "Fabric mods have no build for this system yet (Linux and Windows only).".into()
}

/// Moves an unpacked build into `to`, replacing what was there.
fn place(from: &Path, to: &Path) -> Result<(), String> {
    if !from.join(CLIENT).is_file() {
        return Err(format!("The download has no {CLIENT}"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(from.join(CLIENT), fs::Permissions::from_mode(0o755));
    }
    let _ = fs::remove_dir_all(to);
    fs::rename(from, to).map_err(|e| e.to_string())?;
    fs::write(to.join(COMPLETE), "").map_err(|e| e.to_string())
}

/// A Java mods build the player has (built from a checkout with `--features java-mods`, or from
/// a fork's Actions) as installed version `tag`'s. Nothing signed it.
pub fn use_build(tag: &str, program: &Path) -> Result<(), String> {
    if !program.is_file() {
        return Err(format!("{}: no such file", program.display()));
    }
    if !backend::version_dir(tag).join(CLIENT).is_file() {
        return Err(format!("{tag} is not installed."));
    }
    let staging = backend::version_dir(tag).join(".java.partial");
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
    fs::copy(program, staging.join(CLIENT)).map_err(|e| format!("{}: {e}", program.display()))?;
    place(&staging, &dir(tag))
}

/// Installs `tag`'s Java mods build unless it has it: the release of that tag, from GitHub.
pub fn ensure(tag: &str, progress: impl Fn(f32)) -> Result<(), String> {
    if is_installed(tag) {
        return Ok(());
    }
    fetch(tag, progress).map_err(|e| {
        format!("Fabric 26.3 mods are on, but {tag} has no Fabric mods build here: {e}")
    })
}

fn fetch(tag: &str, progress: impl Fn(f32)) -> Result<(), String> {
    asset().ok_or_else(no_build_here)?;
    if crate::build_import::is_imported(tag) {
        return Err(format!(
            "{tag} is an imported build: give it its Fabric mods build with \
             horizon-launcher java-mods use-build <horizon-client> --version {tag}"
        ));
    }
    let releases = backend::releases()?;
    let release = releases
        .iter()
        .find(|r| r.tag == tag)
        .ok_or_else(|| format!("No Horizon release is tagged {tag}."))?;
    install(release, tag, progress)
}

/// After `release` installed as `tag`: its Java mods build too, when an instance with the switch
/// on plays that version (pinned to it, or following the launcher's). Best effort: Play tries
/// again.
pub fn follow(release: &Release, tag: &str) {
    let wanted = crate::instances::list()
        .iter()
        .any(|i| i.java_mods && i.version.as_deref().is_none_or(|v| v == tag));
    if !wanted || asset().is_none() || is_installed(tag) {
        return;
    }
    if let Err(e) = install(release, tag, |_| {}) {
        eprintln!("Fabric mods build of {tag} not installed: {e}");
    }
}

/// Turns the plain game's command (`backend::client_command`) into the Java mods build's: the
/// same arguments, folder and variables, with [`OPT_IN_ARG`].
pub fn switch_command(cmd: &mut Command) -> Result<(), String> {
    let plain = PathBuf::from(cmd.get_program());
    let version = plain
        .parent()
        .ok_or_else(|| format!("{}: no version folder", plain.display()))?;
    asset().ok_or_else(no_build_here)?;
    let java = version.join(DIR);
    if !complete(&java) {
        let tag = version.file_name().unwrap_or_default().to_string_lossy();
        return Err(format!(
            "{tag}'s Fabric mods build isn't installed: turn Fabric 26.3 mods off and on again, \
             or horizon-launcher java-mods on"
        ));
    }
    let mut next = Command::new(java.join(CLIENT));
    next.args(cmd.get_args());
    for (key, value) in cmd.get_envs() {
        match value {
            Some(value) => next.env(key, value),
            None => next.env_remove(key),
        };
    }
    if let Some(folder) = cmd.get_current_dir() {
        next.current_dir(folder);
    }
    next.arg(OPT_IN_ARG);
    *cmd = next;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_java_builds_are_the_plain_ones_with_java() {
        for (os, arch, asset) in ASSETS {
            let plain = backend::PLATFORMS
                .iter()
                .find(|(o, a, _, _)| *o == os && *a == arch)
                .map(|p| p.2)
                .unwrap();
            let (stem, ext) = plain.split_once('.').unwrap();
            assert_eq!(asset, format!("{stem}-java.{ext}"));
        }
        assert_eq!(asset_for("macos", "aarch64"), None);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn the_command_becomes_the_java_builds_with_its_arguments() {
        let scratch = std::env::temp_dir().join(format!("hz-java-variant-{}", std::process::id()));
        let version = scratch.join("v1.2.3");
        fs::create_dir_all(&version).unwrap();
        let mut cmd = Command::new(version.join(CLIENT));
        cmd.current_dir(&scratch)
            .arg("--resources")
            .arg("res")
            .env("HORIZON_HOME", &scratch)
            .env_remove("GONE");
        // No build yet: refused, the command left as it was.
        let err = switch_command(&mut cmd).unwrap_err();
        assert!(err.contains("v1.2.3"), "{err}");
        assert_eq!(PathBuf::from(cmd.get_program()), version.join(CLIENT));

        let staged = scratch.join("staged");
        fs::create_dir_all(&staged).unwrap();
        fs::write(staged.join(CLIENT), "").unwrap();
        place(&staged, &version.join(DIR)).unwrap();
        switch_command(&mut cmd).unwrap();
        assert_eq!(
            PathBuf::from(cmd.get_program()),
            version.join(DIR).join(CLIENT)
        );
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args, ["--resources", "res", OPT_IN_ARG]);
        assert_eq!(cmd.get_current_dir(), Some(scratch.as_path()));
        let envs: Vec<_> = cmd.get_envs().collect();
        assert!(envs.contains(&("HORIZON_HOME".as_ref(), Some(scratch.as_os_str()))));
        assert!(envs.contains(&("GONE".as_ref(), None)));
        let _ = fs::remove_dir_all(&scratch);
    }
}
