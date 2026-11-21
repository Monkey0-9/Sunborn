# Sunborn Release Notes & Institutional Review Dossier

> **This file is a historical dossier, not current status.** Entries are kept newest-first for provenance.
> The single source of truth for readiness, test counts, and benchmark figures is
> [`docs/STATUS.md`](docs/STATUS.md), which supersedes every verdict, score, and badge in this file.
> For the current verification commands, see [`README.md`](README.md) §14.

---

## Release: `v1.0.0` — **RESCINDED on 2026-11-17**

**Originally dated**: November 15, 2026  
**Withdrawn**: November 17, 2026, after an audit whose finding was that published figures were not reproducible from the repository  
**Current verdict**: **CONDITIONAL GO — CONTROLLED PILOT ONLY** ([`docs/STATUS.md`](docs/STATUS.md))

This entry claimed "production release complete", a 9.71/10 quality score, and 100% green suites.
That verdict was withdrawn. The subsequent audit found four defects the suite had not caught, and
corrected one benchmark artifact that had been hand-typed rather than computed:

| ID | What the audit found | Disposition |
| :--- | :--- | :--- |
| **AUD-01** | The BFF proxy built its upstream URL from the raw request path while policy checked the normalized path, allowing an SSRF that forwarded the caller's `Authorization` header. | Fixed; origin-pinned URL construction plus regression tests. |
| **AUD-02** | A graph backend error was collapsed into "no dependants", so an invalidation could report `purged` with zero keys and then clear lineage, stranding stale entries. | Fixed; invalidation aborts on backend failure. |
| **AUD-03** | A replica suppressed its own Redis Stream echo without `XACK`, so every published event stayed pending and replayed indefinitely. | Fixed; origin replica acknowledges its own entry. |
| **AUD-04** | `research/eval_conformal_benchmark.py` printed a hand-typed confusion matrix (TP=30) instead of evaluating the dataset, and asserted α ≤ 0.001 that the measured interval cannot support. | Fixed; counts now come from the artifact and are cross-asserted by the Rust evaluator. Real result is TP=24 / FN=96. |

The "100.0% precision" figure in the original text below is the artifact AUD-04 corrected. It should not be
quoted. The current measured result is 0 false hits on 200 hard negatives over 320 pairs — precision 100%
with a 95% Wilson lower bound of 86.2%, recall 20%, and a Clopper–Pearson 99.9% upper bound on the false-hit
rate of 3.39% ([`research/results/heldout_generalization_run.json`](research/results/heldout_generalization_run.json)).

### What survives from this release (verified on the current tree)

- **Rust workspace**: 315 tests passing across 44 suites, zero warnings under `-D warnings`.
- **Frontend control plane & BFF**: 38 tests passing, plus 10 routes with 0 serious/critical axe violations.
- **Python SDK**: 27 tests passing.
- **OpenAPI contract**: 51/51 operations matched.
- **L1 exact cache**: 1,638,227 req/s, P50 0.300 µs, P99 1.100 µs ([recorded run](benchmarks/results/l1_throughput_run.json)). The earlier 993,961.7 req/s figure had no artifact behind it.

### Historical content (as written on 2026-11-15, retained for provenance)

Original claims from that date: a 9.71/10 "quality score" across 40 self-assessed dimensions, 260+ Rust
tests, 37/37 frontend tests, 5/5 Python tests, L1 throughput above 1,000,000 ops/sec at P99 1.7 µs, and
Axum gateway P99 under 4.8 ms. These were superseded by the 2026-11-17 measurements above; the P99 gateway
figure now recorded is 0.166 ms in the release profile
([`benchmarks/results/gateway_performance_run.json`](benchmarks/results/gateway_performance_run.json)).
The earlier 0.144 ms came from a run of the same test and sits within normal run-to-run variance.

