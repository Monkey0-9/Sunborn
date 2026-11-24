# sunborn-proof

Proof-Carrying Semantic Reuse (PCSR) certificate generation and zero-trust independent verification engine for Sunborn.

## Key Capabilities

1. **Self-Contained Verification Certificates**: Structured, cryptographically verifiable certificates carrying tenant scope, model fingerprints, prompt digests, context digests, dependency roots, policy versions, and freshness evidence.
2. **Ed25519 & HMAC Signatures**: Native cryptographic signing allowing third-party verification of reuse assertions without querying the primary cache database.
3. **Anti-Tamper Evidence Checks**: Independent validation verifying that model IDs, dependency epochs, and freshness thresholds have not experienced post-issuance tampering.

## Testing

```bash
cargo test -p sunborn-proof
cargo test --doc -p sunborn-proof
```
