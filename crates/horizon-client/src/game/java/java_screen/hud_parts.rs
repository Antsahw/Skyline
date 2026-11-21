//! The mods' HUD as one frame per player: the 26.3 bridge's part and the 1.20.1 bridge's, each
//! as it last drew it.
use super::{CustomPayload, HUD_CHANNEL};

/// Which bridge drew a part of a player's HUD frame ([`send_hud_part`]).
#[derive(Clone, Copy)]
pub(in crate::game) enum HudPart {
    /// The 26.3 mods' (`java_server`).
    #[cfg_attr(not(feature = "java-mods"), allow(dead_code))]
    Mods263 = 0,
    /// The 1.20.1 mods' (`java_mods_120/hud.rs`).
    #[cfg_attr(not(feature = "java-mods-120"), allow(dead_code))]
    Mods120 = 1,
}

/// Each player's HUD as each bridge last drew it, by connection: a world with both kinds of
/// mods sends one frame holding both, as each changes.
static HUD_PARTS: std::sync::Mutex<Option<std::collections::HashMap<u32, [String; 2]>>> =
    std::sync::Mutex::new(None);

/// The two bridges' parts of a HUD as one frame: their operations one after the other.
fn merge_hud(parts: &[String; 2]) -> String {
    let inner: Vec<&str> = parts
        .iter()
        .map(|p| {
            p.trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .trim()
        })
        .filter(|p| !p.is_empty())
        .collect();
    format!("[{}]", inner.join(","))
}

/// `part` of `conn`'s HUD drew `frame` (`[...]`): the whole HUD, both bridges' parts, to them.
pub(in crate::game) fn send_hud_part(
    host: &mut crate::game::lan::LanHost,
    conn: u32,
    part: HudPart,
    frame: &str,
) {
    let merged = {
        let mut guard = HUD_PARTS.lock().unwrap_or_else(|e| e.into_inner());
        let parts = guard
            .get_or_insert_with(Default::default)
            .entry(conn)
            .or_default();
        parts[part as usize] = frame.to_owned();
        merge_hud(parts)
    };
    host.hub.send_to(
        conn,
        &horizon_protocol::packets::play::Clientbound::CustomPayload(CustomPayload {
            channel: HUD_CHANNEL.into(),
            data: merged.into_bytes(),
        }),
    );
}

/// A player left: their HUD's parts with them.
#[cfg(feature = "java-mods-120")]
pub(in crate::game) fn forget_hud(conn: u32) {
    if let Some(parts) = HUD_PARTS.lock().unwrap_or_else(|e| e.into_inner()).as_mut() {
        parts.remove(&conn);
    }
}

#[cfg(test)]
mod tests {
    use super::merge_hud;

    #[test]
    fn both_bridges_parts_are_one_frame() {
        let parts = [
            r#"[{"op":"fill"}]"#.to_owned(),
            r#"[{"op":"blit"},{"op":"text"}]"#.to_owned(),
        ];
        let merged: serde_json::Value = serde_json::from_str(&merge_hud(&parts)).unwrap();
        let ops: Vec<&str> = merged
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["op"].as_str().unwrap())
            .collect();
        assert_eq!(ops, ["fill", "blit", "text"]);
        // A bridge that drew nothing, or hasn't drawn yet, adds nothing.
        let parts = [String::new(), "[]".to_owned()];
        assert_eq!(merge_hud(&parts), "[]");
    }
}
