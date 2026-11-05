//! Mods' items drawn with the quads their Java models make (the Java bridge).
use super::*;

impl ItemRenderer {
    /// Forgets the GUI items already drawn (their models changed: [`set_java_item_models`]).
    pub fn forget_drawn(&self) {
        if let Ok(mut drawn) = self.gui_drawn.lock() {
            drawn.clear();
        }
    }

    /// `layers` of `item`, or, for an item Horizon's definition draws nothing for, its
    /// Java model's.
    pub(super) fn java_layers(
        &self,
        item: &str,
        display: DisplayContext,
        mut layers: Vec<LayerRender>,
    ) -> Vec<LayerRender> {
        // An item Horizon's definition draws nothing for (a mod's own model type, a model
        // whose quads all failed) whose Java model made quads: those (`set_java_item_models`).
        let java = java_item_model(item).filter(|_| {
            layers.iter().all(|l| match &l.geometry {
                LayerGeometry::Quads(baked) => {
                    baked.quads.is_empty() || Arc::ptr_eq(baked, &self.bake(MISSING_MODEL))
                }
                LayerGeometry::Special { .. } => false,
            })
        });
        if let Some(baked) = java {
            layers.clear();
            layers.push(LayerRender {
                geometry: LayerGeometry::Quads(baked.clone()),
                tints: Vec::new(),
                foil: FoilType::None,
                matrix: glam_matrix(
                    &baked.layer_matrix(display, &horizon_assets::math::Mat4::IDENTITY),
                ),
                uses_block_light: baked.uses_block_light,
                particle: sprite_name(&baked.particle.sprite).to_owned(),
                extents: Arc::new(corners(&baked)),
            });
        }
        layers
    }
}

/// Mods' items whose definitions Horizon can't read (their mods' own item model types),
/// drawn with the quads their Java models make (the Java bridge, once its models load).
static JAVA_ITEM_MODELS: std::sync::RwLock<Option<HashMap<String, Arc<BakedItemModel>>>> =
    std::sync::RwLock::new(None);

/// Replaces the Java-made item models ([`java_item_models_from_json`]).
pub fn set_java_item_models(models: HashMap<String, BakedItemModel>) {
    if let Ok(mut slot) = JAVA_ITEM_MODELS.write() {
        *slot = Some(models.into_iter().map(|(k, v)| (k, Arc::new(v))).collect());
    }
}

fn java_item_model(item: &str) -> Option<Arc<BakedItemModel>> {
    JAVA_ITEM_MODELS.read().ok()?.as_ref()?.get(item).cloned()
}

