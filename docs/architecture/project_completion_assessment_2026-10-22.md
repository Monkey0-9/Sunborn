# Sunborn Project Completion Assessment

**Assessment date:** 2026-10-22  
**Repository baseline:** `ed2e4135ffd8135d8780945730fde2744b4d6dc8`  
**Scope:** repository, checked-in evidence, current working tree, GitHub metadata, and Linear planning records.  
**Decision:** **RELEASE HOLD.** This assessment is not a certification, independent penetration test, or production approval.

## Executive assessment

Sunborn has a substantial prototype and research codebase, but the project is not complete against its own Phase 0–28 exit criteria. The repository contains source implementations, local tests, deployment templates, and frozen benchmark artifacts. Those do not prove multi-replica behavior, production control-plane protections, accepted SLOs, independent research reproduction, or operational readiness.

The assurance goal should be a bounded, evidence-backed system for a named workload and threat model. “Zero defects,” “zero risk,” “FIPS validated,” and universal safe reuse are not valid completion criteria. The product must abstain when correctness is uncertain, and the release record must show measured limits and residual risks.

## Current evidence and material findings

| Area | Evidence available in this checkout | Assessment |
|---|---|---|
| Git/GitHub | The review changes live in draft PR #1. At head `2005627666a4f7ee8a64178ab543e6ecb921f175`, Security workflow #64 passed; CI #68 failed in the research reproduction step after Rust, frontend, Helm, and API/SDK jobs passed. | The CI failure was a benchmark credential/application-scope mismatch (HTTP 401); a scoped test-credential fix is prepared, but must pass a new full run. No independent PR review is attached. |
| Linear plan | MON-458 described issue-level phase controls only for Phases 00–19 and Phases 20–28 as missing. MON-458 and MON-526 were marked Done despite open evidence criteria. | Both records are now In Progress with findings appended; missing phase controls remain blocked by the workspace issue limit. |
| Authorization and API contract | Router has 41 route declarations and 42 method/path operations. The checker now matches all 42 operations to OpenAPI and checks minimum response/security/request-body schema structure. The status schema documents its observation timestamp, single-node state, local provider, and disabled spend enforcement. | Runtime schema, authorization, and error behavior still require contract/policy tests; independent adversarial review remains open. |
| Credentials | PostgreSQL schema and startup hydration exist. Credential persistence/revocation methods have no runtime call sites; revocation does not invalidate already hydrated credentials on a live replica. | Durable credential lifecycle is incomplete. Design persistence, local eviction, cross-replica invalidation, and failure behavior before claiming G1 closure. |
| Deployment secrets | Static Kustomize included a placeholder Secret, and the frontend imported the gateway Secret. The checked-in changes remove these deployment paths. Helm now references only required gateway secret keys and defaults public HTTPS egress off. | Validate the rendered chart and actual External Secrets integration; cluster evidence is still required. |
| Release images | Release packaging sets chart `appVersion`, but values previously pinned images to `0.1.0`. The values now defer to the chart app version. | Verify packaged chart rendering for an actual release version. |
| Semantic benchmark | The checked-in 124-query artifact reports B2: 51 false hits (41.13% of queries), B3: 15 hits, 0 false hits, 12.09% modeled cost savings. The certificate is local and unauthenticated. | Finite-suite evidence only; rerun independently on the exact candidate and do not describe the certificate as a trusted signature. |
| Temporal benchmark | The 19-scenario artifact reports B3: 0 hits, 0 false hits, 0% recall; B2: 3 false hits (15.79%). | B3's 100% precision is vacuous here and demonstrates abstention, not useful temporal reuse. |
| Capacity | The checked-in run reports 1,327 requests, stage-one throughput 2,500.9 req/s, P99 10.339 ms, and RSS growth 86.68% with `envelope_stable: false`. | Existing `<5 ms` P99, `>10k req/s`, and bounded-memory targets are not met by this artifact. Diagnose, then repeat a preregistered sustained run. |
| Research provenance | The certificate identifies `LOCAL_DEVELOPMENT_INTEGRITY_CHECK` and `NONE_UNAUTHENTICATED_LOCAL_DIGEST`; benchmark artifacts identify commit `6bf5d40`, not this review base. | It verifies local file consistency for its recorded inputs, not independent authorship or the current candidate. |
| Release controls | Repository workflow declares a `release-approval` environment; actual required reviewers, tag rules, branch protections, required checks, registry ownership, and rollback drills were not verified. | Release governance cannot be declared effective from YAML alone. |

