//! Legacy `.lang` translations (pre-1.13 `key=value` lines), carried as JSON.
use super::*;

/// `assets/<ns>/lang/<code>.json` for a legacy `assets/<ns>/lang/<code>.lang` (its code
/// lowercased: `en_US.lang`).
pub(super) fn legacy_lang(name: &str) -> Option<String> {
    let stem = name.strip_suffix(".lang")?;
    let (dir, code) = stem.rsplit_once('/')?;
    let parts: Vec<&str> = dir.split('/').collect();
    (parts.len() == 3 && parts[0] == "assets" && parts[2] == "lang" && !code.is_empty())
        .then(|| format!("{dir}/{}.json", code.to_ascii_lowercase()))
}

/// A `.lang` file's `key=value` lines (`#` comments, blank lines skipped) as a JSON object.
pub(super) fn lang_json(bytes: &[u8]) -> Vec<u8> {
    let text = String::from_utf8_lossy(bytes);
    let map: serde_json::Map<String, Value> = text
        .lines()
        .map(|l| l.trim_start_matches('\u{feff}'))
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_owned(), Value::String(v.to_owned())))
        .collect();
    serde_json::to_vec_pretty(&Value::Object(map)).expect("strings serialize")
}