/// The Java bridge's item models: `{"<item>": {"quads": [{"p": [x,y,z ×4], "uv": [u,v ×4]
/// (the sprite's own, 0..1), "s": sprite, "d": direction, "c": argb, "t": translucent, "e":
/// emission, "sd": shade direction}], "block_light": bool, "particle": sprite, "display":
/// {"<context>": [rotation ×3, translation ×3 (blocks), scale ×3]}}}`.
pub fn java_item_models_from_json(json: &str) -> HashMap<String, BakedItemModel> {
    use horizon_assets::{
        bake::ModelQuad,
        model::{DISPLAY_CONTEXTS, ItemTransform, Material},
    };
    let Ok(Value::Object(items)) = serde_json::from_str::<Value>(json) else {
        return HashMap::new();
    };
    let floats = |v: Option<&Value>| -> Vec<f32> {
        v.and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_f64)
                    .map(|f| f as f32)
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut out = HashMap::new();
    for (item, o) in items {
        let mut quads = Vec::new();
        for q in o
            .get("quads")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let (p, uv) = (floats(q.get("p")), floats(q.get("uv")));
            let Some(sprite) = q.get("s").and_then(Value::as_str) else {
                continue;
            };
            if p.len() != 12 || uv.len() != 8 {
                continue;
            }
            let dir = |k: &str| {
                q.get(k)
                    .and_then(Value::as_str)
                    .and_then(Direction::by_name)
            };
            let emission = q.get("e").and_then(Value::as_u64).unwrap_or(0).min(15) as u8;
            quads.push(ModelQuad {
                positions: std::array::from_fn(|i| [p[i * 3], p[i * 3 + 1], p[i * 3 + 2]]),
                uv: std::array::from_fn(|i| [uv[i * 2], uv[i * 2 + 1]]),
                facing: dir("d").unwrap_or(Direction::Up),
                cull: None,
                material: Material {
                    sprite: sprite.to_owned(),
                    force_translucent: q.get("t").and_then(Value::as_bool).unwrap_or(false),
                },
                tint_index: -1,
                shade_direction_override: dir("sd"),
                light_emission: emission,
                light: [emission, 0],
                color: q.get("c").and_then(Value::as_i64).map(|c| c as i32),
                ambient_occlusion: None,
                render_type: None,
                shade: true,
            });
        }
        if quads.is_empty() {
            continue;
        }
        let mut transforms: [Option<ItemTransform>; 9] = Default::default();
        if let Some(display) = o.get("display").and_then(Value::as_object) {
            for (i, name) in DISPLAY_CONTEXTS.iter().enumerate() {
                let t = floats(display.get(*name));
                if t.len() == 9 {
                    transforms[i] = Some(ItemTransform {
                        rotation: [t[0], t[1], t[2]],
                        translation: [t[3], t[4], t[5]],
                        scale: [t[6], t[7], t[8]],
                    });
                }
            }
        }
        let particle = o
            .get("particle")
            .and_then(Value::as_str)
            .map_or_else(|| quads[0].material.sprite.clone(), str::to_owned);
        out.insert(
            item,
            BakedItemModel {
                quads,
                uses_block_light: o
                    .get("block_light")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
                particle: Material {
                    sprite: particle,
                    force_translucent: false,
                },
                transforms,
            },
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bridge's item models: a quad's corners, its sprite's own UVs, colour, translucency
    /// and emission as Java sent them; each context's display transform by its name; a quad
    /// that isn't four corners is left out, an item with none is left out.
    #[test]
    fn java_item_models_keep_their_quads_and_transforms() {
        let json = r#"{"mod:tool": {"block_light": true, "particle": "mod:item/tool",
            "quads": [
              {"p": [0,0,0, 1,0,0, 1,1,0, 0,1,0], "uv": [0,1, 1,1, 1,0, 0,0], "s": "mod:item/tool",
               "d": "north", "c": -16711936, "t": true, "e": 7},
              {"p": [0,0,0], "uv": [0,0], "s": "mod:item/tool", "d": "up"}],
            "display": {"gui": [30,225,0, 0,0.0625,0, 0.625,0.625,0.625]}},
            "mod:empty": {"quads": []}}"#;
        let models = java_item_models_from_json(json);
        assert_eq!(models.len(), 1);
        let tool = &models["mod:tool"];
        assert_eq!(tool.quads.len(), 1);
        let q = &tool.quads[0];
        assert_eq!(q.positions[2], [1.0, 1.0, 0.0]);
        assert_eq!(q.uv[3], [0.0, 0.0]);
        assert_eq!(q.facing, Direction::North);
        assert_eq!(q.material.sprite, "mod:item/tool");
        assert!(q.material.force_translucent);
        assert_eq!(
            (q.color, q.tint_index, q.light_emission),
            (Some(-16711936), -1, 7)
        );
        assert!(tool.uses_block_light);
        let gui = tool
            .transform(DisplayContext::Gui)
            .expect("its GUI transform");
        assert_eq!(
            (gui.rotation, gui.translation[1], gui.scale[0]),
            ([30.0, 225.0, 0.0], 0.0625, 0.625)
        );
        assert!(tool.transform(DisplayContext::Ground).is_none());
    }
}
