# 26 — Cryptographic Validation

---

## 1. Primitives (all established libraries, no custom crypto)

| Primitive | Use | Library | Verdict |
|---|---|---|---|
| AES-256-GCM | envelope encryption | `aes-gcm` | correct: 96-bit nonce, AAD, tag check |
| HMAC-SHA-256 | certificate signature | `hmac`/`sha2` | correct, constant-time compare |
| SHA-256 | identity/hashing/Merkle | `sha2` | correct |
| OS RNG | nonces/keys | `getrandom` via aes-gcm | correct |
| Zeroize | key material | `zeroize` | correct |

No custom block ciphers, no homemade MACs, no RSA hand-rolling.

## 2. Envelope encryption

`crypto_shred.rs:1102-1189`:
- Per-tenant TDEK wrapped under master KEK.
- `EncryptedEnvelope{tenant_id,nonce,ciphertext,auth_tag,key_version}`.
- Decrypt rejects tag mismatch.
- `ZeroizingKey` implements `Zeroize` + `Drop`.

**Verdict:** sound.

## 3. Crypto-shredding (GDPR Art. 17)

- Deleting the wrapped TDEK makes ciphertext unrecoverable in O(1).
- Tested (`crypto_shred.rs:1218+`, 11 AEAD tests).
- Caveat: shred only guarantees unrecoverability if no plaintext copies exist outside the key scope (logs, backups, upstream provider). Documented limitation required.

**Verdict:** sound within stated scope.

## 4. Certificate signing

- Merkle over 5 leaves with domain separation, length prefixing.
- HMAC signature, constant-time verification.
- **Weakness F-015:** signing key == learning-feedback HMAC secret.
- **Weakness F-004:** evidence payload not measured.
- No key id, no rotation, no asymmetric signature (HMAC = symmetric; verifier must share the secret, so no independent third-party verification).

**Verdict:** integrity primitives sound; key management and independent verifiability insufficient.

## 5. Merkle DAG / inclusion proofs

- Domain-separated tags, LE length framing, sorted parents.
- Odd-tail duplication for tree build.
- Tamper and avalanche tests present.
- Not byte-exact RFC 6962 audit path (leaves pre-hashed externally).

**Verdict:** sound for the implemented contract.

## 6. Randomness

- 96-bit nonces from OS RNG per encryption.
- No nonce reuse detected in reviewed code.

## 7. Key providers

- Local AEAD, AWS KMS (`KmsClient`), Vault Transit (HTTP) implemented.
- `Pkcs11HsmKeyProvider` and some providers are **documented test doubles** (`crypto_shred.rs:254,319,395`).
- Production selection fails closed when credentials absent (`state.rs:204-230`).

## 8. Claims check

| Claim | Verdict |
|---|---|
| AES-256-GCM per-tenant | TRUE |
| O(1) crypto-shred | TRUE (key deletion) |
| "Dual-admin cryptographic sign-offs" | FALSE — second API key header, not a signature |
| "Independent verifiable attestation" | FALSE — symmetric HMAC, no verifier CLI |
| "FIPS/PKCS#11" | NOT IMPLEMENTED (test double) |

## 9. Verdict

Crypto primitives and envelope encryption are competently implemented. The **guarantee narrative around attestation is not justified**: symmetric keyed MAC, shared key role, unmeasured evidence, no third-party verifier. Crypto score: **6/10** (primitives 8, key management/attestation 4).
