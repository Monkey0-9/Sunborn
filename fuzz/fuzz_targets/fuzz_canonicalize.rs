#![no_main]

use libfuzzer_sys::fuzz_target;
use sunborn_identity::{
    canonicalize_json, canonicalize_json_rfc8785, canonicalize_text, normalize_unicode_nfc,
};

fuzz_target!(|data: &[u8]| {
    // 1. Fuzz Unicode & text normalization with raw byte streams
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = normalize_unicode_nfc(s);
        let c1 = canonicalize_text(s);
        let c2 = canonicalize_text(&c1);
        // Idempotency check
        assert_eq!(c1, c2, "canonicalize_text must be idempotent");

        // 2. Fuzz JSON canonicalization if valid JSON
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(s) {
            let canon_v = canonicalize_json(&val);
            let canon_v2 = canonicalize_json(&canon_v);
            assert_eq!(canon_v, canon_v2, "canonicalize_json must be idempotent");

            let rfc_str = canonicalize_json_rfc8785(&val);
            if let Ok(reparsed) = serde_json::from_str::<serde_json::Value>(&rfc_str) {
                let rfc_str2 = canonicalize_json_rfc8785(&reparsed);
                assert_eq!(
                    rfc_str, rfc_str2,
                    "RFC 8785 serialization must be idempotent"
                );
            }
        }
    }
});
