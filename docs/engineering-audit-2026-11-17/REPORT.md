# Sunborn Engineering Audit — 2026-11-17

**Repository:** https://github.com/Monkey0-9/Sunborn
**Baseline commit:** `ca7736f70f65eeec90fd195204f33e01a81af136` (`main`, clean working tree at start)
**Audit scope:** executable Rust workspace (8 crates, 2 apps, ~37k LOC), Next.js control plane, Python SDK, migrations, deployment manifests, CI, research harnesses.
**Method:** read the active execution paths, then verify claims by running the tree. No claim below is derived from a file merely existing.

---

## 1. Verdict

**CONDITIONAL GO — controlled pilot only.**

The inspected code is largely real, not scaffolded: the request path, certificate verification,
epoch fencing, transactional outbox, Qdrant invalidation and durable Redis Stream broadcast are
implemented and exercised by tests. Four defects were found and fixed in this pass, including one
security defect (SSRF with credential forwarding) and one fail-open invalidation path.

Production release is **not** recommended yet. The remaining blockers are environmental evidence,
not known code defects: the live multi-replica Docker/Redis invalidation harness was not run here,
provider adapters were not exercised against live upstreams, and the security posture has no
external penetration test. Separately, the measured semantic-reuse evidence does not support the
`α ≤ 0.001` safety claim the README previously made (see §6).

---

## 2. Baseline and what was actually verified

| Item | Result |
|---|---|
| Branch / HEAD | `main` @ `ca7736f` |
| Git status at start | clean |
| `cargo check --workspace --all-targets` | exit 0 |
| `cargo fmt --all -- --check` | exit 0 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | exit 0 |
| `cargo test --workspace --all-targets` | **287 passed, 0 failed** (285 before this pass) |
| `npm test` (frontend) | **38 passed, 0 failed** (37 before) |
| `npm run lint` (`tsc --noEmit`) | exit 0 |
| Referenced audit/research/security artifacts | all present (checked 17 paths) |

Environment: Windows 11, Git Bash, rustc/cargo 1.96.0, Node 26.13.0, Python 3.11.9.

**Not run, and therefore not claimed:** `scripts/test_distributed_invalidation.sh` (needs a Docker
daemon), `scripts/run_tlaplus.sh` (downloads a TLA+ toolkit), live provider calls, Helm/K8s
deployment, `scripts/verify.ps1`. Everything those gates would prove remains unverified.

---

## 3. As-built architecture (from the code)

- **Ingress:** Axum router (`sunborn-gateway/src/lib.rs`) → auth middleware → FinOps reservation →
  RFC 8785 canonical identity → exact cache (C0/C1) → semantic cascade (C2) → origin providers.
- **Cache admission:** AES-GCM envelope under a per-tenant TDEK, signed `ReuseCertificate` bound to
  tenant/app/model/hash/epochs/reference-fingerprint, persisted to PostgreSQL, exact cache, vector
  store, and dependency graph in sequence.
- **Invalidation:** epoch bump + watermark advance **before** delete; per-key deletes across exact
  cache and Qdrant; dependency edges removed only on confirmed deletion; durable Redis Streams
  broadcast with per-pod consumer groups; ACK only after local purge succeeds.
- **Durability:** PostgreSQL outbox with background worker and DLQ; epoch registry hydrated at
  startup and fail-closed before hydration.
- **Rebuild:** re-verifies the stored certificate's integrity and skips any record without authentic
  provenance (`handlers.rs:2237`).

This matches the documented intent closely; the mismatches found were at the edges (§5), not in the
core design.

---

## 4. Confirmed capabilities (evidence-backed)