### Research claim corrections required

Use generated artifact values consistently: the static B2 comparator is **51/124 (41.13%)**, not 60/124 (48.39%). For the temporal suite, B2 is **3/19 (15.79%)**, not 63.16%. Report the B3 temporal result as **0 reuse hits and 0 recall** alongside 0 false hits. The capacity artifact is a failed SLO observation, not proof of stable capacity: **P99 10.339 ms** and **RSS +86.68%**.

## Weighted completion priorities

Weights allocate engineering and review effort; they do not allow a high score in one area to compensate for an open release blocker.

| Priority | Weight | Required outcome | Exit evidence |
|---|---:|---|---|
| P0 — Security, authorization, cryptography, tenant isolation | 25% | Close route and service authorization, tenant/app scope, admin paths, credential and key lifecycle, and cross-replica revocation. | Route/method policy inventory; automated negative tests; independent penetration review; revocation and key-rotation staging evidence. |
| P0 — Cache correctness, durable state, invalidation | 20% | Define authoritative versus derived state; prove exact/semantic/execution reuse compatibility, freshness fences, eviction, fail-through, and recovery. | Adversarial/property tests; delayed-event, partition, mutation, multi-tenant, restart/replay, restore, and RPO/RTO evidence. |
| P1 — Research quality, calibration, independent reproduction | 15% | Make claims useful, reproducible, statistically bounded, and limited to tested inputs; keep adaptive promotion shadow-only until approved. | Held-out and shift evaluation; exact/no-cache and naive semantic baselines; confidence intervals; actual billing data for savings claims; independent candidate-SHA reproduction and authenticated provenance or explicit unauthenticated scope. |
| P1 — Reliability, performance, failure recovery | 15% | Meet predeclared workload SLOs and resource envelopes, or revise requirements before confirmatory tests; recover safely under partial failures. | Preregistered load profile, raw traces, repeated runs, error rates, RSS envelope, p50/p95/p99, throughput, shutdown, provider/DB/cache/vector failure, restore and rollback drills. |
| P1 — Deployment, SRE, observability, disaster recovery | 10% | Make deployment fail closed, pin and attest artifacts, define SLO/RPO/RTO, alert on failure, and validate rollback and regional behavior. | Helm/Kustomize render checks; immutable artifact/provenance; external GitHub protections; dashboards/runbooks; staging upgrade, regional failover, failback, and recovery evidence. |
| P1 — API, SDK, frontend, developer experience | 10% | Reconcile contracts and prove supported client lifecycle, operator surfaces, accessibility, and truthful status reporting. | Runtime OpenAPI/schema/auth/error parity; clean SDK build/install, retries/timeouts/cancellation/resource cleanup; browser/BFF tests; accessibility and responsive review. |
| P2 — Governance, compliance, release operations | 5% | Keep roadmap traceable and release decisions reviewable; publish accurate security, data-handling, migration, support, and limitations documentation. | Phase/workstream owner and independent reviewer; evidence-linked gates; named human sign-off; release notes, escalation/runbooks, rollback plan, residual-risk record. |

## Ordered completion plan

