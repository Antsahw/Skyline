//! Namespaces of the mod's own client textures, which no JSON names.
use super::*;

/// A namespace holding only the mod's own client textures (no data, no models or block
/// states: GUI sheets its code draws by name, Cloth Config's `cloth-config2`) is its too:
/// nothing names those ids in JSON, but its screens need them.
pub(super) fn only_textures(
    zip: &zip::ZipArchive<fs::File>,
    candidates: BTreeSet<String>,
) -> Vec<String> {
    let mut claimed = Vec::new();
    for ns in candidates {
        let (data, assets) = (format!("data/{ns}/"), format!("assets/{ns}/"));
        let mut textures = false;
        let only_looks = zip.file_names().filter_map(Result::ok).all(|name| {
            if name.starts_with(&data) {
                return false;
            }
            match name.strip_prefix(&assets) {
                Some(rest) if rest.starts_with("textures/") => {
                    textures = true;
                    true
                }
                Some(rest) => {
                    rest.starts_with("lang/") || rest.starts_with("sounds") || rest.is_empty()
                }
                None => true,
            }
        });
        if only_looks && textures {
            claimed.push(ns);
        }
    }
    claimed
}
