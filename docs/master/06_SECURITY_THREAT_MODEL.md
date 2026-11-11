# Sunborn Security Threat Model & Cryptographic Trust Specification

**Document Version:** 2.0.0  
**Effective Commit:** `7f1f161`  
**Standard:** NIST SP 800-38D, RFC 8785, STRIDE Methodology, OWASP Top 10 for LLMs

---

## 1. STRIDE Threat Model & Defense Controls

| Threat Category | Potential Attack Vector | Sunborn Architectural Defense | Verification Test |
|---|---|---|---|
| **Spoofing** | Forging tenant identity or administrative tokens | Cryptographic API key authentication; constant-time token comparison; tenant bounds strictly derived from key metadata, not client headers. | `test_tenant_spoofing_rejected_with_authenticated_key` |
| **Tampering** | Modifying execution certificates or cached responses | HMAC-SHA256 authenticated `ReuseCertificate` binding request hash, Merkle root, epoch, and parameters. AEAD AES-256-GCM auth tags reject tampered ciphertext. | `test_reuse_certificate_tampered_merkle_root_abstains_to_revalidate` |
| **Repudiation** | Denying an administrative deletion or spend event | Immutable `audit_events` and `finops_ledger` tables in PostgreSQL; dual-admin quorum signatures recorded for destructive endpoints. | `test_adversarial_dual_admin_quorum_authorization_enforcement` |
| **Information Disclosure** | Cross-tenant cache poisoning, PII leakage in embeddings | Strict tenant namespace scoping on all cache keys and vector queries; inline DFA/NER PII tokenization replaces sensitive plaintext before caching. | `test_cross_tenant_isolation_zero_leakage` & `test_pii_redaction_and_canonicalization` |
| **Denial of Service** | Cache stampede, unbounded queries, resource exhaustion | Singleflight coalescing for concurrent misses; FinOps spend velocity controller; token bucket rate limiting on ingress. | `test_singleflight_streamer_multiplexing` & `test_login_rate_limiting_enforcement_429` |
| **Elevation of Privilege** | Non-admin caller calling destructive endpoints | Deny-by-default RBAC; dual-admin quorum authorization (`X-Sunborn-Admin-Key` + `X-Sunborn-Quorum-Key`); fail-closed BFF proxy. | `test_non_admin_forbidden_on_all_destructive_endpoints` |

---

## 2. Cryptographic Architecture & Key Management

### Multi-Tier Envelope Encryption
Every tenant payload is encrypted using standard AES-256-GCM authenticated encryption (NIST SP 800-38D):
1. **Fresh 96-Bit Nonce**: Generated from OS entropy (`rand_core::OsRng`). Reused nonces fail closed.
2. **Tenant Data Encryption Key (TDEK)**: Unique 256-bit key per tenant, held in `ZeroizingKey` memory wrappers.
3. **Key Encryption Key (KEK)**: Root master key held in external Key Management Systems (AWS KMS, Vault Transit, or PKCS#11 HSM).
4. **Authenticated Additional Data (AAD)**: Encrypted payloads bind tenant ID, application ID, and version string in the AAD to prevent ciphertext substitution.

### $\mathcal{O}(1)$ Cryptographic Shredding (GDPR Article 17)
To fulfill the *Right to be Forgotten* across petabyte-scale distributed storage without performing slow, locking database table scans:
1. When tenant deletion is authorized, the gateway destroys the wrapped TDEK in `tenant_wrapped_keys`.
2. All in-memory copies of the TDEK are zeroized using `zeroize::Zeroize`.
3. Without the 256-bit key, all historical ciphertexts stored in PostgreSQL, DragonflyDB, L1 memory, and vector databases instantaneously become unrecoverable random noise ($\mathcal{O}(1)$ execution time).

---

## 3. Adversarial Prompt Defense & Linguistic Invariance

To prevent adversarial prompt injection (OWASP LLM-01) from poisoning the semantic cache:
1. **Structural Transposition Defense (Gap 2)**: Extracts role-bound numeric tokens. Queries swapping roles (e.g. sender and recipient) are rejected before vector lookup.
2. **Negation & Antonym Inversion**: Detects negation modifiers ("not", "never", "unapproved") and polarity antonyms.
3. **Strict Parameter Bounds**: Floating point parameters (`temperature`, `top_p`) reject `NaN` and `Infinity` values.
