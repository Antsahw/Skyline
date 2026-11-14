//! The Mods page's rows for the Java mods: a section per Java runtime (Fabric 26.3, Fabric
//! 1.20.1) listing what is installed for it, each with its settings when the world's server
//! can open them (game/java_screen.rs), then the server's other mods with a settings screen.
use super::{Act, Row, button, inactive};
use crate::{lang, settings::Settings, text::Text};

/// The Java mods: each runtime's section, then the settings of mods in none of them.
pub(super) fn mod_rows(s: &Settings) -> Vec<Row> {
    let settings_of = |id: &str| s.mods.java.iter().position(|(m, _)| m == id);
    let mut rows = Vec::new();
    let mut listed = vec![false; s.mods.java.len()];
    for runtime in s.mods.runtimes.iter().filter(|r| !r.mods.is_empty()) {
        rows.push(Row::Heading(runtime.title.clone()));
        for m in &runtime.mods {
            rows.push(Row::Buttons(
                match settings_of(&m.id) {
                    Some(i) => {
                        listed[i] = true;
                        let label = format!("{}: {}", m.title, lang::get("horizon.mods.settings"));
                        button(
                            0.0,
                            0.0,
                            310.0,
                            Text::literal(label),
                            Act::JavaModSettings(i),
                        )
                    }
                    None => inactive(button(
                        0.0,
                        0.0,
                        310.0,
                        Text::literal(m.title.clone()),
                        Act::JavaModSettings(usize::MAX),
                    )),
                },
                None,
            ));
        }
    }
    let rest: Vec<Row> = s
        .mods
        .java
        .iter()
        .enumerate()
        .filter(|(i, _)| !listed[*i])
        .map(|(i, (_, name))| {
            let label = format!("{name}: {}", lang::get("horizon.mods.settings"));
            Row::Buttons(
                button(
                    0.0,
                    0.0,
                    310.0,
                    Text::literal(label),
                    Act::JavaModSettings(i),
                ),
                None,
            )
        })
        .collect();
    if !rest.is_empty() {
        rows.push(Row::Heading("horizon.mods.runtime.server".into()));
        rows.extend(rest);
    }
    rows
}
