//! Blocks only their block entity's renderer draws (`RenderShape.INVISIBLE`).
use super::*;

/// A block only its block entity's renderer draws (RenderShape.INVISIBLE: Chunk Loaders'
/// floating core): its models carried empty (their particle kept), or Horizon drew them still
/// under the moving one. The blockstate `v` of `ns:b` as it is carried then (its empty models
/// written); `None` for another block, the file kept as it is.
pub(super) fn blockstate(
    facts: Option<&Facts>,
    ns: &str,
    b: &str,
    v: &Value,
    inside: &Path,
) -> Result<Option<Vec<u8>>, String> {
    let invisible = facts
        .and_then(|f| f.blocks.get(&format!("{ns}:{b}")))
        .is_some_and(|block| {
            !block.states.is_empty() && block.states.values().all(|s| s.invisible)
        });
    if invisible {
        let mut hidden = v.clone();
        let mut made = BTreeMap::new();
        empty_models(&mut hidden, ns, &mut made);
        for (model, parent) in &made {
            let path = model.split_once(':').map_or(model.as_str(), |(_, p)| p);
            let out = inside.join(format!("assets/{ns}/models/{path}.json"));
            let json = json!({"parent": parent, "elements": []});
            write(&out, &serde_json::to_vec_pretty(&json).unwrap_or_default())?;
        }
        return Ok(serde_json::to_vec_pretty(&hidden).ok());
    }
    Ok(None)
}

/// A blockstate's models replaced with empty children of them (`<ns>:block/horizon_invisible/...`,
/// recorded in `made` with their parent): the block's model draws nothing.
fn empty_models(v: &mut Value, ns: &str, made: &mut BTreeMap<String, String>) {
    match v {
        Value::Object(o) => {
            if let Some(Value::String(model)) = o.get("model").cloned() {
                let name = format!(
                    "{ns}:block/horizon_invisible/{}",
                    model.replace([':', '/'], "_")
                );
                made.insert(name.clone(), model);
                o.insert("model".into(), Value::String(name));
            }
            for (_, child) in o.iter_mut() {
                empty_models(child, ns, made);
            }
        }
        Value::Array(a) => a.iter_mut().for_each(|c| empty_models(c, ns, made)),
        _ => {}
    }
}
