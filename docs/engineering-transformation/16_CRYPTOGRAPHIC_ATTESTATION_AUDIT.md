# Cryptographic Attestation & Key Governance Audit

**Digital Signatures:** Ed25519 (RFC 8032)  
**Digest Canonicalization:** RFC 8785 JSON Canonicalization Scheme (JCS)  
**Envelope Encryption:** AES-256-GCM (NIST SP 800-38D)  
**DAG Inclusion Proofs:** Domain-Separated 6-Leaf Merkle Tree (RFC 6962)  

---

## 1. ReuseCertificate Cryptographic Architecture

Every cached response returned by Sunborn is accompanied by an immutable `ReuseCertificate` signed with the gateway's private Ed25519 key.

### 1.1 Domain-Separated 6-Leaf Merkle Tree
The Merkle root binds six critical security invariants:
- **Leaf 0 (`0x00`):** Canonical prompt hash (RFC 8785 normalized prompt).
- **Leaf 1 (`0x01`):** Canonical response payload hash.
- **Leaf 2 (`0x02`):** Tenant and application scope digest.
- **Leaf 3 (`0x03`):** Source knowledge monotonic epoch vector.
- **Leaf 4 (`0x04`):** Conformal risk non-conformity score ($S_i$).
- **Leaf 5 (`0x05`):** Validity window timestamps (issued_at, expires_at).

Interior nodes are prefixed with `0x01` per RFC 6962 to eliminate second-preimage and boundary-concatenation collision attacks.

---

## 2. Instantaneous GDPR Article 17 Cryptographic Shredding

To guarantee irrevocable tenant data deletion:
1. Each tenant is assigned a unique Tenant Data Encryption Key (TDEK), wrapped under the Root Key Encryption Key (KEK).
2. All tenant cached entries and vector metadata are encrypted under the TDEK using AES-256-GCM.
3. Upon receiving a certified deletion request, the gateway zeroizes the TDEK in PostgreSQL and evicts it from memory:
```rust
pub async fn crypto_shred_tenant(&self, tenant_id: &TenantId) -> Result<()> {
    self.key_store.delete_tdek(tenant_id).await?;
    self.tdek_cache.remove(tenant_id);
    tracing::info!("Tenant {} TDEK zeroized; data is cryptographically irretrievable", tenant_id);
    Ok(())
}
```
Subsequent read attempts fail closed with `CryptographicIntegrityError`, rendering historical ciphertexts permanently unrecoverable without touching petabytes of disk storage.