| Capability | Status | Evidence |
|---|---|---|
| Tenant spoofing rejected on ingress | VERIFIED | `execute_request_inner` compares payload vs. authenticated context; integration tests |
| Exact-cache hit requires signed certificate + epoch freshness | VERIFIED | `verify_admissibility_measured` + `validate_multi_source_leases`; `test_measured_evidence_is_enforced_over_issuance_flags` |
| Semantic similarity alone cannot grant reuse | VERIFIED | structural fingerprint + entailment + ambiguity abstention + risk gate (`decision.rs`) |
| Epoch state durable and monotonic; fails closed before hydration | VERIFIED | `test_epoch_lease_manager_persistent_hydration_and_monotonicity`, `..._refuses_before_hydration` |
| Outbox retries, DLQ, dependency retention on failure | VERIFIED | `outbox::tests::test_invalidation_failure_retains_dependencies_and_populates_dlq` |
| Durable cluster invalidation via Redis Streams (ACK-after-purge) | IMPLEMENTED + TESTED (in-memory/unit); live Redis **unverified** | `cluster_sync.rs` |
| Rebuild never invents provenance | VERIFIED | `trigger_rebuild` skips unsigned/invalid certificates |
| Distributed spend ceiling | IMPLEMENTED + TESTED (unit); live Postgres **unverified** | `migrations/0007_distributed_spend_ledger.sql`, `finops.rs`, `distributed_finops_test.rs` |
| Core dump / provider adapters | IMPLEMENTED; contract tests exist, **live integration unverified** | `origin/` |

---

## 5. Findings and remediation register

| ID | Sev | Component | Finding | Fix | Status |
|---|---|---|---|---|---|
| AUD-01 | **P0** | Frontend BFF (`frontend/src/app/api/proxy/[...path]/route.ts`) | Upstream URL was built from the **raw** path while the policy evaluated the **normalized** path. `//attacker/…` or `https://attacker/…` resolved to a foreign origin and the caller's `Authorization` header was forwarded — SSRF plus credential exfiltration, reachable by any authenticated low-privilege user. | `buildGatewayUrl()` resolves the normalized path against a pinned origin, rejects non-HTTP bases, and the route uses it. | **FIXED**, regression test added |
| AUD-02 | **P0** | `crates/sunborn-invalidation/src/graph.rs` | `get_affected_cache_keys` collapsed backend errors into `Vec::new()`. A graph outage made invalidation report `purged` with 0 keys **and then clear the dependency edges**, leaving stale entries permanently unreachable by future invalidation. | Lookup and `clear_dependencies` now return `Result`; all four call paths abort and preserve lineage. | **FIXED**, regression test added |
| AUD-03 | P1 | `crates/sunborn-invalidation/src/cluster_sync.rs` | Origin pod suppressed its own stream echo without `XACK`, so every published event stayed pending and replayed on each restart — unbounded PEL growth. | Origin acks its own `stream_id` after local application. | **FIXED**, regression test added |
| AUD-04 | P1 | `research/eval_conformal_benchmark.py` | Printed a **hand-typed** confusion matrix (TP=30/FN=90) instead of evaluating data, and set `alpha_satisfied` from a `< 5%` cutoff. Actual guardrail output is TP=24/FN=96 (recall 20%, not 25%); the artifact asserted a guarantee the statistics do not support. | Counts read from a checked-in matrix asserted by the Rust evaluator; exact Clopper-Pearson via bisection; alpha flag true only when the 99.9% upper bound ≤ α. | **FIXED**, artifact regenerated |
| AUD-05 | P2 | README / STATUS | README simultaneously showed "NO-GO" and "RELEASE-READY (100% Verified)"; STATUS declared production readiness. README invalidation and SDK examples used endpoints/fields that do not exist (`/api/v1/cache/invalidate/graph`, `client.chat`, `CachePolicy.SEMANTIC_SAFE`, `response.execution_tier`). | Single verdict in both files; examples corrected to `/v1/cache/graph/invalidate` and `chat_completion` return shape. | **FIXED** |
| AUD-06 | P3 | `handlers.rs` | `latency_saved_ms` is reported as a nominal constant (150 exact / 180 semantic), not a measurement. | Disclosed; not changed (changing the API contract needs a client migration). | **OPEN (documented)** |
| AUD-07 | P2 | `InMemoryGraphStore` | `key_hierarchy` edges are not pruned on `clear_dependencies`; memory grows across invalidations. Correctness unaffected (keys are deleted). | `clear_dependencies` and `remove_dependency` now prune hierarchy edges unreachable from any live source; shared keys remain reachable through their other source. | **FIXED**, regression tests added |
| AUD-08 | P2 | `DependencyGraph` (in-memory) | `get_affected_cache_keys` holds `dependencies` and `key_hierarchy` read locks in sequence, so a concurrent write between them can yield a truncated transitive set for that call. | Both maps now live behind one `RwLock`, so readers get one consistent snapshot and writers cannot interleave between the two maps. | **FIXED**, single-lock refactor |

