// ============================================================================
// crates/sunborn-identity/tests/fuzz_canonicalize_test.rs
// Automated Fuzz & Invariant Testing for RFC 8785 JCS & Unicode Normalization (DEF-019)
// ============================================================================

use serde_json::{json, Map, Value};
use sunborn_identity::{
    canonicalize_json, canonicalize_json_rfc8785, canonicalize_text, normalize_unicode_nfc,
    privacy::PiiTokenizer,
};

/// Deterministic Linear Congruential Generator (LCG) for reproducible pseudo-random testing
struct FuzzRng {
    state: u64,
}

impl FuzzRng {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u32(&mut self) -> u32 {
        self.state = self
            .state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.state >> 32) as u32
    }

    fn next_in_range(&mut self, min: usize, max: usize) -> usize {
        let span = max - min + 1;
        min + (self.next_u32() as usize % span)
    }

    fn next_f64(&mut self) -> f64 {
        let u = self.next_u32();
        (u as f64) / (u32::MAX as f64)
    }

    fn random_string(&mut self, max_len: usize) -> String {
        const CHAR_POOL: &[char] = &[
            'a', 'b', 'c', '1', '2', '3', ' ', '\t', '\r', '\n', 'é', 'è', 'ê', 'ë', 'á', 'à', 'â',
            'ä', 'í', 'ó', 'ú', 'ç', 'ñ', '\u{0301}', '\u{0300}', '\u{0302}', '\u{0308}',
            '\u{0327}', '\u{0303}', // Combining marks
            '🚀', '💥', '🛡', '中', '文', '日', '本', '語', '"', '\\', '/', '{', '}', '[', ']',
        ];
        let len = self.next_in_range(0, max_len);
        let mut s = String::with_capacity(len * 4);
        for _ in 0..len {
            let idx = self.next_in_range(0, CHAR_POOL.len() - 1);
            s.push(CHAR_POOL[idx]);
        }
        s
    }

    fn random_json_value(&mut self, depth: usize) -> Value {
        if depth > 4 || (depth > 1 && self.next_in_range(0, 10) < 4) {
            // Leaf node
            match self.next_in_range(0, 5) {
                0 => Value::Null,
                1 => Value::Bool(self.next_u32() % 2 == 0),
                2 => {
                    let num_type = self.next_in_range(0, 4);
                    match num_type {
                        0 => json!(0),
                        1 => json!(-0.0),
                        2 => json!(self.next_u32() as i64 - 1000),
                        3 => {
                            let f = ((self.next_f64() - 0.5) * 10000.0 * 1000.0).round() / 1000.0;
                            json!(f)
                        }
                        _ => json!(42.0),
                    }
                }
                3 => Value::String(self.random_string(30)),
                _ => Value::String("".to_string()),
            }
        } else {
            // Container node
            if self.next_in_range(0, 1) == 0 {
                // Object
                let entries_count = self.next_in_range(0, 6);
                let mut map = Map::new();
                for _ in 0..entries_count {
                    let key = format!("k_{}_{}", self.random_string(8), self.next_u32() % 100);
                    let val = self.random_json_value(depth + 1);
                    map.insert(key, val);
                }
                Value::Object(map)
            } else {
                // Array
                let items_count = self.next_in_range(0, 5);
                let mut arr = Vec::with_capacity(items_count);
                for _ in 0..items_count {
                    arr.push(self.random_json_value(depth + 1));
                }
                Value::Array(arr)
            }
        }
    }
}

#[test]
fn test_fuzz_text_canonicalization_invariants() {
    let mut rng = FuzzRng::new(0xCAFE_BABE_F00D_1234);

    for iter in 0..1_000 {
        let input = rng.random_string(80);

        // Invariant 1: No panics on arbitrary Unicode and combining marks
        let norm = normalize_unicode_nfc(&input);
        assert!(
            !norm.is_empty() || input.is_empty(),
            "Iter {}: normalization non-empty check",
            iter
        );

        // Invariant 2: Text canonicalization idempotency
        let c1 = canonicalize_text(&input);
        let c2 = canonicalize_text(&c1);
        assert_eq!(
            c1, c2,
            "Iter {}: canonicalize_text must be idempotent for input: {:?}",
            iter, input
        );

        // Invariant 3: Combining character equivalence (NFD decomposed vs NFC composed)
        let decomposed = "e\u{0301}";
        let composed = "é";
        assert_eq!(normalize_unicode_nfc(decomposed), composed);
        assert_eq!(canonicalize_text(decomposed), canonicalize_text(composed));
    }
}

