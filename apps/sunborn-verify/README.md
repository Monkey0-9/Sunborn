# sunborn-verify

Standalone verification utility for evaluating cryptographic reproducibility certificates, Ed25519 reuse attestations, and OpenAPI specification contract parity without running the full gateway.

## Capabilities

1. **Independent Reuse Attestation Verification**: Verifies signed `ReuseCertificate` structures using raw 32-byte Ed25519 public keys or HMAC digests without accessing the primary PostgreSQL or vector databases.
2. **OpenAPI Specification Parity**: Statically parses and checks `docs/api/openapi.yaml` against Axum router endpoints in `crates/sunborn-gateway/src/lib.rs` to ensure 100% route and method contract parity.

## Usage

```bash
# Verify OpenAPI contract parity
cargo run --bin sunborn-verify -- --openapi

# Verify cryptographic reuse certificate
cargo run --bin sunborn-verify -- --certificate cert.json --public-key <32-byte-hex-key>

# Display CLI options
cargo run --bin sunborn-verify -- --help
```
