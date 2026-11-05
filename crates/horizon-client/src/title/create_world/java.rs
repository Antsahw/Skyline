//! A mod's world preset (`oneblock:oneblock`) on the World Type button, after vanilla's.
use super::*;

impl WorldOptions {
    /// The World Type button (`id` 25) cycles through the mods' presets too: read them
    /// the first time it's pressed.
    pub(in crate::title) fn load_mod_presets(&mut self, id: u8, root: &std::path::Path) {
        if id == 25 && self.presets.is_empty() {
            self.presets = mod_presets(root);
        }
    }

    /// The World Type button's step through the mods' presets: true if it moved to, along or
    /// off them (else vanilla's next or previous preset follows).
    pub(super) fn cycle_mod_preset(&mut self, step: i32) -> bool {
        // WorldCreationUiState's preset list: vanilla's, then the mods' presets in
        // the `#minecraft:normal` tag.
        use crate::save::WorldPreset;
        let (first, last) = (
            WorldPreset::ALL[0],
            WorldPreset::ALL[WorldPreset::ALL.len() - 1],
        );
        let at = self
            .mod_preset
            .as_ref()
            .and_then(|p| self.presets.iter().position(|q| q == p));
        if step >= 0 {
            match at {
                Some(i) => {
                    self.mod_preset = self.presets.get(i + 1).cloned();
                    if self.mod_preset.is_none() {
                        self.world_type = first;
                    }
                }
                None if self.world_type == last && !self.presets.is_empty() => {
                    self.mod_preset = self.presets.first().cloned();
                }
                None => return false,
            }
        } else {
            match at {
                Some(0) => {
                    self.mod_preset = None;
                    self.world_type = last;
                }
                Some(i) => self.mod_preset = self.presets.get(i - 1).cloned(),
                None if self.world_type == first && !self.presets.is_empty() => {
                    self.mod_preset = self.presets.last().cloned();
                }
                None => return false,
            }
        }
        true
    }

    /// The World Type button's value: the mod's preset, else vanilla's.
    pub(super) fn world_type_label(&self) -> String {
        match &self.mod_preset {
            // WorldPreset's description: generator.<namespace>.<path>.
            Some(p) => {
                let key = format!("generator.{}", p.replacen(':', ".", 1));
                let name = tr(&key);
                if name == key { p.clone() } else { name }
            }
            None => tr(self.world_type.label()),
        }
    }
}

/// The mods' world presets a new world can take: those of `<root>/data/*/worldgen/world_preset/`
/// in the `#minecraft:normal` tag (what CreateWorldScreen's World Type cycles through), mods'
/// only (vanilla's are `WorldPreset`), sorted.
fn mod_presets(root: &std::path::Path) -> Vec<String> {
    let Ok(text) =
        std::fs::read_to_string(root.join("data/minecraft/tags/worldgen/world_preset/normal.json"))
    else {
        return Vec::new();
    };
    let Ok(tag) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Vec::new();
    };
    let mut out: Vec<String> = tag
        .get("values")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().or_else(|| v.get("id").and_then(|i| i.as_str())))
        .filter(|id| !id.starts_with('#'))
        .filter_map(|id| {
            let (ns, path) = id.split_once(':').unwrap_or(("minecraft", id));
            (ns != "minecraft"
                && root
                    .join(format!("data/{ns}/worldgen/world_preset/{path}.json"))
                    .is_file())
            .then(|| format!("{ns}:{path}"))
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mod_world_presets_cycle_and_keep_their_dimensions() {
        let base = std::env::temp_dir().join(format!("horizon-mod-presets-{}", std::process::id()));
        let root = base.join("content");
        let write = |path: &str, text: &str| {
            let p = root.join(path);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, text).unwrap();
        };
        write(
            "data/minecraft/tags/worldgen/world_preset/normal.json",
            r#"{"values":["minecraft:normal","minecraft:flat","oneblock:oneblock","gone:missing"]}"#,
        );
        write(
            "data/oneblock/worldgen/world_preset/oneblock.json",
            r#"{"dimensions":{"minecraft:overworld":{"type":"minecraft:overworld","generator":{"type":"oneblock:oneblock"}}}}"#,
        );
        // Mods' presets only, and only those whose file is there.
        let presets = mod_presets(&root);
        assert_eq!(presets, ["oneblock:oneblock"]);
        // Vanilla's presets, the mod's, Default again.
        let mut o = WorldOptions {
            presets,
            ..Default::default()
        };
        let mut creative = false;
        let mut seen = Vec::new();
        for _ in 0..6 {
            o.press(25, &mut creative);
            seen.push((o.world_type, o.mod_preset.clone()));
        }
        use crate::save::WorldPreset as P;
        assert_eq!(
            seen,
            [
                (P::Flat, None),
                (P::LargeBiomes, None),
                (P::Amplified, None),
                (P::SingleBiomeSurface, None),
                (P::SingleBiomeSurface, Some("oneblock:oneblock".to_owned())),
                (P::Normal, None),
            ]
        );
        // The world keeps the preset's id and its dimensions.
        let dir = base.join("oneblock");
        let options = crate::save::WorldGen {
            mod_preset: Some("oneblock:oneblock".into()),
            ..Default::default()
        };
        let dimensions = crate::save::preset_dimensions(&root, "oneblock:oneblock");
        assert!(dimensions.is_some());
        crate::save::write_world_gen_settings_preset(
            &dir,
            7,
            options.clone(),
            None,
            None,
            dimensions,
        )
        .unwrap();
        assert_eq!(crate::save::WorldGen::read(&dir), Some(options));
        let settings = crate::save::world_gen_settings(&dir).unwrap();
        assert_eq!(
            crate::save::text(
                settings
                    .get("dimensions")
                    .and_then(|d| d.get("minecraft:overworld"))
                    .and_then(|o| o.get("generator"))
                    .and_then(|g| g.get("type"))
            )
            .as_deref(),
            Some("oneblock:oneblock")
        );
    }
}