#[test]
fn test_fuzz_json_canonicalization_and_rfc8785_invariants() {
    let mut rng = FuzzRng::new(0xDEAD_BEEF_5678_90AB);

    for iter in 0..500 {
        let val = rng.random_json_value(0);

        // Invariant 1: canonicalize_json must not panic and must be strictly idempotent
        let canon1 = canonicalize_json(&val);
        let canon2 = canonicalize_json(&canon1);
        assert_eq!(
            canon1, canon2,
            "Iter {}: canonicalize_json idempotency violated on {:?}",
            iter, val
        );

        // Invariant 2: Keys in canonicalized JSON must be sorted lexicographically
        if let Value::Object(ref map) = canon1 {
            let keys: Vec<&String> = map.keys().collect();
            let mut sorted_keys = keys.clone();
            sorted_keys.sort();
            assert_eq!(keys, sorted_keys, "Iter {}: Object keys not sorted", iter);
        }

        // Invariant 3: RFC 8785 canonicalization produces valid JSON string
        let rfc_str = canonicalize_json_rfc8785(&val);
        let reparsed: Result<Value, _> = serde_json::from_str(&rfc_str);
        assert!(
            reparsed.is_ok(),
            "Iter {}: RFC 8785 output was not valid JSON: {:?}",
            iter,
            rfc_str
        );

        // Invariant 4: RFC 8785 serialization is strictly idempotent
        if let Ok(ref parsed_val) = reparsed {
            let rfc_str2 = canonicalize_json_rfc8785(parsed_val);
            assert_eq!(
                rfc_str, rfc_str2,
                "Iter {}: RFC 8785 serialization round-trip idempotency violated",
                iter
            );
        }
    }
}

#[test]
fn test_fuzz_numeric_edge_cases_rfc8785() {
    // Edge cases specified in RFC 8785 Section 3.2.2.3:
    // 0.0 -> "0", -0.0 -> "0", integer representations, scientific notations
    let cases = vec![
        (json!(0), "0"),
        (json!(0.0), "0"),
        (json!(-0.0), "0"),
        (json!(100), "100"),
        (json!(-42), "-42"),
        (json!(1.0), "1"),
        (json!(100000.0), "100000"),
    ];

    for (val, expected) in cases {
        let res = canonicalize_json_rfc8785(&val);
        assert_eq!(
            res, expected,
            "RFC 8785 numeric formatting failed for {:?}",
            val
        );
    }
}

#[test]
fn test_fuzz_pii_tokenizer_no_panics_and_ephemeral_hydration() {
    let mut rng = FuzzRng::new(0x1337_C0DE_FACE_FEED);

    for iter in 0..300 {
        let prefix = rng.random_string(20);
        let ssn = format!(
            "{:03}-{:02}-{:04}",
            rng.next_in_range(100, 999),
            rng.next_in_range(10, 99),
            rng.next_in_range(1000, 9999)
        );
        let email = format!("user_{}@corp.com", rng.next_in_range(1, 99999));
        let suffix = rng.random_string(20);

        let input = format!("{} ssn: {} email: {} {}", prefix, ssn, email, suffix);

        // Fuzz invariant 1: Tokenizer never panics
        let (tokenized, session) = PiiTokenizer::tokenize(&input);

        // Fuzz invariant 2: Sensitive PII must not appear in the tokenized output
        assert!(
            !tokenized.contains(&ssn),
            "Iter {}: SSN leaked into tokenized string: {:?}",
            iter,
            tokenized
        );
        assert!(
            !tokenized.contains(&email),
            "Iter {}: Email leaked into tokenized string: {:?}",
            iter,
            tokenized
        );

        // Fuzz invariant 3: Ephemeral session hydration recovers original text exactly
        let hydrated = session.hydrate(&tokenized);
        assert_eq!(
            hydrated, input,
            "Iter {}: Hydration failed to round-trip original text",
            iter
        );
    }
}
