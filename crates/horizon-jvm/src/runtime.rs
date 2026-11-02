//! Java for the mods when the player has none: Mojang's own runtime for the game version (the
//! one Minecraft itself runs on, `javaVersion.component` of the version JSON), downloaded once
//! into the Horizon launcher's runtime folder, where the launcher keeps the ones it downloads
//! too (`<launcher data>/minecraft/runtime/<component>/<platform>/<component>/`), so each
//! finds the other's. Every file is checked by the SHA-1 Mojang's manifest gives.

use sha1::{Digest, Sha1};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

/// Mojang's index of its Java runtimes (the official launcher's, and Horizon's launcher's).
const RUNTIMES: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";

/// The Horizon launcher's data folder, as the launcher finds it: `HORIZON_LAUNCHER_HOME`, else
/// the system's data folder + `horizon-launcher`.
pub fn launcher_dir() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os("HORIZON_LAUNCHER_HOME").filter(|h| !h.is_empty()) {
        return Some(PathBuf::from(home));
    }
    let home = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
        .filter(|h| !h.is_empty())
        .map(PathBuf::from);
    let data = if cfg!(windows) {
        std::env::var_os("APPDATA").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        home.map(|h| h.join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .filter(|d| Path::new(d).is_absolute())
            .map(PathBuf::from)
            .or_else(|| home.map(|h| h.join(".local/share")))
    }?;
    Some(data.join("horizon-launcher"))
}

/// The launcher's runtime folder (`minecraft/runtime`): Mojang's runtimes it downloaded.
pub fn runtime_dir() -> Option<PathBuf> {
    launcher_dir().map(|d| d.join("minecraft").join("runtime"))
}

/// The major version a Java home's `release` file names (`JAVA_VERSION="25.0.1"`).
pub fn release_major(home: &Path) -> Option<i64> {
    let text = std::fs::read_to_string(home.join("release")).ok()?;
    let line = text.lines().find_map(|l| l.strip_prefix("JAVA_VERSION="))?;
    let version = line.trim().trim_matches('"');
    let major = version.split(['.', '+', '-']).next()?;
    // Java 8 and older: "1.8.0_402".
    match major.parse().ok()? {
        1 => version.split('.').nth(1)?.parse().ok(),
        major => Some(major),
    }
}

/// Mojang's platform name for this system.
fn platform() -> Option<&'static str> {
    Some(match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => "mac-os-arm64",
        ("macos", "x86_64") => "mac-os",
        ("linux", "x86_64") => "linux",
        ("linux", "x86") => "linux-i386",
        ("windows", "x86_64") => "windows-x64",
        ("windows", "aarch64") => "windows-arm64",
        ("windows", "x86") => "windows-x86",
        _ => return None,
    })
}

/// Mojang's `component` runtime in `runtime`, downloaded (or completed) now: its Java home.
/// Files already there with the right SHA-1 are kept.
pub fn ensure(component: &str, runtime: &Path, log: &dyn Fn(String)) -> Result<PathBuf, String> {
    if component.is_empty()
        || !component
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(format!("unsafe Java runtime name {component:?}"));
    }
    let platform = platform().ok_or("Mojang has no Java for this system")?;
    let index: serde_json::Value = serde_json::from_slice(&download(RUNTIMES)?)
        .map_err(|e| format!("Mojang's Java index: {e}"))?;
    let entry = &index[platform][component][0];
    let manifest = &entry["manifest"];
    let (Some(url), Some(sha1)) = (manifest["url"].as_str(), manifest["sha1"].as_str()) else {
        return Err(format!("Mojang has no {component} for {platform}"));
    };
    let manifest: serde_json::Value = serde_json::from_slice(&checked(url, sha1)?)
        .map_err(|e| format!("Mojang's {component} manifest: {e}"))?;
    let files = manifest["files"]
        .as_object()
        .ok_or_else(|| format!("Mojang's {component} manifest lists no files"))?;
    let home = runtime.join(component).join(platform).join(component);
    let mut wanted = Vec::new();
    let mut links = Vec::new();
    for (name, file) in files {
        let path = home.join(inside(name)?);
        match file["type"].as_str() {
            Some("file") => {
                let raw = &file["downloads"]["raw"];
                let (Some(url), Some(sha1)) = (raw["url"].as_str(), raw["sha1"].as_str()) else {
                    return Err(format!("{name} has no download in Mojang's manifest"));
                };
                wanted.push((
                    path,
                    url.to_string(),
                    sha1.to_string(),
                    file["executable"] == true,
                ));
            }
            Some("link") => {
                let target = file["target"].as_str().unwrap_or_default().to_string();
                links.push((path, target));
            }
            _ => {}
        }
    }
    let missing: Vec<_> = wanted
        .into_iter()
        .filter(|(path, _, sha1, _)| {
            std::fs::read(path).map_or(true, |bytes| hex(&Sha1::digest(&bytes)) != *sha1)
        })
        .collect();
    if !missing.is_empty() {
        let name = entry["version"]["name"].as_str().unwrap_or(component);
        log(format!(
            "Java mods: downloading Minecraft's Java {name} from Mojang ({} files) into {}",
            missing.len(),
            home.display()
        ));
        // A few at a time: hundreds of small files.
        let next = std::sync::atomic::AtomicUsize::new(0);
        let failed = std::sync::Mutex::new(None::<String>);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    loop {
                        let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        let Some((path, url, sha1, executable)) = missing.get(i) else {
                            break;
                        };
                        if failed.lock().is_ok_and(|f| f.is_some()) {
                            break;
                        }
                        if let Err(e) = fetch_to(path, url, sha1, *executable)
                            && let Ok(mut f) = failed.lock()
                        {
                            f.get_or_insert(e);
                        }
                    }
                });
            }
        });
        if let Some(e) = failed.into_inner().ok().flatten() {
            return Err(e);
        }
    }
    #[cfg(unix)]
    for (path, target) in &links {
        // A link stays inside the runtime (Mojang's macOS ones point into the bundle).
        let from = path.strip_prefix(&home).ok().and_then(Path::parent);
        let depth = link_depth(from.unwrap_or(Path::new("")), target);
        if depth < 0 {
            return Err(format!(
                "unsafe link {target:?} in Mojang's {component} manifest"
            ));
        }
        let _ = std::fs::remove_file(path);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::os::unix::fs::symlink(target, path).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    #[cfg(not(unix))]
    let _ = links;
    Ok(home)
}

