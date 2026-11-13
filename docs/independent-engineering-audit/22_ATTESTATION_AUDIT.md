# EXECUTION ATTESTATION & CRYPTOGRAPHIC PROVENANCE AUDIT

**Audit Standard**: RFC 6962 / NIST SP 800-57 / FIPS 198-1 (HMAC)  
**Scope**: Cryptographic certificates, Merkle proofs, attestation generation and verification  
**Auditor**: Independent Principal Cryptographic Systems Auditor  

---

## 📜 Cryptographic `ReuseCertificate` Specification

Every entry admitted into Sunborn's exact cache tier carries an authoritative `ReuseCertificate`:

```rust
pub struct ReuseCertificate {
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub model_name: String,
    pub input_hash: String,            // SHA-256 of JCS canonicalized request
    pub output_hash: String,           // SHA-256 of decrypted response body
    pub merkle_root: String,           // RFC 6962 root of knowledge sources
    pub valid_from: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
    pub monotonic_epoch_lease: u64,
    pub signature: String,             // HMAC-SHA256 signature
}
```

---

## 🔒 Cryptographic Key Separation (Audit Hardening)

To prevent security role confusion, Sunborn enforces strict **cryptographic key separation**:
- **Master KEK (`SUNBORN_MASTER_KEK`)**: Used solely for wrapping Tenant Data Encryption Keys (TDEKs) in AES-256-GCM envelope encryption.
- **Certificate Signing Key (`SUNBORN_CERTIFICATE_SIGNING_KEY`)**: Dedicated 32-byte secret used solely for signing and verifying `ReuseCertificate` tokens. If unspecified in development, derived via HMAC-SHA256 from master KEK with domain separation string `sunborn-certificate-signing-v1:`.
- **Learning HMAC Secret (`SUNBORN_LEARNING_HMAC_SECRET`)**: Used solely for authenticating adaptive reinforcement feedback and canary promotion records.
- **Admin Key (`SUNBORN_ADMIN_KEY`)**: Used solely for control plane API authorization.

**Production Invariant**: In production mode (`SUNBORN_ENV=production`), `AppState::verify_production_readiness` asserts that all four keys are pairwise distinct and possess at least 32 bytes of high entropy. Violation causes immediate startup termination.

---

## 🌳 RFC 6962 Merkle Inclusion Proofs

For agentic tool DAGs, Sunborn generates inclusion proofs certifying that a replayed step was computed from authentic parent nodes:
1. **Leaf Hash**: $\text{SHA256}(0x00 \parallel \text{node\_id} \parallel \text{tool} \parallel \text{input\_digest})$
2. **Internal Node Hash**: $\text{SHA256}(0x01 \parallel \text{left} \parallel \text{right})$
3. **Audit Path**: Logarithmic sequence of sibling hashes proving inclusion in `merkle_root`.
4. **Verification**: Can be independently verified by external client applications without trusting gateway memory.
- **Verification Evidence**: Tested in `crates/sunborn-execution/src/merkle.rs::tests`.
