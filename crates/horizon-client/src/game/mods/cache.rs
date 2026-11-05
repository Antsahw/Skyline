//! The game's cache folder.

/// The game's cache folder (`.horizon-cache` in the working directory), by its real path: what
/// is built there (the mods' content, the content root) names files by it and is stamped with
/// it, so a cache reached through a link (a launcher's or a test's per-run folder linking a
/// kept cache) is found again under the same name the next time.
pub(in crate::game) fn horizon_cache() -> std::path::PathBuf {
    let here = std::env::current_dir()
        .unwrap_or_default()
        .join(".horizon-cache");
    let _ = std::fs::create_dir_all(&here);
    std::fs::canonicalize(&here).unwrap_or(here)
}