1. **Freeze scope and truth (now):** declare the release hold; choose one authoritative 10-gate record; reconcile route/method counts, benchmarks, version labels, and Linear phases. Remove claims of production clearance, certification, or independent review unless evidence is attached.
2. **Close source-level security blockers:** enforce tenant and application scope at handlers; audit every route/method; connect credential create/expiry/revoke to durable storage and cache invalidation; remove placeholder credentials and gateway secrets from frontend workloads; make external egress opt-in and bounded.
3. **Prove persistence and correctness:** add database-backed lifecycle and multi-replica invalidation; test restart, replay, duplicate/delayed outbox events, vector-store drift, partition, and key rotation. Define which state is authoritative, replicated, reconstructible, ephemeral, or derived.
4. **Fix and remeasure performance:** explain the unstable RSS growth and 10.339 ms P99; agree a representative workload and limits before reruns; retain all raw failed and successful runs. Do not change thresholds after seeing confirmatory results without recording a new experiment.
5. **Harden release engineering:** verify reusable CI workflows and permissions; render charts for tag versions; validate required ExternalSecret keys; use image digests and SBOM/provenance; inspect branch protection, signed/protected tags, required reviewers, package ownership, and rollback controls in GitHub.
6. **Independent qualification:** have reviewers not involved in implementation perform threat-model review, penetration testing, clean-environment benchmark reproduction, and staging DR/upgrade/rollback drills. Record reviewer identities, exact commit, commands, artifacts, findings, and residual risks.
7. **Scoped release decision:** close each gate only with its stated evidence and accountable human sign-off. Start with a limited, reversible pilot only after applicable gates close; unrestricted production remains a separate decision.

## Release gates and accountable sign-off

Use the ten gates in [`release_acceptance_record.md`](release_acceptance_record.md) as the single release ledger. All are **OPEN** until evidence is attached to the exact release candidate and reviewed. This assessment does not close any gate. The current capacity artifact fails the stated performance and memory targets, credential revocation is not live-replica safe, and independent security, research, staging, and GitHub governance evidence is absent.

## Local verification for this change

- Rust formatting and strict Clippy passed. In the full workspace test run, unit tests passed, but two socket-based adversarial tests could not bind in the sandbox. The focused 18-test adversarial suite passed when rerun with local network permission; the full workspace suite was not rerun as one command with that permission.
- Frontend production build, TypeScript check, and all 24 frontend tests passed. The browser integration tests also required local network permission to start the Next.js test server.
- The historical checks listed above were performed on the prior audit branch state. The fresh update's verification is listed below; neither set closes operational gates.
- Research verifier negative tests passed, and the checked-in certificate verifier returned `INTEGRITY_VERIFIED_UNAUTHENTICATED` for the historical artifacts. Those artifacts remain tied to the older commit identified above.
- The Python SDK unit suite did not complete: it stalled at `test_async_sunborn_client` in this environment. Treat SDK CI as unresolved until that test is diagnosed and the full suite passes.
- Local Semgrep was unavailable, and no Semgrep MCP tool was exposed in this session. The security workflow changes have not yet been run by GitHub Actions for this branch.
- No PostgreSQL, multi-replica deployment, staging environment, production cluster, disaster-recovery drill, or external accessibility/penetration review was available here.

## Review boundaries

This assessment included repository source and documentation inspection, three scoped agent code-review passes, current connected GitHub repository metadata, and Linear control-record inspection. It did not represent 300–400 human reviewers, an external audit firm, production deployment, live staging verification, or a FIPS validation. No universal zero-risk guarantee is possible.

## Fresh completion review update — October 22, 2026

The following source-level corrections have been made on the release-readiness branch. They reduce misleading behavior and make unsupported production configuration fail closed; they do not close the corresponding operational release gates.

