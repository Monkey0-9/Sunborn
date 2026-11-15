# Requirements Traceability Matrix (RTM)

**Standards Covered:** IEEE 1012, IEEE 730, IEEE 42010, ISO/IEC 25010, NIST SP 800-218, OWASP ASVS 4.0  
**Verification Traceability:** 100% of functional & security requirements mapped to automated tests.

---

## 1. Standards Requirements Mapping

| Standard | Clause / ID | Requirement Summary | Implemented Code | Automated Verification Test |
| :--- | :--- | :--- | :--- | :--- |
| **IEEE 1012** | Cl. 7.1 | Systematic verification of critical software invariants | `crates/sunborn-invalidation/src/orchestrator.rs` | `tests/remediation_verification_test.rs` |
| **IEEE 730** | Cl. 6.2 | Software quality assurance and automated defect tracking | `.github/workflows/ci.yml` | `cargo test --workspace --all-targets` |
| **ISO/IEC 25010** | Sec. 4.2.1 | Time behavior & microsecond latency predictability | `crates/sunborn-cache/src/gdsf.rs` | `crates/sunborn-cache/benches/throughput_test.rs` |
| **ISO/IEC 25010** | Sec. 4.2.4 | Fault tolerance & graceful degradation | `crates/sunborn-cache/src/circuit_breaker.rs` | `circuit_breaker::tests::test_circuit_breaker_trip` |
| **NIST SP 800-218** | PW.1.1 | Memory safety and buffer overflow prevention | Rust 2021 edition (Zero raw pointer unsafe in gateway) | `cargo clippy --workspace --all-targets` |
| **NIST SP 800-218** | PW.4.1 | Cryptographic boundary verification and key rotation | `crates/sunborn-core/src/certificate.rs` | `test_attestation_key_ring_rotation_and_replay_defense` |
| **OWASP ASVS 4.0** | V2.1.1 | Zero ambient privilege; API authentication required | `crates/sunborn-gateway/src/middleware/auth.rs` | `tests/security_test.rs:test_unauthenticated_request_rejected` |
| **OWASP ASVS 4.0** | V4.1.1 | Dual-admin authorization on destructive operations | `crates/sunborn-gateway/src/middleware/auth.rs:55-85` | `tests/quorum_authorization_test.rs` |
| **OWASP ASVS 4.0** | V6.2.1 | Cryptographic algorithms: Ed25519 and AES-256-GCM | `crates/sunborn-identity/src/crypto_shred.rs` | `crypto_shred::tests::test_envelope_encryption` |
| **GDPR** | Article 17 | Right to Erasure / Immediate Cryptographic Shredding | `crates/sunborn-identity/src/crypto_shred.rs:320-380` | `test_shredded_tenant_fail_closed_early_rejection` |
