# 48 — Final Engineering Verdict

**Classification: FUNCTIONAL PROTOTYPE / EARLY-STAGE PRODUCT. Production: NO-GO.**

---

## Methodology for percentages

Denominator = **45 enumerated features** in `09_FEATURE_VERIFICATION.md`. A feature is "verified working" only if the runtime path executes it against a real backend and a test or reproducible artifact supports it. "Working with limitations" = executes but with a material caveat (non-default backend, naming mismatch, or known defect). "Simulated/mocked/unwired/dead" = not genuinely exercised. "Not implemented / design only" = absent. Exclusions: purely internal helpers (tracked in `inventories/functions.csv`) are not counted.

---

## 1. What is the actual condition of Sunborn?

A well-structured Rust gateway (real auth, FinOps, crypto, persistence) wrapped in a safety/performance narrative that the code does not yet support. The default runtime is simulated, the attestation is self-referential, several named techniques are absent, and the public docs contradict each other. It is a credible research prototype, not a product.

## 2–5. Feature percentages

| Bucket | Count | % |
|---|---:|---:|
| Verified working | 18 | **40%** |
| Working with limitations | 14 | **31%** |
| Simulated / mocked / unwired / dead | 8 | **18%** |
| Not implemented / design only | 4 | **9%** |
| Failing | 1 | **2%** |
| **Total** | **45** | 100% |

## 6. What is mocked or simulated?

- Default origin LLM (`MockOriginProvider`).
- Default embeddings (LSH hash).
- Default DB/cache/vector (in-memory).
- Frontend backend (`standalone-simulator.ts`, 698 LOC).
- Frontend credentials/session/proofs on failure.
- Benchmarks (`edges*1.8`, unconditional PASS prints, `is_valid:true`).

## 7. What is misleadingly represented?

- "571,428 req/s / 3,257 req/s / P99 475 µs".
- "0.0% false hits" (vacuous; reuse 0).
- "2.15 MB / 1.8 B per edge".
- "HNSW / W-TinyLFU / AST / cross-encoder NLI / PostgreSQL WAL".
- "Durable transactional outbox / distributed causal consistency".
- "Dual-admin cryptographic sign-offs".
- "WCAG AAA".
- "0 vulnerabilities" while advisories are suppressed.

## 8. What functions are broken?

- `QdrantVectorStore::delete` (id mismatch) — F-006.
- `CompressedLineageGraph::invalidate_document` (drops lineage) — F-017.
- Certificate issuance/verification evidence path — F-004.
- `splice_dag` latency estimate (cosmetic constant) — not a correctness break but a false metric.
- Frontend purge/circuit-breaker handlers (state changes on failure).

## 9. What algorithms are incorrect or inefficient?

- Incorrect as named: HNSW (absent), W-TinyLFU (GDSF), AST (token heuristics), cross-encoder NLI (table), WAL (OLTP).
- Inefficient: memory vector search O(n·d); `KEYS` O(N) prefix scans; `run_blocking_async` spin.

## 10. What mathematical models are unsupported?

- Conformal α≤0.001 online guarantee (seeded thresholds; rigorous calibrator unwired).
- Precision 100% from empty denominator.
- Memory/throughput constants.

## 11. What integrations are disconnected?

Neo4j store; distributed cluster sync; durable Postgres outbox; write-behind flusher; cross-encoder backend; rigorous conformal calibrator; micro-batch embedder; ingest-driven epoch bumps.

## 12. Biggest security weaknesses

Dev auth bypass + plaintext secret session; fake credential minting; self-attested proof; client-controlled freshness; silent fallbacks; no CSP; no general rate limits; suppressed advisories; no signing/SLSA.

## 13. Are cryptographic guarantees justified?

Primitives (AES-256-GCM, HMAC, Merkle, zeroize, CSPRNG): **yes**. Attestation guarantees: **no** — symmetric key, shared key role, unmeasured evidence, no independent verifier.

## 14. Are AI/ML claims supported?

No. Default embeddings are non-semantic, no ANN, no real-model evaluation; all semantic numbers use hash vectors.

## 15. Are benchmarks credible?

No. Fabricated constants, a nonexistent cited file, contradictory tables, vacuous precision, and no signed reproducible artifact.

## 16. Biggest architectural weaknesses

Simulated defaults; unwired durable/distributed components; proof layer separated from measurement; four doc suites.

## 17. Biggest scalability problems

No ANN client; O(N) `KEYS`; in-memory lineage/outbox; process-local metrics; no distributed coordination.

## 18. Actual engineering maturity

**~4.4/10.** Strong gateway and crypto primitives; weak safety verification, benchmarks, frontend truthfulness, and production wiring.

## 19. Standards applicable

IEEE 1012/29119/29148/42010, ISO/IEC 25010/27001/27005/27701, NIST CSF/800-218/800-57/800-90/AI RMF, OWASP Top 10/API/ASVS, SLSA/SPDX/CycloneDX, WCAG 2.2, RFC 6962/8785. None fully satisfied.

## 20. Requirements not satisfied

See `08_REQUIREMENTS_TRACEABILITY.md`: R-05/06/07/08/09/12/18/20/21/22/24/28/29/30/32/33/34 are partial/not met.

## 21. What needs complete rebuilding?

- The proof layer (measurement-first attestation).
- Freshness/epoch authority (ingestion-driven).
- The benchmark/evaluation harness.
- The frontend data layer (no simulation in default path).
- Documentation (collapse to one SSOT).

## 22. What must be completed before production?

All P0 and P1 tasks in `46_REMEDIATION_ROADMAP.md`, plus the four passing infrastructure gates, live-infra tests, real benchmarks, and an honest release record.

## 23. Exact next engineering roadmap

`P0 (7 tasks) → P1 (15 tasks) → P2 → P3 → P4` per `46`. First three concrete actions: land the claims rewrite, remove the frontend fake backend, make certificate evidence measured.

---

## Final classification

| Candidate | Evidence |
|---|---|
| Experimental prototype | Too complete — real gateway, 315 tests |
| **Functional prototype** | **Matches: real core, simulated defaults, unverified guarantees** |
| Early-stage product | Partial — missing durability/hardening |
| Functionally complete app | No |
| Production-capable | No |
| Production-ready | No |

**Verdict: FUNCTIONAL PROTOTYPE / EARLY-STAGE PRODUCT. NO-GO for production.**

> **Post-audit progress (2026-11-13):** all seven originally-listed P0s are now
> addressed — the frontend fake backend, dev auth bypass, fabricated credentials,
> Qdrant delete mismatch, false public claims, broken root OpenAPI pointer, and
> **measured certificate evidence** (reference fingerprint bound into the signed
> certificate and re-verified against the candidate prompt). Freshness is now
> **server-derived** from the epoch registry, and Neo4j transitive lineage is
> wired when configured. The verdict remains NO-GO because the durable outbox,
> distributed cluster sync, write-behind, real embeddings/ANN/NLI, live-infra
> tests, and supply-chain signing are still open. See
> `49_REMEDIATION_VERIFICATION.md`.

The path to a genuinely world-class project is not more adjectives — it is making the safety layer measure what it claims, wiring the durable/distributed components, publishing reproducible benchmarks, and telling the truth. See `46` P4 (Daredevil upgrade).
