//! The signed SHA-256 of a release's file: what [`super::install`] checks the game's archive
//! against, and any other file of the same release (`java_variant`'s build) too.
use super::*;

/// The signed SHA-256 of `release`'s file `asset`, or why it must not be installed.
pub(crate) fn release_digest(release: &Release, asset: &str) -> Result<String, String> {
    let tag = release.tag.as_str();
    match &release.nightly {
        // Signed for the Nightly channel, served under its own GitHub tag.
        Some(published) => crate::signed::nightly_digest(
            published,
            asset,
            &horizon_release_sign::trusted_keys(),
            fetch_small,
        ),
        None if is_game_tag(tag) => signed_digest(tag, asset),
        None => Err(format!("{tag} is not a Horizon release.")),
    }
}
