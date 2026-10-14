use serde_json::Value;

/// Canonicalizes prompt text: trims whitespace, normalizes CRLF to LF, collapses consecutive whitespace if needed
pub fn canonicalize_text(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let trimmed = normalized.trim();
    // Normalize excessive blank lines/spaces
    let mut result = String::with_capacity(trimmed.len());
    let mut prev_space = false;
    for ch in trimmed.chars() {
        if ch == '\n' {
            if prev_space {
                result.pop();
            }
            result.push('\n');
            prev_space = false;
        } else if ch.is_whitespace() {
            if !prev_space {
                result.push(' ');
                prev_space = true;
            }
        } else {
            result.push(ch);
            prev_space = false;
        }
    }
    result
}

/// Recursively sorts JSON keys deterministically to produce a stable representation
pub fn canonicalize_json(val: &Value) -> Value {
    match val {
        Value::Object(map) => {
            let mut sorted: Vec<(&String, &Value)> = map.iter().collect();
            sorted.sort_by_key(|&(k, _)| k);
            let mut new_map = serde_json::Map::new();
            for (k, v) in sorted {
                new_map.insert(k.clone(), canonicalize_json(v));
            }
            Value::Object(new_map)
        }
        Value::Array(arr) => {
            let new_arr = arr.iter().map(canonicalize_json).collect();
            Value::Array(new_arr)
        }
        other => other.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonicalize_text_whitespace() {
        let input = "  Hello   world! \r\n This is a   test. \n\n ";
        let canonical = canonicalize_text(input);
        assert_eq!(canonical, "Hello world!\n This is a test.");
    }

    #[test]
    fn test_canonicalize_json_order() {
        let json1: Value = serde_json::from_str(r#"{"b": 2, "a": 1}"#).unwrap();
        let json2: Value = serde_json::from_str(r#"{"a": 1, "b": 2}"#).unwrap();
        assert_eq!(
            serde_json::to_string(&canonicalize_json(&json1)).unwrap(),
            serde_json::to_string(&canonicalize_json(&json2)).unwrap()
        );
    }
}