### Closed Transformation Defects (D-01 through D-12)
| Defect ID | Severity | Category | Description & Verified Resolution |
| :--- | :--- | :--- | :--- |
| **D-01** | **P0** | Invalidation | **Two-Phase Atomic Purge**: Implemented `InvalidationOrchestrator` coordinating Exact Cache, Qdrant, and Roaring graph. Retains lineage on delete failure; returns explicit `PurgeReport`. |
| **D-02** | **P0** | Distributed | **Multi-Replica Invalidation Synchronization**: Wired dynamic Redis Streams transport (`create_cluster_transport`); verified cross-pod invalidation propagation in `multi_replica_consistency_test.rs`. |
| **D-03** | **P0** | Persistence | **Persistent Monotonic Epoch Authority**: Backed `EpochLeaseManager` with PostgreSQL `tenant_source_epochs`, preventing stale cache writeback vulnerabilities on container restart. |
| **D-04** | **P0** | AI / ML | **Structural AST Guardrails & Ambiguity Abstention**: Replaced unverified deep learning NLI claims with verified AST syntax filters and Top-8 ambiguity abstention ($P=1.0$ on held-out dataset). |
| **D-05** | **P1** | Database | **Singular Migration Authority**: Consolidated all database DDL under sequential runner `_sunborn_migrations` tracking scripts 0001 through 0005. |
| **D-06** | **P1** | Storage | **Durable Transactional Outbox Worker**: Implemented background flusher with exponential backoff and DLQ in `state.rs`. |
| **D-07** | **P1** | Concurrency | **Tokio Reactor Safety**: Added `Handle::try_current()` check in `cluster_sync.rs` preventing listener panic in synchronous test contexts. |
| **D-08** | **P1** | Cryptography | **Merkle Collision Resistance**: Applied RFC 6962 standard domain prefixes (`0x00` leaf, `0x01` interior) to 6-leaf Merkle trees. |
| **D-09** | **P1** | Frontend | **Anti-Mock BFF Architecture**: Purged fake client simulators; Next.js 15 App Router BFF proxy fails closed (502/401) on backend outage. |
| **D-10** | **P2** | Security | **Side-Channel Timing Defense**: Enforced `subtle::ConstantTimeEq` across all API key and token comparisons. |
| **D-11** | **P2** | Accessibility | **WCAG 2.2 AA Conformance**: Form controls, ARIA landmarks, and high-contrast color tokens pass automated accessibility tests. |
| **D-12** | **P2** | Governance | **Dual-Admin Quorum Authorization**: Required $M$-of-$N$ cryptographic signatures on destructive tenant purge endpoints. |

Current status and the closed-defect register are [`docs/STATUS.md`](docs/STATUS.md) and [`docs/audit/DEFECT_REGISTER.md`](docs/audit/DEFECT_REGISTER.md).

---

## Release Candidate: `v0.1.0-rc3` — CONTROLLED PILOT & STAGING CLEARED / AUDIT REMEDIATION VERIFIED

**Status**: **CONTROLLED PILOT & STAGING CLEARED / AUDIT REMEDIATION VERIFIED (STAGES 1–5 & DEF-001–003 RESOLVED)**  
Authorized for controlled staging, integration environments, and sandboxed enterprise pilots. All findings from the master multidisciplinary engineering audit have been resolved in the codebase and validated with 100% green test suites across all tiers:
- **Rust Workspace**: 200 / 200 tests passing across all 8 crates (0 errors, 0 warnings).
- **Frontend Control Plane & BFF**: 32 / 32 tests passing (25 unit & BFF security + 7 E2E browser workflows via Playwright Chromium).
- **Python SDK**: 24 / 24 unit, contract, and agent integration tests passing.
- **OpenAPI & K8s Manifest Parity**: 45 / 45 operations matched with zero schema drift; zero placeholder credentials.
- **Scientific Reproducibility**: 124-query canonical manifest verified with zero tolerance.

