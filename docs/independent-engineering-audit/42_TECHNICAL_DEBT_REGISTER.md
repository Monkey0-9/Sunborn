# 42 — Technical Debt Register

| ID | Debt | Area | Severity | Evidence |
|---|---|---|---|---|
| TD-01 | Four contradictory doc suites; no SSOT | Docs | P1 | 40 |
| TD-02 | Non-deterministic SBOM; generated artifacts tracked | Build | P2 | 39 |
| TD-03 | Broken root `openapi.yaml` pointer | API/build | P1 | 17 |
| TD-04 | `.gitignore` incomplete; caches/junk unignored | Repo | P2 | 05 |
| TD-05 | Silent fallback to memory/LSH on backend errors | Core | P1 | 25 F-010 |
| TD-06 | Durable outbox / write-behind / cluster sync / Neo4j unwired | Distributed | P1 | 27 |
| TD-07 | Shared HMAC key role | Crypto | P1 | 26 |
| TD-08 | No live-infra integration tests | Testing | P1 | 28 |
| TD-09 | Cosmetics scripts (`audit_licenses`, `ci_load_test`) presented as gates | CI | P2 | 30 |
| TD-10 | Branch-protection names match no CI job | CI | P2 | 30 |
| TD-11 | Frontend dead code (aurora, toast, 42% API methods) | Frontend | P3 | 15 |
| TD-12 | Duplicate SDK implementation (`sunborn_agent.py`) | SDK | P2 | 23 |
| TD-13 | `KEYS` O(N) scans | Cache | P2 | 16 |
| TD-14 | Lineage dropped after invalidation | Invalidation | P1 | 20 F-017 |
| TD-15 | Process-local metrics presented as Prometheus | Observability | P2 | 16 |
| TD-16 | Fake NFC table; dual canonicalizers | Identity | P2 | 11 |
| TD-17 | `run_blocking_async` runtime spin | Concurrency | P2 | 16 |
| TD-18 | No general rate limiting / body cap | Security | P1 | 25 |
| TD-19 | No CSP | Frontend | P2 | 38 |
| TD-20 | Accessibility source-grep tests | Testing | P2 | 38 |
| TD-21 | Advisories suppressed in `audit.toml` | Supply chain | P1 | 39 |
| TD-22 | No concurrency ids/idempotency on mutations | API | P2 | 17 |
| TD-23 | Multi-region manifests not runnable | Deploy | P3 | 30 |
| TD-24 | Frontend screenshots committed/intended | Repo | P3 | 05 |
| TD-25 | No cloud KMS fallback policy documented | Crypto | P2 | 26 |

**Total:** 25 tracked debt items. P1: 10, P2: 11, P3: 4.
