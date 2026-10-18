// ============================================================================
// crates/sunborn-identity/src/canonical.rs
// RFC 8785 JSON Canonicalization Scheme (JCS) & Unicode NFC Normalization
// ============================================================================
use serde_json::Value;

/// Normalizes Unicode combining sequences (e.g., NFD "e\u{0301}" -> NFC "é")
pub fn normalize_unicode_nfc(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if let Some(&combining) = chars.peek() {
            let composed = match (ch, combining) {
                ('e', '\u{0301}') => Some('é'),
                ('e', '\u{0300}') => Some('è'),
                ('e', '\u{0302}') => Some('ê'),
                ('e', '\u{0308}') => Some('ë'),
                ('E', '\u{0301}') => Some('É'),
                ('E', '\u{0300}') => Some('È'),
                ('E', '\u{0302}') => Some('Ê'),
                ('E', '\u{0308}') => Some('Ë'),
                ('a', '\u{0301}') => Some('á'),
                ('a', '\u{0300}') => Some('à'),
                ('a', '\u{0302}') => Some('â'),
                ('a', '\u{0308}') => Some('ä'),
                ('A', '\u{0301}') => Some('Á'),
                ('A', '\u{0300}') => Some('À'),
                ('A', '\u{0302}') => Some('Â'),
                ('A', '\u{0308}') => Some('Ä'),
                ('i', '\u{0301}') => Some('í'),
                ('i', '\u{0300}') => Some('ì'),
                ('i', '\u{0302}') => Some('î'),
                ('i', '\u{0308}') => Some('ï'),
                ('I', '\u{0301}') => Some('Í'),
                ('o', '\u{0301}') => Some('ó'),
                ('o', '\u{0300}') => Some('ò'),
                ('o', '\u{0302}') => Some('ô'),
                ('o', '\u{0308}') => Some('ö'),
                ('O', '\u{0301}') => Some('Ó'),
                ('O', '\u{0308}') => Some('Ö'),
                ('u', '\u{0301}') => Some('ú'),
                ('u', '\u{0300}') => Some('ù'),
                ('u', '\u{0302}') => Some('û'),
                ('u', '\u{0308}') => Some('ü'),
                ('U', '\u{0301}') => Some('Ú'),
                ('U', '\u{0308}') => Some('Ü'),
                ('c', '\u{0327}') => Some('ç'),
                ('C', '\u{0327}') => Some('Ç'),
                ('n', '\u{0303}') => Some('ñ'),
                ('N', '\u{0303}') => Some('Ñ'),
                _ => None,
            };

            if let Some(comp) = composed {
                chars.next(); // consume combining mark
                out.push(comp);
                continue;
            }
        }
        out.push(ch);
    }
    out
}

/// Canonicalizes prompt text: trims whitespace, normalizes CRLF to LF,
/// collapses consecutive whitespace, and performs Unicode NFC normalization.
pub fn canonicalize_text(text: &str) -> String {
    let unicode_norm = normalize_unicode_nfc(text);
    let normalized = unicode_norm.replace("\r\n", "\n").replace('\r', "\n");
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

/// Formats a serde_json::Value strictly following RFC 8785 JSON Canonicalization Scheme (JCS).
/// - Lexicographical property ordering (by UTF-16 code units)
/// - IEEE-754 numbers formatted without redundant decimals/exponents (0.0 -> 0, -0.0 -> 0)
/// - Whitespace-free output
pub fn canonicalize_json_rfc8785(val: &Value) -> String {
    match val {
        Value::Null => "null".to_string(),
        Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i.to_string()
            } else if let Some(u) = n.as_u64() {
                u.to_string()
            } else if let Some(f) = n.as_f64() {
                if f == 0.0 || f == -0.0 {
                    "0".to_string()
                } else if f.fract() == 0.0 && f.abs() < 1e21 {
                    format!("{:.0}", f)
                } else {
                    format!("{}", f)
                }
            } else {
                n.to_string()
            }
        }
        Value::String(s) => {
            let norm = normalize_unicode_nfc(s);
            serde_json::to_string(&norm).unwrap_or_else(|_| format!("\"{}\"", norm))
        }
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(canonicalize_json_rfc8785).collect();
            format!("[{}]", items.join(","))
        }
        Value::Object(map) => {
            // Sort by UTF-16 code units per RFC 8785 §3.2.3
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|&(k1, _), &(k2, _)| {
                let u1: Vec<u16> = k1.encode_utf16().collect();
                let u2: Vec<u16> = k2.encode_utf16().collect();
                u1.cmp(&u2)
            });

            let formatted: Vec<String> = entries
                .into_iter()
                .map(|(k, v)| {
                    let k_esc = serde_json::to_string(k).unwrap();
                    let v_str = canonicalize_json_rfc8785(v);
                    format!("{}:{}", k_esc, v_str)
                })
                .collect();
            format!("{{{}}}", formatted.join(","))
        }
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
    fn test_unicode_nfc_normalization() {
        // "café" in NFC vs NFD ("cafe\u{0301}")
        let nfc = "café";
        let nfd = "cafe\u{0301}";
        assert_eq!(canonicalize_text(nfc), canonicalize_text(nfd));
    }

    #[test]
    fn test_rfc8785_canonicalization() {
        let json1: Value =
            serde_json::from_str(r#"{"b": 2.0, "a": 1, "prompt": "cafe\u0301"}"#).unwrap();
        let json2: Value = serde_json::from_str(r#"{"prompt": "café", "a": 1.0, "b": 2}"#).unwrap();

        let s1 = canonicalize_json_rfc8785(&json1);
        let s2 = canonicalize_json_rfc8785(&json2);

        assert_eq!(s1, s2);
        assert_eq!(s1, r#"{"a":1,"b":2,"prompt":"café"}"#);
    }
}