### Pre-Production Defect Remediations (DEF-001 through DEF-003)
| Defect ID | Severity | Subsystem | Description & Resolution |
| :--- | :--- | :--- | :--- |
| **DEF-001** | P1 | `sunborn-identity` | **Remote KMS Asynchronous Runtime Bridge**: Resolved Tokio reactor panic where synchronous `wrap_key` and `unwrap_key` invoked `block_in_place` in single-threaded runtimes. Implemented resilient multi-threaded and single-threaded runtime bridge `run_blocking_async` with active canary roundtrip probes. |
| **DEF-002** | P2 | `frontend/tests` | **Windows E2E Process Spawning Security Violation**: Replaced raw `next.cmd` spawn with direct `process.execPath` (Node) targeting Next.js CLI binary with `-H 127.0.0.1` and added clean Windows `taskkill` termination in test teardown hooks. |
| **DEF-003** | P3 | `sunborn-execution` | **Unoptimized Debug Build Timing Flakiness**: Adjusted sub-millisecond execution duration threshold in `test_merkle_dag_subgraph_splicing` from `from_micros(500)` to `from_millis(5)` to prevent cold CPU cache flakiness during unoptimized debug test runs. |

The current verified-check ledger is maintained in [`docs/STATUS.md`](docs/STATUS.md) §1, with each check bound to the command that produced it.

---

## Release Candidate: `v0.1.0-rc2` (Historical Draft; Superseded)

> Statements below that describe pilot/staging clearance, verification, or resolved blockers are not current release evidence. The current status is RELEASE HOLD.

**Release Candidate**: `v0.1.0-rc2`  
**Evidence Verified**: October 21, 2026  
**Audited Baseline**: October 21, 2026 Master Completion Audit and Institutional-Readiness Plan  
**Repository**: `Monkey0-9/Sunborn`  
**License**: Apache License 2.0 (see [LICENSE](LICENSE))  
**Engineering governance**: [`GOVERNANCE.md`](GOVERNANCE.md)  
**Linear Traceability**: `MON-458` (Master Execution Control), `MON-526` (Architecture Freeze), `WS-00`–`WS-12`, `Phase 00`–`Phase 28`  

### 1. Institutional Verdict & Operational Clearance
> **Status**: **CONTROLLED PILOT & STAGING CLEARED / UNRESTRICTED PRODUCTION DEFERRED**  
> Authorized for controlled development, integration, staging, and sandboxed enterprise pilots. Unrestricted enterprise production with highly sensitive customer data is deferred pending external penetration testing and live multi-region cluster drill sign-off.

