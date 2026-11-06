//! The Mods page's rows for the world's Java mods with a settings screen (the server opens
//! them: game/java_screen.rs).
use super::{Act, Row, button};
use crate::{settings::Settings, text::Text};

/// The world's Java mods with a settings screen (the server opens them), under a header.
pub(super) fn mod_rows(s: &Settings) -> Vec<Row> {
    let java = s.mods.java.iter().enumerate().map(|(i, (_, name))| {
        Row::Buttons(
            button(
                0.0,
                0.0,
                310.0,
                Text::literal(format!("{name}: Settings")),
                Act::JavaModSettings(i),
            ),
            None,
        )
    });
    if s.mods.java.is_empty() {
        Vec::new()
    } else {
        std::iter::once(Row::Header("Java mods"))
            .chain(java)
            .collect()
    }
}
