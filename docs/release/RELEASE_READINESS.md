# Release Readiness

**Supersedes all earlier release records.** The single authoritative assessment is
[`docs/independent-engineering-audit/48_FINAL_ENGINEERING_VERDICT.md`](../independent-engineering-audit/48_FINAL_ENGINEERING_VERDICT.md).

---

## Decision

> ## NO-GO for production.
> Sunborn is a **functional prototype / early-stage product**. A controlled,
> single-tenant sandbox may be run for evaluation, but no production or
> multi-tenant pilot is approved.

This replaces the previous "CONDITIONAL PRODUCTION PILOT CLEARED" language, which
overstated readiness.

## Evidence snapshot (2026-11-13)

- Tests executed: **315 passing, 0 failing** (252 Rust, 37 frontend, 26 Python).
- Default runtime: mock LLM origin, LSH embeddings, in-memory DB/cache/vector.
- Attestation evidence: **asserted, not measured** (P0).
- Freshness: **client-supplied** epochs (P0).
- Durable outbox / cluster sync / Neo4j / write-behind: **implemented but unwired**.
- Capacity artifact: **2,500.9 req/s at P99 10.339 ms** — below the project's own target.
- Frontend fake backend and dev auth bypass: **removed in this cleanup**, but the
  surrounding data layer still needs real wiring (P4).
- Supply chain: advisories suppressed; no image signing/SLSA.

## Blocking items

See [`46_REMEDIATION_ROADMAP.md`](../independent-engineering-audit/46_REMEDIATION_ROADMAP.md):
all **P0** and **P1** tasks must close, and the gates in
[`31_PRODUCTION_READINESS.md`](../independent-engineering-audit/31_PRODUCTION_READINESS.md)
must pass with recorded transcripts.

## Sandbox guardrails (evaluation only)

1. Single region; no multi-region routing.
2. Ingress capped at ≤ 1,000 req/s per node.
3. Real upstream LLM and embeddings configured explicitly.
4. PostgreSQL strict durability enabled; do not run in silent-fallback mode.
5. Admin keys provisioned via secret manager; never defaults.
6. Treat cache as an accelerator; the upstream provider and database remain
   authoritative.
