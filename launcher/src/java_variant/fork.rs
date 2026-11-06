//! The java-bridge fork's own Fabric mods builds (fork-only, never upstream): when a Horizon
//! release has no Fabric mods build, the fork's release ([`java_release`]) is installed instead,
//! signed with the fork's key.
use super::{asset, download_into, no_build_here, CLIENT};
use crate::{backend, java_release};

/// Installs the java-bridge fork's own Fabric mods build ([`java_release`]) as installed
/// version `tag`'s: signed with the fork's key, and only when `tag` prepares the files it reads.
pub fn install_from_fork(tag: &str, progress: impl Fn(f32)) -> Result<(), String> {
    let asset = asset().ok_or_else(no_build_here)?;
    let version = backend::version_dir(tag);
    if !version.join(CLIENT).is_file() {
        return Err(format!("{tag} is not installed."));
    }
    let keys = java_release::keys();
    let fetch = |url: &str| backend::fetch_small(url);
    let manifest = java_release::manifest(&keys, &fetch)?;
    java_release::fits(&manifest, java_release::prepared_format(&version), tag)?;
    let sha256 = java_release::digest(asset, &keys, &fetch)?;
    let url = java_release::asset_url(asset);
    download_into(
        tag,
        &url,
        &sha256,
        &java_release::marker(&manifest),
        &progress,
    )
}

/// `own` (installing the version's own release's build), else the fork's build for `tag`: the
/// fallback when the release has none. The error names both.
pub(super) fn or_fork(
    own: Result<(), String>,
    tag: &str,
    progress: &dyn Fn(f32),
) -> Result<(), String> {
    own.or_else(|own| {
        install_from_fork(tag, progress)
            .map_err(|fork| format!("{own} From {}: {fork}", java_release::REPO))
    })
}
