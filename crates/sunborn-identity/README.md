# sunborn-identity

Multi-tenant cryptographic identity, deterministic RFC 8785 JSON canonicalization, inline zero-leakage PII tokenization, and $O(1)$ AEAD tenant crypto-shredding for the Sunborn platform.

## Architecture & Features

### 1. RFC 8785 JSON Canonicalization Scheme (JCS) & Unicode Normalization
- **Unicode NFC Normalization**: Decomposes and recomposes Unicode glyphs to canonical composed forms (e.g., NFD `cafe\u{0301}` $\to$ NFC `café`).
- **Deterministic Key Sorting**: Strictly orders JSON object properties by UTF-16 code units per RFC 8785 §3.2.3.
- **IEEE-754 Numeric Formatting**: Strips superfluous trailing decimals and unifies negative zero (`-0.0` $\to$ `0`).

### 2. State-Complete 10-Dimensional Cache Identity
Computes cryptographic cache identity keys over 10 independent operational dimensions:
1. `tenant_id` and `application_id`
2. `model` and policy version
3. Canonical system prompt and prompt text
4. IEEE 754 sampling parameter gate ($t=0$ deterministic isolation vs $(t, p, \text{seed})$ tuples)
5. Token budget bounds (`max_tokens`)
6. Deterministically sorted stop sequences
7. Tool schemas and definitions
8. Tool choices and constraints
9. Response format requirements (e.g. JSON schema)
10. Multi-turn conversation history digests and retrieval context hashes

### 3. Zero-Leakage Inline PII Tokenization
Compliant with SOC 2 Type II, HIPAA, and PCI-DSS requirements:
- Redacts sensitive identifiers (SSNs, credit card numbers, Bearer/API tokens, email addresses, phone numbers) into typed tokens (`<US_SSN_1>`, `<EMAIL_1>`).
- Retains raw sensitive data exclusively in an ephemeral, in-memory, request-scoped `PiiVaultSession`.
- Allows downstream caches, embeddings, and vector databases to index queries without persisting unredacted PII.
- Automatically re-hydrates responses before returning data to the authenticated client.

### 4. $O(1)$ Cryptographic Tenant Shredding (GDPR Article 17)
- Encrypts each tenant's sensitive cached outputs with a distinct 256-bit Tenant Data Encryption Key (TDEK) using AES-256-GCM authenticated encryption (RFC 5116 / NIST SP 800-38D).
- Uses cryptographically secure 96-bit random nonces derived from OS entropy.
- Supports pluggable Key Management Providers:
  - **Local AES-256-GCM AEAD**: High-performance local encryption for development and standalone environments.
  - **AWS KMS**: Hardware-backed envelope key wrapping (`aws-sdk-kms`) with live canary probes.
  - **HashiCorp Vault Transit**: Enterprise transit engine key wrapping.
  - **PKCS#11 HSM**: Physical Hardware Security Module integration.
- Shredding a tenant's TDEK executes in $< 1\,\text{ms}$ in $O(1)$ time, instantly rendering all stored ciphertext across memory, Redis/Dragonfly, PostgreSQL, and vector stores unrecoverable white noise.

## Testing

```bash
cargo test -p sunborn-identity
cargo test --doc -p sunborn-identity
```
