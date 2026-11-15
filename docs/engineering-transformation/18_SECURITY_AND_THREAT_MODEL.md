# Application Security & STRIDE Threat Model Audit

**Standard:** OWASP Application Security Verification Standard (ASVS) 4.0 Level 3  
**Threat Modeling Methodology:** STRIDE (Spoofing, Tampering, Repudiation, Information Disclosure, DoS, Elevation)  
**Side-Channel Mitigations:** `subtle::ConstantTimeEq`, Memory Zeroization  
**Privilege Escalation Defense:** Dual-Admin Quorum Authorization  

---

## 1. STRIDE Threat Model & Mitigations

| STRIDE Threat | Attack Vector | Implemented Mitigation | Verification Test |
| :--- | :--- | :--- | :--- |
| **Spoofing** | Forging `X-Tenant-ID` header to access peer tenant data | Gateway resolves tenant strictly from validated API key in `middleware/auth.rs`. Unauthenticated or mismatched tenant headers are rejected with 401/403. | `crates/sunborn-gateway/tests/security_test.rs:test_tenant_spoofing_rejected` |
| **Tampering** | Modifying cached responses or vector embeddings | Cached payloads are signed with Ed25519 `ReuseCertificate`. Tampered payloads fail cryptographic MAC verification. | `apps/sunborn-verify/src/main.rs:test_tamper_detection` |
| **Repudiation** | Denying an administrative cache purge or shredding event | All administrative mutations require dual-admin cryptographic signatures logged to immutable PostgreSQL audit trail. | `crates/sunborn-gateway/tests/quorum_authorization_test.rs` |
| **Information Disclosure** | Timing attacks against API key comparisons | API keys and session tokens use `subtle::ConstantTimeEq` to prevent micro-architectural timing leaks. | `crates/sunborn-gateway/tests/security_test.rs` |
| **Denial of Service** | Thundering herds on LLM providers under cache miss spikes | Singleflight request multiplexing coalesces concurrent identical prompts into a single origin stream. | `crates/sunborn-gateway/src/origin/singleflight.rs` |
| **Elevation of Privilege** | Compromised individual admin key attempting tenant wipe | Destructive operations (`/v1/admin/purge`) require $M$-of-$N$ quorum verification before execution. | `crates/sunborn-gateway/tests/quorum_authorization_test.rs` |