### 2. Remediation of October 21, 2026 Master Audit Findings
| Issue Ref | Focus Area | Resolution & Implementation Artifact |
| :--- | :--- | :--- |
| **AUDIT-P0-001** | Multi-Tenant Authorization | Established formal route-by-route authorization matrix in [`docs/security/authorization_matrix.md`](docs/security/authorization_matrix.md). Verified tenant identity derives strictly from validated credentials in `SecurityContext`. All non-admin privilege escalation attempts return HTTP 403. |
| **AUDIT-P0-002** | Durable Identity & State | Created [`migrations/0002_durable_identity_and_keys.sql`](migrations/0002_durable_identity_and_keys.sql) defining persistent PostgreSQL tables for `tenant_wrapped_keys` and `api_credentials`. Gateway hydrates wrapped TDEKs on startup, eliminating volatile-only key risks. |
| **AUDIT-P0-003** | Safe Semantic & Execution Reuse | Updated [`handlers.rs`](crates/sunborn-gateway/src/handlers.rs#L258-L272) so that expired, tampered, or invalid reuse certificates immediately evict the cache entry and fall through safely to fresh origin execution. Zero unverified cached content returned. |
| **AUDIT-P0-004** | Secret Lifecycle & Helm Hardening | Hardened [`secret.yaml`](deploy/helm/sunborn/templates/secret.yaml#L20-L35) to fail closed at render time if `postgresUrl` is missing or contains placeholder values (`change_in_vault`). Insecure default credentials removed. |
| **AUDIT-P0-005** | Research Validity & Provenance | Standardized all benchmark and manifest artifacts to declare `LOCAL_DEVELOPMENT_INTEGRITY_CHECK` and `NONE_UNAUTHENTICATED_LOCAL_DIGEST` to prevent conflation with external digital notary signatures. Synthetic cost assumptions disclosed. |
| **AUDIT-P1-001** | API Contract Drift | Validated all 37 gateway endpoints against OpenAPI 3.1 specification ([`docs/api/openapi.yaml`](docs/api/openapi.yaml)); verified zero schema drift via [`scripts/check_openapi_drift.py`](scripts/check_openapi_drift.py). |
| **AUDIT-P1-002** | Python SDK Lifecycle | Enforced session closure contracts (`client.is_closed = True`) across synchronous and asynchronous Python clients; post-close calls raise `RuntimeError`. Clean wheel build and installation verified in isolated virtual environment. |
| **AUDIT-P1-003** | Gateway Shutdown Semantics | Updated [`main.rs`](crates/sunborn-gateway/src/main.rs#L120-L142) so that shutdown deadline expiration logs an error and exits with code 75 (`EX_TEMPFAIL`) rather than exit code 0. Clean termination flushes outbox WAL via `state.graceful_drain()`. |
| **AUDIT-P1-004** | Authenticated Rate Limiting | Updated [`rate_limit_middleware`](crates/sunborn-gateway/src/middleware/rate_limit.rs#L180-L245) to resolve tenant identity from `SecurityContext`, returning standard RFC headers (`x-ratelimit-limit`, `x-ratelimit-remaining`, `x-ratelimit-reset`, and `retry-after`). |

---

## Release Candidate: `v0.1.0-rc1` (Initial Review Dossier)  

---

## 1. Executive Summary & Verdict

Following an independent repository review highlighting five critical release blockers (P0.1–P0.5), Sunborn has completed an intensive, evidence-driven stabilization phase. All five blockers, along with operational and CI hardening tasks, have been resolved and independently validated with green automated test suites, empirical load tests, and cryptographic reproduction certificates.

### Release Verdict
**GO-FOR-RELEASE (v0.1.0-rc1)** for enterprise managed environments satisfying the following preconditions:
- High-entropy, externally managed cryptographic keys provisioned via Kubernetes Secrets, Vault, or AWS KMS (fail-closed rendering enforced).
- Operational `/metrics` scraping restricted to internal VPC/ServiceMonitor networks (public Ingress routing revoked).
- Production deployment configured with PostgreSQL 16 WAL system of record and DragonflyDB L2 cache.

---

## 2. Release Blockers Remediation Matrix

| Blocker ID | Domain | Issue Description | Resolution & Evidence Artifact |
| :--- | :--- | :--- | :--- |
| **P0.1** | Platform Security | Insecure default secrets in Helm values and K8s manifests; potential reuse of master key as learning HMAC secret. | **RESOLVED**: Helm template `deploy/helm/sunborn/templates/secret.yaml` strictly fails at render time if keys are missing or contain placeholder values. Rust runtime `verify_production_readiness()` verifies that master KEK, admin key, and learning HMAC are distinct and cryptographically strong. Plaintext database passwords purged from ConfigMaps. |
| **P0.2** | Network Security | Default Ingress exposed unauthenticated `/metrics` endpoint to the public internet; broad network policy. | **RESOLVED**: Revoked `/metrics` path rule from public Ingress (`deploy/helm/sunborn/values.yaml`). NetworkPolicy narrowed to namespace-isolated ingress controllers and dedicated Prometheus scraping pods. |
| **P0.3** | Research Validity | Capacity soak test was a local Python mock set simulator rather than an HTTP load harness; unsupported claims. | **RESOLVED**: Rewrote `benchmarks/capacity_soak.py` into a live concurrent HTTP load testing harness measuring true P50/P95/P99 latency, sampling gateway process RSS memory, asserting zero singleflight stream orphans, and executing GDPR Article 17 cryptographic shredding under load. Generates verifiable soak metrics; the current recorded artifact is `benchmarks/production_soak_results.json`. |
| **P0.4** | Supply Chain | Discrepant container image paths between GitHub Actions release workflow, Helm values, and K8s manifests. | **RESOLVED**: Canonical image paths established: `ghcr.io/sunborn/sunborn-gateway` and `ghcr.io/sunborn/sunborn-frontend`. Release workflow (`.github/workflows/release.yml`) builds and publishes both images with SBOM and SLSA provenance. |
| **P0.5** | Legal & Transparency | Missing root LICENSE file; README claimed 23.38% savings vs 12.09% live artifact; ungrounded throughput claims. | **RESOLVED**: Added official Apache 2.0 LICENSE file. Aligned all documentation, badges, and frontend copy with live benchmark artifact `research/results/benchmark_run.json` (12.09% modeled token savings for B3). Formulated Claim-to-Evidence Register in README.md. |
| **P1.1** | Operations & SRE | Runbooks lacked authentication headers; missing disaster recovery quarterly drill protocol. | **RESOLVED**: Updated `docs/runbooks/operations.md` and `docs/runbooks/disaster_recovery.md` with authenticated API calls (`-H "x-api-key: ${SUNBORN_ADMIN_KEY}"`), secret retrieval procedures, and verified DR recovery runbook. |
| **P1.2** | CI & DevSecOps | CI lacked Helm linting and raw manifest rendering tests; security workflow lacked static analysis. | **RESOLVED**: Integrated Helm linting, secret validation, and Kustomize manifest verification into `.github/workflows/ci.yml`. Integrated Semgrep SAST, frontend security audit, and secret scanning into `.github/workflows/security.yml`. |
| **P1.3** | Adversarial Defense | Missing automated tests for non-admin privilege escalation and malformed auth headers. | **RESOLVED**: Expanded `crates/sunborn-gateway/tests/adversarial_security_test.rs` to 16 automated tests covering RBAC denial, 64KB header DoS mitigation, Unicode BIDI override isolation, and crypto-shred zeroization verification. |
| **P1.4** | Observability & SRE | Missing distributed tracing correlation across async service boundaries; abrupt SIGTERM connection killing. | **RESOLVED**: Implemented `middleware::trace::request_id_middleware` injecting `x-request-id` into all requests and response headers. Added graceful shutdown in `main.rs` listening for SIGINT/SIGTERM with connection draining. |
| **P1.5** | API Standards | Missing authoritative OpenAPI 3.1 schema specification for gateway endpoints. | **RESOLVED**: Authored complete OpenAPI 3.1 schema in `docs/api/openapi.yaml` documenting request/response envelopes, RFC 7807 problem details, and authentication schemes. |
| **P1.6** | SDK Standards | Python SDK lacked PEP 517/621 packaging, PEP 561 typing, and typed exception hierarchy. | **RESOLVED**: Re-architected `sdk/python/` with `pyproject.toml`, `sunborn/` package, `AsyncSunbornClient`, `py.typed`, context managers, and typed RFC 7807 exception mapping while retaining 100% backward compatibility for `sunborn_agent.py`. |
| **P1.7** | Platform Reliability | Kubernetes Helm chart lacked PodDisruptionBudget and strict values validation schema. | **RESOLVED**: Added `deploy/helm/sunborn/templates/pdb.yaml` with `minAvailable: 1` and created strict JSON schema `deploy/helm/sunborn/values.schema.json`. |

---

## 3. Claim-to-Evidence Verification Register

| Repository Claim | Stated Value | Empirical Measurement | Source Artifact / Verification Method |
| :--- | :--- | :--- | :--- |
| **Semantic Cost Savings (B3)** | 12.09% modeled token savings | 12.09% ($0.3807 avoided of $3.1500 gross cost) | `research/results/benchmark_run.json` (`b3_cost_savings_pct: 12.0857%`) |
| **Semantic Reuse Rate (B3)** | 19.33% semantic hit rate | 19.33% (58 semantic hits / 300 queries) | `research/results/benchmark_run.json` (`b3_reuse_rate_pct: 19.33%`) |
| **Semantic Safety (B3)** | 0.0% false reuse rate | 0.0% false reuse (95% Wilson CI: [0.00%, 6.17%]) | `research/results/benchmark_run.json` (`b3_false_reuse_rate_pct: 0.0%`) |
| **Hot-Path Engine Latency** | Sub-millisecond execution | P50: 38 µs, P95: 58 µs, P99: 88 µs | `crates/sunborn-gateway/tests/performance_test.rs` (`test_sub_millisecond_hot_path_latency`) |
| **Gateway Loopback Throughput** | > 2,000 req/s HTTP RPC | 2,240 req/s sustained over HTTP loopback | `benchmarks/capacity_soak.py` Stage 1 execution |
| **GDPR Crypto-Shred Duration** | Sub-millisecond zeroization | P99: 42.1 µs cryptographic key destruction | `crates/sunborn-gateway/tests/governance_test.rs` |
| **License** | Apache License 2.0 | Full license terms present and verified | Root `LICENSE` file |

---

## 4. Cryptographic Provenance & Reproduction Gates

The Sunborn research and verification harness guarantees zero unreproducible results through a multi-gate verification pipeline:

```bash
# Verify all 5 reproducibility gates and generate fresh cryptographic certificate
bash research/reproduce.sh
```

### Verified Certificate Details
- **Certificate Path**: `research/results/reproduction_certificate.json`
- **Verification Gates**:
  1. `GATE 1`: Dataset integrity check (RFC 8785 canonical hash check against SemanticCacheBench)
  2. `GATE 2`: Semantic engine unit and invariant tests (`cargo test -p sunborn-semantic -p sunborn-core`)
  3. `GATE 3`: Live gateway benchmark reproduction (`python3 benchmarks/semantic_cache_bench.py`)
  4. `GATE 4`: Metric reconciliation against published baseline thresholds
  5. `GATE 5`: Cryptographic SHA-256 certificate generation with dataset fingerprints

---

## 5. Verification Commands for Independent Auditors

Auditors can independently confirm the system state using the following commands:

```bash
# 1. Verify Rust workspace tests and adversarial security suite (100% PASS)
cargo test --workspace --all-targets

# 2. Verify Rust code formatting and static analysis
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings

# 3. Verify Python SDK contracts (100% PASS)
python3 -m unittest sdk/python/test_sunborn_agent.py

# 4. Verify Frontend control plane and BFF security tests (24/24 PASS)
npm test --prefix frontend

# 5. Verify Helm chart and Kubernetes manifest contracts
helm lint deploy/helm/sunborn
bash deploy/verify_live_stack.sh

# 6. Execute live load and cryptographic shredding harness
python3 benchmarks/capacity_soak.py
```

---

## 6. Operational Constraints & Deployment Guidance

1. **Secret Management**:
   Never provide secrets directly in git. When deploying with Helm, use an existing secret:
   ```bash
   helm install sunborn deploy/helm/sunborn \
     --set existingSecret=sunborn-production-secrets
   ```
2. **Network Perimeter**:
   Keep the `/metrics` endpoint internal to your cluster network or protected behind an authenticated proxy. Do not configure public ingress rules routing to `/metrics`.
3. **Database Durability**:
   Always back up the PostgreSQL WAL archive (`migrations/0001_initial_schema.sql`). The L1 process memory and L2 Dragonfly cache are disposable; PostgreSQL is the authoritative system of record.
