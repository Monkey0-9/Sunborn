# Sunborn Applications (`apps/`)

Standalone command-line applications and administrative utilities for the Sunborn platform.

## Applications

- **`sunborn-rebuild`**: Offline vector index reconstruction, graph re-indexing, and transactional WAL replay utility.
- **`sunborn-verify`**: Standalone verification tool for evaluating cryptographic reproducibility certificates, Merkle proofs, and tenant isolation invariants without running the full gateway.

## Usage

```bash
cargo run --bin sunborn-rebuild -- --help
cargo run --bin sunborn-verify -- --help
```