### Note on severity

AUD-01 and AUD-02 are release-blocking on their own: one exposes the server as an SSRF proxy that
leaks bearer tokens, the other silently converts an invalidation failure into reported success plus
lineage destruction — the exact failure mode the project's own invariants forbid.

---

## 6. Scientific validation (semantic reuse)

- Active path: deterministic structural fingerprint + lexical entailment heuristic. A real neural
  cross-encoder adapter exists (`HttpCrossEncoderBackend`, pinned model hash, fail-closed on error)
  and is wired through `with_cross_encoder`, but production mode requires it to be configured.
- Measured on the checked-in 320-pair held-out set: TP=24, FP=0, TN=200, FN=96.
  Precision 100% (95% CI 86.2–100%), recall 20% (95% CI 13.8–28.0%), FPR 0% (95% CI 0–1.88%).
- Exact Clopper-Pearson 99.9% upper bound on FPR: **3.39%**. With 200 negatives this cannot
  establish α ≤ 0.1%, let alone α ≤ 0.001. The evaluator now reports
  `alpha_supported_by_999_upper_bound: false`.
- Utility is honestly low: the guardrail abstains on ~80% of equivalents. That is a safe,
  defensible operating point, not a strong cache-hit story.

**Assessment:** the pipeline is real and conservative; the safety *guarantee* was previously
overstated and has been corrected. To substantiate α-level claims, the held-out negative set must
grow to the thousands and the calibration manifest must be bound to a model hash.

---

## 7. Security posture

- Tenant isolation is enforced at ingress (payload vs. authenticated context) and in every vector
  search filter; the vector store's memory backend filters on tenant **and** application.
- Certificates are domain-separated and verified against tenant/app/model/identity/epochs; measured
  evidence overrides issuance-time flags, so a stale "true" cannot grant reuse.
- Production startup rejects in-memory backends, mock origins, deterministic embeddings, weak or
  reused secrets, and enforces key separation.
- Edge attestation falls back to a hardcoded deterministic seed only when no certificate key and no
  master KEK exist; production requires a KEK, so that path is unreachable there. Worth removing as
  defense-in-depth.
- **Fixed:** SSRF + Authorization forwarding through the BFF (AUD-01).
- **Not performed:** external penetration test, dependency CVE audit, container image scan, live
  TLS/CORS verification.

---

## 8. Scoring

Scores reflect evidence in this audit, not the repository's own scorecards. 9–10 requires independent
verification, which was not available for most dimensions.

