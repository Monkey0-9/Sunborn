# Sunborn: Security Architecture, STRIDE Threat Model & Adversarial Hardening

**Target Standard:** Enterprise Security & Adversarial Threat Modeling  
**Document Version:** 1.0.0  
**Baseline Commit:** `dd25675c5eae200b2518557f966feff74d1556fe`  
**Primary Test Sources:**  
- [`crates/sunborn-gateway/tests/adversarial_security_suite.rs`](file:///C:/Sunborn/crates/sunborn-gateway/tests/adversarial_security_suite.rs)  
- [`frontend/tests/bff_security.test.mjs`](file:///C:/Sunborn/frontend/tests/bff_security.test.mjs)  

---

## 1. STRIDE Threat Model & Defense Matrix

| Threat Category | Specific Attack Vector | Target Subsystem | Sunborn Defense Mechanism | Verified Invariant & Test Source |
|:---|:---|:---|:---|:---|
| **Spoofing** | Forged API Keys / Ambient Cookies | Auth Middleware / BFF | Strict constant-time MAC verification; zero ambient cookie trust on API routes. | `unexpired_api_credentials_remain_valid`, `bff_security.test.mjs` |
| **Spoofing** | Forged Attestation Certificate | Certificate Verifier | Asymmetric signature verification; unauthenticated certificates marked `UNAUTHENTICATED`. | `research/tests/test_verifier_negative.py` |
| **Tampering** | In-flight payload alteration | Execution DAG / Cache | AES-256-GCM authentication tags (128-bit); Merkle inclusion proofs. | `crypto_shred::tests::test_tampered_ciphertext_fails_mac_verification` |
| **Tampering** | JSON property reordering | JCS Canonicalizer | RFC 8785 deterministic sorting guarantees byte-identical serialization. | `identity::tests::test_identity_determinism` |
| **Repudiation** | LLM output or origin denial | Cryptographic Provenance | SHA-256 Merkle root binding model identity, prompt hash, and source epochs. | `certificate::tests::test_certificate_creation_integrity_and_tamper_detection` |
| **Info Disclosure** | Cross-tenant cache poisoning | Multi-Tenant Cache | Tenant UUID prefixing on all Redis keys, Qdrant payload filters, and Roaring bitmaps. | `test_adversarial_cross_tenant_cache_poisoning_isolation` |
| **Info Disclosure** | PII leakage to third-party LLMs | Gateway Privacy Layer | Automated regex redaction for SSNs, credit cards, emails before egress. | `privacy::tests::test_pii_redaction_and_canonicalization` |
| **Denial of Service**| 64KB+ Oversized HTTP Headers | Axum Gateway Edge | Axum request limits enforce hard 8KB header ceiling; rejects with HTTP 431. | `test_adversarial_oversized_64kb_header_dos_real_http_stack` |
| **Denial of Service**| Deeply nested JSON / Billion Laughs | Serde JSON Deserializer | Recursion depth limit hard-capped at 64; parsing immediately aborts on overflow. | `test_adversarial_malformed_json_and_deep_nesting` |
| **Elevation of Priv**| Non-admin user accessing `/admin/*` | Gateway RBAC / BFF | Deny-by-default role validator; non-admin tokens strictly rejected with HTTP 403. | `test_adversarial_non_admin_privilege_escalation_denied` |

---

## 2. Empirical Adversarial Suite Verification

The 14-test adversarial test suite ([`adversarial_security_suite.rs`](file:///C:/Sunborn/crates/sunborn-gateway/tests/adversarial_security_suite.rs)) validates edge security boundaries against real malicious payloads:

1. **CRLF Header Injection:** HTTP response splitting attempts via `\r\nSet-Cookie:` are strictly stripped.
2. **Null Byte Injection:** `%00` characters in tenant and entity IDs are sanitized.
3. **Unicode Bidi Override:** Bi-directional text overrides (`\u202E`) engineered to invert visual prompt logic are detected and neutralized.
4. **Unicode Homoglyphs & Zero-Width Spaces:** Cyrillic homoglyphs and zero-width spaces (`\u200B`) are normalized during JCS canonicalization.
5. **SQL Injection Payloads:** Injected SQL commands in prompt fields (`' OR '1'='1`) are isolated and treated purely as opaque string literals.
6. **XSS Script Payloads:** `<script>alert(1)</script>` payloads are escaped and prevented from executing in dashboard views.
7. **Brute Force Lockout:** Rapid repeated authentication failures trigger automatic IP/token lockout via spend velocity rate limiters.
8. **Path Traversal on Dynamic Routes:** `../../etc/passwd` path traversal attacks are rejected by Axum router path normalizers.

---

## 3. Cryptographic Lifecycle & Key Security

- **Constant-Time Verification:** All signature and API key comparisons use constant-time equality checks (`subtle::ConstantTimeEq`) to eliminate timing side-channels.
- **Key Separation Invariant:** The gateway verifies during startup that administrative keys, master KEKs, and HMAC secrets are completely distinct 256-bit values. Reusing the master KEK as the HMAC secret causes an immediate, fail-closed startup abort ([`production_startup_test.rs`](file:///C:/Sunborn/crates/sunborn-gateway/tests/production_startup_test.rs)).
