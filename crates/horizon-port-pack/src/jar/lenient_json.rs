//! Fabric Loader's lenient JSON, for `fabric.mod.json` files serde_json refuses.

/// `text` with the control characters inside its strings escaped: Fabric Loader's JSON reader
/// takes raw newlines in strings (Better End's description has them), serde_json doesn't.
pub(super) fn lenient_json(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let (mut in_string, mut escaped) = (false, false);
    for c in text.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            } else if (c as u32) < 0x20 {
                out.push_str(&format!("\\u{:04x}", c as u32));
                continue;
            }
        } else if c == '"' {
            in_string = true;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn raw_newlines_in_strings_are_read() {
        let info = super::super::fabric(
            "{\"schemaVersion\": 1, \"id\": \"betterend\", \"description\": \"a\nb\"}",
        )
        .expect("lenient");
        assert_eq!(info.mod_id, "betterend");
    }
}
