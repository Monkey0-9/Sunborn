# Sunborn Integration & End-to-End Tests (`tests/`)

Cross-crate integration tests, adversarial stress suites, and high-concurrency race condition audits for Sunborn.

## Test Suites & Scenarios

- **Multi-Replica Consistency**: Validates atomic invalidation propagation and monotonic epoch leases across distributed gateway clusters.
- **Cryptographic Zeroization**: Confirms physical disk erasure and cryptographic key shredding during tenant offboarding (DPDP / GDPR Article 17).
- **Adversarial Resilience**: Tests gateway behavior under prompt injection, denial-of-service traffic spikes, and network partitions.
- **Concurrency Race Audits (`graph_concurrency_race_audit.py`)**: Multi-threaded concurrency stress tests exercising simultaneous knowledge graph invalidations against continuous read-through traffic to verify zero stale reads.

## Running Tests

```bash
# Run all Rust cross-crate integration tests
cargo test --test '*'

# Run concurrency race condition audit script
python tests/graph_concurrency_race_audit.py
```
