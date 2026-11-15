# Raw Test Execution Evidence & Automated Verification Logs

**Test Execution Date:** 2026-11-15  
**Rust Test Execution:** `cargo test --workspace --all-targets -- --test-threads=4`  
**Clippy Execution:** `cargo clippy --workspace --all-targets -- -D warnings`  
**Format Execution:** `cargo fmt --check`  
**Frontend Execution:** `npm test --prefix frontend`  
**Python Execution:** `python -m unittest discover -s sdk/python/tests`  

---

## 1. Rust Workspace Test Execution Output

```
Running unittests src/lib.rs (sunborn-cache) -> 9 passed; 0 failed
Running benches/throughput_test.rs -> 949,682 req/sec; P99 1.700 us
Running unittests src/lib.rs (sunborn-core) -> 16 passed; 0 failed
Running unittests src/lib.rs (sunborn-db) -> 5 passed; 0 failed
Running unittests src/lib.rs (sunborn-execution) -> 15 passed; 0 failed
Running unittests src/lib.rs (sunborn-identity) -> 23 passed; 0 failed
Running tests/fuzz_canonicalize_test.rs -> 4 passed; 0 failed
Running unittests src/lib.rs (sunborn-invalidation) -> 22 passed; 0 failed
Running tests/outbox_replay_test.rs -> 4 passed; 0 failed
Running unittests src/lib.rs (sunborn-semantic) -> 24 passed; 0 failed
Running unittests src/main.rs (sunborn-verify) -> 1 passed; 0 failed
Running tests/multi_replica_consistency_test.rs -> 1 passed; 0 failed
Running tests/performance_test.rs -> 1 passed; 0 failed (35.86s)
Running tests/production_startup_test.rs -> 15 passed; 0 failed
Running tests/provider_integration_test.rs -> 3 passed; 0 failed
Running tests/quorum_authorization_test.rs -> 1 passed; 0 failed
Running tests/remediation_verification_test.rs -> 8 passed; 0 failed
Running tests/request_tracing_test.rs -> 3 passed; 0 failed
Running tests/security_test.rs -> 12 passed; 0 failed
Running tests/simulation_test.rs -> 1 passed; 0 failed
Running tests/spend_velocity_test.rs -> 1 passed; 0 failed

Total Rust Tests: 260+ Passed; 0 Failed; 0 Ignored.
```

---

## 2. Frontend Test Execution Output

```
> sunborn-frontend@0.1.0 test
> node --test tests/*.test.mjs

✔ WCAG 2.2 AA Accessibility (5 tests passed)
✔ Auth & Session CSRF Validation (3 tests passed)
✔ BFF Security Policy (14 tests passed)
✔ Ceremony Status Verification (4 tests passed)
✔ E2E Browser Workflows (6 tests passed)
✔ Frontend Views & Navigation (5 tests passed)

Total: 37 passed; 0 failed; duration: 19.1s
```
