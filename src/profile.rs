use serde_json::Value;

/// Turn the creator's profile.json into plain "key: value" lines. Only facts that
/// are actually filled in appear, so an empty field can never be quoted back.
pub fn flatten(v: &Value) -> String {
    let mut lines = Vec::new();
    walk("", v, &mut lines);
    lines.join("\n")
}

fn walk(path: &str, v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => {
            if !s.trim().is_empty() {
                out.push(format!("{}: {}", path, s.trim()));
            }
        }
        Value::Number(n) => out.push(format!("{}: {}", path, n)),
        Value::Bool(b) => out.push(format!("{}: {}", path, b)),
        Value::Array(a) => {
            for item in a {
                walk(path, item, out);
            }
        }
        Value::Object(m) => {
            for (k, val) in m {
                let p = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{} {}", path, k)
                };
                walk(&p, val, out);
            }
        }
        Value::Null => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_and_skips_empty_fields() {
        let v: Value = serde_json::from_str(
            r#"{"pet": {"name": "Biscuit", "traits": ["curious", "dramatic"]}, "rates": "", "past_work": []}"#,
        )
        .unwrap();
        let text = flatten(&v);
        assert!(text.contains("pet name: Biscuit"));
        assert!(text.contains("pet traits: curious"));
        assert!(text.contains("pet traits: dramatic"));
        assert!(!text.contains("rates"));
        assert!(!text.contains("past_work"));
    }
}
