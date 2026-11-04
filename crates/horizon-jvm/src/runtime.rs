//! Where Mojang's Java runtimes are on this computer: the Horizon launcher's runtime folder
//! (`<launcher data>/minecraft/runtime/<component>/<platform>/<component>/`), where the launcher
//! downloads the game version's Java (`javaVersion.component`) for an instance with Fabric mods
//! on, after its Microsoft sign-in. The game itself downloads no Java: started outside the
//! launcher, it runs the mods only on a Java 25 already installed.

use std::path::{Path, PathBuf};

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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn launcher_home_wins() {
        // Read only: the variable may be set by the test run already.
        if let Some(home) = std::env::var_os("HORIZON_LAUNCHER_HOME").filter(|h| !h.is_empty()) {
            assert_eq!(launcher_dir(), Some(PathBuf::from(home)));
        }
    }
}
