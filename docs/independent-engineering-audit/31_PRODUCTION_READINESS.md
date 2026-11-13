# 31 — Production Readiness

**Verdict: NO-GO.**

---

## 1. Readiness gates

| Gate | Status | Evidence |
|---|---|---|
| G1 Default runtime uses real backends | FAIL | mock LLM, LSH embeddings, memory stores (`config.rs:44-50`, `state.rs:298,114`) |
| G2 Safety guarantee measured, not asserted | FAIL | self-attested evidence (F-004) |
| G3 Freshness server-derived | FAIL | client-supplied epochs (F-005) |
| G4 Durable invalidation survives restart | FAIL | in-memory outbox; Postgres outbox dead |
| G5 Distributed consistency | FAIL | cluster sync unwired |
| G6 Real benchmarks reproducible | FAIL | fabricated constants; missing cited file |
| G7 Security posture | FAIL | dev auth bypass, fake credentials, no CSP |
| G8 Frontend truthfulness | FAIL | fake backend default, hardcoded metrics |
| G9 Docs single source of truth | FAIL | four contradictory suites |
| G10 Accessibility conformance | FAIL | WCAG violations |
| G11 Supply chain integrity | FAIL | ignored advisories, no signing |
| G12 CI enforces claims | FAIL | branch-protection names fictional |
| G13 Tests exercise real infrastructure | FAIL | no live-infra tests |
| G14 Graceful shutdown | PASS | real, bounded (`main.rs`) |
| G15 Auth/RBAC in production mode | PASS | real (`auth.rs`) |
| G16 Envelope encryption/shredding | PASS | real (`crypto_shred.rs`) |
| G17 FinOps reservations | PASS | real (`finops.rs`) |

**PASS: 4 / 17. NO-GO.**

## 2. What must be true before any production claim

1. Default is production-real or the service refuses to start in production with mock defaults (partially done: readiness fails closed on mock origin, but not on memory/LSH).
2. Certificate evidence computed from measurements.
3. Server-derived freshness.
4. Durable, idempotent invalidation.
5. Real benchmarks with artifacts.
6. Frontend fake paths removed.
7. CI enforces the claims and branch protection matches real jobs.
8. Accessibility fixes.

## 3. Operational readiness

- Runbooks exist but are unverified against a running stack.
- No SLO dashboards, no alerting, no on-call evidence.
- Multi-region is design-only.
- No backup/restore automation.

## 4. Classification

Sunborn is a **functional prototype / early-stage product**. It is not an MVP for regulated production, not a pilot-ready enterprise system, and not a production-capable cache gateway.
