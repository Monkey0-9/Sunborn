# Sunborn Integration & End-to-End Tests (`tests/`)

Cross-crate integration tests, adversarial stress tests, and end-to-end verification suites for Sunborn.

## Scope

- **Multi-Replica Consistency**: Validates atomic invalidation propagation and monotonic epoch leases across distributed gateway clusters.
- **Cryptographic Zeroization**: Confirms physical disk erasure and cryptographic key shredding during tenant offboarding.
- **Adversarial Resilience**: Tests gateway behavior under prompt injection, denial-of-service traffic spikes, and network partitions.

## Running Tests

```bash
cargo test --test '*'
```
