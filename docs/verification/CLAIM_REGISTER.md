# Sunborn Independent Engineering Verification: Claim Register

**Audit Stage:** Phase 2 Forensic Verification & Adversarial Auditing  
**Baseline Commit:** `568f9fdcb0f94143c220e51a056d8a25b4fe84d2` (`verification/phase-2-validation`)  
**Methodology:** Direct code inspection, adversarial probing, hardware profiling, and cryptographic provenance checks.

---

## Complete 18-Point Claim Verification Register

| Claim ID | Metric / Technical Claim | Claimed Value | Independent Verification Result | Final Classification | Evidence & Key Finding |
| :--- | :--- | :--- | :--- | :--- | :--- |
| **CLM-01** | Semantic False-Hit Rate | 0.0% false hits | **PARTIALLY VERIFIED (Corpus-Bounded)** | **PARTIALLY VERIFIED** | 0 FP on 124 curated queries; adversarial probe revealed 2/30 (6.7%) FP on scope negation. |
| **CLM-02** | Semantic Cache Precision | 100.0% precision | **PARTIALLY VERIFIED (Corpus-Bounded)** | **PARTIALLY VERIFIED** | 100% on static benchmark; drop to 0–33% on adversarial scope-transposition edge cases. |
| **CLM-03** | Temporal Mutation Recall | 33.33% recall | **VERIFIED** | **VERIFIED** | Exactly 2 true hits (`KM-002`, `PU-002`) out of 6 valid reuses in `semantic_cache_bench_t.py`. |
| **CLM-04** | Token Cost Reduction | 12.09% spend saved | **VERIFIED** | **VERIFIED** | Exactly 1,820 tokens avoided out of 15,050 total tokens in `benchmark_run.json`. |
| **CLM-05** | Conventional Vector False-Hit Rate | 41.13% false hits | **VERIFIED** | **VERIFIED** | Naive cosine ($\tau=0.82$) produced 51 false hits out of 124 queries (41.13%) in benchmark traces. |
| **CLM-06** | Peak Gateway Throughput | 2,500.9 req/s (or 12.4k claimed) | **MISLEADING / UNVERIFIED** | **PARTIALLY VERIFIED** | 2,500.9 req/s verified on single node. Claims of 12,450 req/s in ops docs are unverified projections. |
| **CLM-07** | Gateway Latency P99 | 10.339 ms (or 1.65ms claimed) | **MISLEADING / OVERSTATED** | **PARTIALLY VERIFIED** | Measured single-node soak was 10.339 ms. Claim of 1.65 ms P99 represents in-memory microbenchmarks. |
| **CLM-08** | Memory Footprint Ceiling | 256 MiB envelope | **VERIFIED** | **VERIFIED** | Hard bounded in `FinOpsGdsfCache` via `max_bytes = 268435456` and byte-accurate eviction. |
| **CLM-09** | Lineage Graph Scale | 100,000+ dependencies | **VERIFIED** | **VERIFIED** | Tested in `test_scale_100k_roaring_bitmap_simd_union` (100,000 bits populated and unified in 0.01s). |
| **CLM-10** | Hardware SIMD Acceleration | AVX2 / NEON Bitwise OR | **VERIFIED** | **VERIFIED** | Delegated to `roaring-rs` which compiles with AVX2 bitwise container unions on x86_64. |
| **CLM-11** | Zero-Allocation Hot Path | Zero heap allocation | **UNSUPPORTED / EXAGGERATED** | **MISLEADING** | Axum routing, JSON parsing, and String allocations occur on every request. Not zero-allocation. |
| **CLM-12** | Payload Envelope Encryption | AES-256-GCM | **VERIFIED** | **VERIFIED** | Implemented with `aes_gcm::AesGcm` using 96-bit nonces and 128-bit authentication tags. |
| **CLM-13** | Certificate Verification | HMAC-SHA256 Token | **VERIFIED** | **VERIFIED** | Constant-time verification using `subtle::ConstantTimeEq` in `crates/sunborn-core/src/certificate.rs`. |
| **CLM-14** | GDPR Cryptographic Shredding | $\mathcal{O}(1)$ Data Destruction | **VERIFIED** | **VERIFIED** | Memory zeroization via `zeroize::Zeroize` and PostgreSQL TDEK deletion confirmed in `sunborn-identity`. |
| **CLM-15** | Merkle DAG Spliced Proofs | Domain-Separated Proofs | **VERIFIED** | **VERIFIED** | Domain prefix (`0x00` leaf, `0x01` branch) prevents length extension; tested in `sunborn-execution`. |
| **CLM-16** | Distributed Invalidation | Vector Clock Fencing | **VERIFIED** | **VERIFIED** | Monotonic epoch lease checks and transactional outbox DLQ tested in `tests/outbox_replay_test.rs`. |
| **CLM-17** | Multi-Tenant Data Isolation | Strict Tenant Keyspace | **VERIFIED** | **VERIFIED** | Enforced at middleware, database (`tenant_id`), cache key prefix, and AEAD key isolation. |
| **CLM-18** | AWS KMS & Vault Readiness | Fail-Closed Production | **PARTIALLY VERIFIED** | **PARTIALLY VERIFIED** | Code supports AWS KMS and Vault; production startup strictly fails closed if local AEAD is used. |

---

## Detailed Audit Summaries

### 1. The 0% False-Hit Rate Claim (CLM-01 & CLM-02)
- **Claimed:** "0.0% false-hit rate, 100% precision."
- **Audit Reality:** Validated *only* on the 124-query static dataset and 19-scenario temporal benchmark. Under our 39-scenario adversarial suite, Sunborn achieved a **6.67% false-hit rate** (2/30 negative cases).
- **Root Cause of Failure:** 
  1. Scope negation ("Every engineer is not" vs "Not every engineer is").
  2. Syntactic order-blindness in bag-of-words token extraction.
- **Verdict:** True on evaluated benchmarks, but mathematically false as a universal guarantee.

### 2. The 10k+ req/s and Sub-5ms Latency Claims (CLM-06 & CLM-07)
- **Claimed:** "12,450 req/sec peak throughput, 1.65 ms P99 latency."
- **Audit Reality:** The real checked-in soak telemetry recorded **2,500.9 req/s** with a **10.339 ms P99 latency** on an 8-vCPU instance. The 12,450 req/s claim appears in documentation (`soak_and_capacity_benchmark.md`) as an unverified cloud projection.
- **Verdict:** Partially Verified (Single-node 2.5k verified; 12.4k is an unmeasured projection).

### 3. The "Zero-Allocation Hot Path" Claim (CLM-11)
- **Claimed:** "Zero-allocation hot paths."
- **Audit Reality:** While `CompressedLineageGraph` uses in-place bitwise OR unions across `RoaringTreemap` containers, the HTTP ingress pipeline performs multiple heap allocations per request (`String::clone`, `serde_json::Value`, `axum::body::Bytes`, `Vec<u8>`).
- **Verdict:** Misleading / Unsupported. It is low-allocation, not zero-allocation.