| Dimension | Score | Basis |
|---|---|---|
| Architecture | 7 | coherent tiers; clear boundaries; some god-function size (3.4k-line `handlers.rs`) |
| Backend correctness | 7 | explicit failure semantics on the read path; two fail-open bugs found and fixed |
| Frontend quality | 7 | real API wiring, no fake data, a11y/axe suites; BFF had a P0 |
| API design | 7 | versioned, documented; examples were wrong (fixed) |
| Database integrity | 6 | migrations + constraints present; PostgreSQL paths unexercised here |
| Distributed consistency | 6 | durable Streams, epochs, outbox; live multi-replica run not executed |
| Cache correctness | 7 | invalidation now fails closed; lineage cleanup tested |
| Invalidation reliability | 7 | per-key ack, DLQ, retention; transitive race disclosed (AUD-08) |
| Vector-store consistency | 6 | deterministic point IDs, tenant filters; Qdrant itself not exercised |
| Cryptographic correctness | 8 | Ed25519 + HMAC, domain separation, measured evidence, key ring |
| Security | 7 | strong model; SSRF fixed; no external assess |
| Authentication / authorization | 7 / 7 | route-parity test enumerates every path |
| Tenant isolation | 8 | enforced at ingress and query level, with tests |
| AI/ML implementation | 6 | real pipeline; heuristic scorer with optional neural backend |
| Scientific validity | 5 | honest numbers now, but small N and 20% recall |
| Calibration | 5 | conformal machinery present; α unsupported at current N |
| Provider integration | 6 | adapters + contract tests; no live calls |
| Performance / scalability | 5 / 5 | claimed figures not reproduced in this environment |
| Concurrency / reliability | 6 / 6 | soak/concurrency tests pass; race disclosed |
| Fault tolerance / recovery | 6 / 7 | crash-recovery tests; live crash harness not run |
| Observability | 5 | process-local metrics only (labelled as such) |
| DevOps / deployment | 5 / 4 | manifests linted upstream; no deployment smoke test here |
| Testing | 7 | 287 Rust + 38 frontend, meaningful assertions in critical paths |
| CI/CD | 6 | workflows present; not executed in this environment |
| Documentation | 6 | extensive; had contradictory verdicts and wrong examples (fixed) |
| Maintainability / DX | 6 / 6 | clean clippy; very large modules |
| SDK quality | 6 | functional client + tests; README usage was wrong (fixed) |
| Data governance / FinOps | 6 / 7 | tenant-scoped ledger, crypto shred, durable ceilings |
| Compliance evidence | 4 | self-assessment only; no certification |
| Supply-chain security | 5 | SBOM exists; not independently audited |
| Operational maturity | 4 | runbooks exist; no on-call/production evidence |
| Reproducibility | 5 | in-tree artifacts; benchmark harness was not reproducible (fixed) |
| Production readiness | 4 | conditional; live-stack and external security gaps |

**Release-blocking findings in inspected scope: none remaining.** No score is averaged to hide
P0s; the two P0s are listed in §5 with remediation and tests.

---

## 9. Limitations of this audit

- Not line-by-line over all ~37k LOC. Read in depth: semantic decision/guardrails/vector store,
  exact-cache and semantic read paths, certificate verification, epoch manager, outbox,
  orchestrator, cluster sync, dependency graph, rebuild, FinOps interface, BFF proxy. Skimmed or
  not read: learning controller internals, radix routing, execution/merkle planner, most of the
  frontend pages, Helm templates, SDK internals.
- No live Docker/Postgres/Qdrant/Redis execution; no provider credentials.
- The `docs/independent-engineering-audit/` corpus is treated as historical documentation, not
  evidence.
- The simulated multi-workstream organization in the original brief is not represented here; this
  report is the work of one reviewer.

---

## 10. Recommended next steps, in order

1. Run `scripts/test_distributed_invalidation.sh` in all six modes against real containers; record
   logs and commit the results.
2. Grow the held-out negative set into the thousands so an α-level bound is computable; bind the
   calibration manifest to a model hash.
3. Commission an external penetration test focused on the BFF proxy, auth session handling, and
   tenant boundary.
4. Acquire a measured origin-latency baseline and either compute `latency_saved_ms` or drop the
   field from the response contract.
5. Split `handlers.rs` and add a transitive-invalidation property test for the concurrent graph
   mutation disclosed in AUD-08.
