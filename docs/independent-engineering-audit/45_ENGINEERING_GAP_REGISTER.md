# 45 — Engineering Gap Register

Gaps between claimed engineering maturity and demonstrated practice.

| ID | Gap | Current | Target | Evidence |
|---|---|---|---|---|
| EG-01 | Safety evidence measured | asserted | computed + tested | F-004 |
| EG-02 | Freshness authority | client-supplied | server-derived | F-005 |
| EG-03 | Default runtime | simulated | real or refuse-to-start | config/state |
| EG-04 | Durable invalidation | in-memory | transactional outbox | 27 |
| EG-05 | Distributed consistency | single-process | pub/sub + idempotency | 27 |
| EG-06 | Real retrieval | LSH hash | real embeddings + ANN | 20 |
| EG-07 | Risk guarantee | heuristic | conformal with stated coverage | 13 |
| EG-08 | Independent verification | symmetric MAC | asymmetric + CLI | 26 |
| EG-09 | Benchmark reproducibility | fabricated constants | criterion + artifacts | 29 |
| EG-10 | Frontend truthfulness | fabricated | real/empty states | 15/41 |
| EG-11 | Accessibility | violates AA | AA conformant | 38 |
| EG-12 | Supply chain | no signing | cosign + SLSA | 39 |
| EG-13 | CI enforcement | decorative scripts | real gates | 30 |
| EG-14 | Docs SSOT | 4 suites | 1 suite | 40 |
| EG-15 | Live-infra testing | none | testcontainers/compose | 28 |
| EG-16 | Key management | shared key | separated + rotation | 26 |
| EG-17 | Observability | process-local | OTel + dashboards | 30 |
| EG-18 | Capacity evidence | unproven | reproducible p50/p99 | 29 |

## Maturity by dimension (0–10)

| Dimension | Score |
|---|---|
| Architecture | 6.5 |
| Code quality | 7 |
| Correctness | 4.5 |
| Functionality | 5 |
| Maintainability | 6.5 |
| Security | 4 |
| Reliability | 4.5 |
| Performance | 3 |
| Scalability | 4 |
| AI/ML engineering | 4 |
| Cryptographic correctness | 6 |
| Mathematical correctness | 4 |
| Testing | 5 |
| Observability | 4 |
| Documentation | 3 |
| Accessibility | 3 |
| DevOps | 5 |
| Production readiness | 2.5 |
| Standards alignment | 3 |
| Scientific reproducibility | 3.5 |

**Mean ≈ 4.4/10.**
