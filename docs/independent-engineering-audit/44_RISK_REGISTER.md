# 44 — Risk Register

Likelihood (L) × Impact (I) = score (1–5 each). Owner area in parentheses.

| ID | Risk | L | I | Score | Mitigation |
|---|---|---|---|---|---|
| R-01 | False cache hit serves wrong/contradictory answer | 3 | 5 | 15 | Measure evidence; fix F-004; real embeddings |
| R-02 | Stale reuse due to forged/cclient freshness | 3 | 5 | 15 | Server-derived epochs; ingest path (F-005) |
| R-03 | Fake backend/credentials mislead operator | 4 | 4 | 16 | Remove simulator default; fail closed (F-002/003) |
| R-04 | Dev auth bypass reaches a shared environment | 2 | 5 | 10 | Remove bypass (F-001) |
| R-05 | Silent fallback hides outage, caches nothing | 5 | 4 | 20 | Fail closed or explicit degraded mode (F-010) |
| R-06 | Qdrant deletes fail → stale vectors persist | 4 | 4 | 16 | Fix id mapping (F-006) |
| R-07 | Public false performance claims → reputational/legal | 4 | 4 | 16 | Claims reconciliation (F-007) |
| R-08 | Invalidation lost on restart → stale cache | 4 | 4 | 16 | Wire durable outbox |
| R-09 | Shared HMAC key compromise | 2 | 5 | 10 | Key separation + rotation (F-015) |
| R-10 | KMS latency starves runtime | 2 | 3 | 6 | Async KMS trait |
| R-11 | `KEYS` scan blocks Dragonfly under load | 3 | 3 | 9 | Use `SCAN` |
| R-12 | No rate limits → abuse/cost blowup | 3 | 3 | 9 | Add limits |
| R-13 | Supply-chain compromise (suppressed advisories) | 3 | 4 | 12 | Remove suppressions; scanning; signing |
| R-14 | WCAG non-conformance blocks enterprise procurement | 4 | 3 | 12 | Fix a11y |
| R-15 | Multi-region manifests applied → outages | 2 | 4 | 8 | Mark design-only; block apply |
| R-16 | Non-deterministic SBOM → audit failure | 3 | 2 | 6 | Deterministic generator |
| R-17 | Tests give false confidence (mock-only E2E) | 4 | 3 | 12 | Live-infra tests |
| R-18 | Duplicate SDK implementations drift | 3 | 2 | 6 | Consolidate |

**Top risks:** R-05 (silent degradation), R-03 (fake surface), R-01/R-02 (safety), R-06 (stale vectors), R-08 (lost invalidation).
