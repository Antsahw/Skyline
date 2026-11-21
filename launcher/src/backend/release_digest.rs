//! The signed SHA-256 of a release's file: what [`super::install`] checks the game's archive
//! against, and any other file of the same release (`java_variant`'s build) too.
use super::*;

/// The signed SHA-256 of `release`'s file `asset`, or why it must not be installed.
pub(crate) fn release_digest(release: &Release, asset: &str) -> Result<String, String> {
    let tag = release.tag.as_str();
    match &release.nightly {
        // Signed for its own version (SHA256SUMS.v2.sig), which its title names and it installs as;
        // served under its own GitHub tag. A title that doesn't parse strictly is refused: nothing
        // signs GitHub's publication time.
        Some(github_tag) => {
            let version = horizon_release_sign::nightly_title_version(&release.name)
                .filter(|v| *v == release.tag)
                .ok_or_else(|| {
                    crate::i18n::trf(
                        "This Nightly's title ({title}) doesn't name its date and commit as Horizon \
                         signs them, so it can't be checked: it was not installed.",
                        &[("title", &format!("{:?}", release.name))],
                    )
                })?;
            crate::signed::nightly_digest(
                github_tag,
                &version,
                asset,
                &horizon_release_sign::trusted_keys(),
                fetch_small,
            )
        }
        None if is_game_tag(tag) => signed_digest(tag, asset),
        None => Err(format!("{tag} is not a Horizon release.")),
    }
}