/// How deep inside the runtime a link in `from` (relative to its home) pointing at `target`
/// ends: negative when it leaves it (or is absolute).
fn link_depth(from: &Path, target: &str) -> i64 {
    let mut depth = 0i64;
    for c in from.components().chain(Path::new(target).components()) {
        match c {
            Component::ParentDir => depth -= 1,
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            _ => return -1,
        }
        if depth < 0 {
            return -1;
        }
    }
    depth
}

/// A manifest's relative name as a path inside the runtime (no `..`, no root).
fn inside(name: &str) -> Result<PathBuf, String> {
    let path = Path::new(name);
    if path
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
    {
        Ok(path.to_path_buf())
    } else {
        Err(format!(
            "unsafe file name {name:?} in Mojang's Java manifest"
        ))
    }
}

fn fetch_to(path: &Path, url: &str, sha1: &str, executable: bool) -> Result<(), String> {
    let bytes = checked(url, sha1)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let partial = path.with_extension("horizon-partial");
    std::fs::write(&partial, &bytes).map_err(|e| format!("{}: {e}", partial.display()))?;
    #[cfg(unix)]
    if executable {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&partial, std::fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("{}: {e}", partial.display()))?;
    }
    #[cfg(not(unix))]
    let _ = executable;
    std::fs::rename(&partial, path).map_err(|e| format!("{}: {e}", path.display()))
}

fn checked(url: &str, sha1: &str) -> Result<Vec<u8>, String> {
    let bytes = download(url)?;
    let got = hex(&Sha1::digest(&bytes));
    if got != sha1 {
        return Err(format!(
            "{url} has SHA-1 {got}, expected {sha1}: refusing it"
        ));
    }
    Ok(bytes)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn download(url: &str) -> Result<Vec<u8>, String> {
    let response = crate::libs::agent()
        .get(url)
        .call()
        .map_err(|e| format!("downloading {url}: {e}"))?;
    let mut bytes = Vec::new();
    response
        .into_reader()
        .take(256 << 20)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("downloading {url}: {e}"))?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_stay_inside() {
        assert!(inside("bin/java").is_ok());
        assert!(inside("../evil").is_err());
        assert!(inside("/etc/passwd").is_err());
        assert!(inside("lib/../../x").is_err());
    }

    #[test]
    fn release_versions() {
        let dir = std::env::temp_dir().join(format!("hz-release-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (text, major) in [
            ("JAVA_VERSION=\"25.0.1\"\n", Some(25)),
            ("JAVA_VERSION=\"1.8.0_402\"", Some(8)),
            ("X=1", None),
        ] {
            std::fs::write(dir.join("release"), text).unwrap();
            assert_eq!(release_major(&dir), major, "{text}");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn links_stay_inside() {
        assert!(link_depth(Path::new("jre.bundle/Contents/Home/bin"), "../lib/x") >= 0);
        assert!(link_depth(Path::new("bin"), "../../x") < 0);
        assert!(link_depth(Path::new("bin"), "/usr/bin/java") < 0);
    }

    #[test]
    fn launcher_home_wins() {
        // Read only: the variable may be set by the test run already.
        if let Some(home) = std::env::var_os("HORIZON_LAUNCHER_HOME").filter(|h| !h.is_empty()) {
            assert_eq!(launcher_dir(), Some(PathBuf::from(home)));
        }
    }
}