| Finding | Repository correction | Still required to close the gate |
|---|---|---|
| KMS provider names implied real remote cryptography | Local envelope test doubles now identify as such; the deterministic KEK fallback is removed; startup rejects unsupported provider names, and production startup rejects local-only crypto. Helm passes the actual `SUNBORN_KMS_PROVIDER` setting and defaults to local-only. Regional sketches no longer claim Vault/AWS KMS is configured. | Implement and exercise a real cloud KMS/HSM adapter with workload identity, authorization, rotation, remote audit evidence, and outage behavior. There is no real remote provider in this candidate. |
| Ceremony page fabricated a successful inference and attestation | The page is permanently labeled simulation-only; it generates no fake action or successful result. Failed, malformed, timed-out, or stale status shows unavailable; provider and cluster values are gateway-reported and unverified. Unsupported claims and fake VIP profiles were removed. | Add a genuine authenticated gateway demonstration with request ID, real timing, and verifiable provenance before advertising live behavior. |
| Multi-region files looked deployable and routed to one region | Manifests are marked design-only, use a non-pullable immutable-image placeholder and `.invalid` host, remove fake KMS ARNs, and document that the route has no failover. | Build real regional infrastructure and run measured primary-loss, integrity, isolation, failback, and recovery drills. No multi-region claim is cleared. |
| Spend controller was an in-memory helper presented as enforcement | Cluster status reports `not_enforced`; source documentation says the helper is not connected to billing requests, durable storage, or replicas. | Wire authenticated tenant and actual provider usage into a durable, distributed budget mechanism; test concurrency, restart, retries, clock changes, and provider billing reconciliation. |
| Rolling conformal recalibration was described as live | Source documentation now labels it experimental shadow research output; it is not used to authorize reuse. | Establish provenance and poisoning controls, preregister coverage criteria, reproduce held-out/shift tests, and keep any promotion shadow-only until approved. |
| Master verifier did not match CI Helm invocation | `scripts/verify.sh` now uses the existing-secret validation override, consistent eight-stage numbering, and non-certifying completion text. | Run the master verifier from a clean candidate checkout and record its result; the research reproduction script deletes existing benchmark outputs before running, so it was not executed. SDK and environment-dependent checks remain unresolved. |

### Fresh review verification

- `cargo fmt --check` passed. The new unsupported-KMS startup test and cluster-status integration test passed; all 19 `sunborn-identity` tests passed.
- Frontend production build passed. The full frontend suite passed: 29 tests, including stale/malformed/timeout status cases and the ceremony page's simulation-only disclosure.
- OpenAPI checker matched 42 method/path operations and validated minimum operation structure. Runtime schema/auth/error parity remains open.
- Helm lint and template rendering passed with the existing-secret validation override.
- `kubectl` is not installed, so Kustomize rendering was not verified. The full master verifier and research reproduction were not run because the reproduction script deletes existing benchmark results before regeneration. Preserve and independently regenerate those candidate-bound artifacts through a non-destructive workflow.
- No real cloud KMS, regional staging environment, live failover, independent review, or exact-candidate signed research reproduction was available.

The proposed effort weights for the remaining completion work are: security/authorization/cryptography/tenant isolation 25%; cache correctness/durable state/invalidation 20%; research/calibration/reproduction 15%; reliability/performance/recovery 15%; deployment/SRE/observability/DR 10%; API/SDK/frontend/developer experience 10%; governance/compliance/release operations 5%. These are planning weights, not completion scores. Preserve Phase 0–28, WS-00–WS-12, MON-458, and MON-526 as the control structure; map each item to an owner, independent reviewer, invariant, tests, evidence, residual risk, and rollback plan. No workstream or phase closes on code presence or green CI alone.

### CI finding pending verification — October 22, 2026

The full CI reproduction run on PR head `2005627666a4f7ee8a64178ab543e6ecb921f175` failed when the live benchmark received HTTP 401: it generated a unique application ID per case but sent one credential scoped to a different application. The gateway correctly rejected those requests. The reproducibility harness now prepares one non-admin credential per benchmark application and uses the matching case key, preserving per-case cache isolation. A unit test validates all 124 mappings. This correction has not yet been confirmed by a full GitHub reproduction run; keep research provenance gate G9 open.

**Current release decision remains RELEASE HOLD.** This review did not involve a 300–400 person independent team and does not claim world-ranking, zero defects, or zero risk. Independent security, research, and operational reviews remain release prerequisites.
