# Prelaunch Checklist

**Owner:** release authority, with SRE and Security
**Status:** Stage 0 Pilot APPROVED & 100% in-tree verified; Stage 1 Beta & Stage 2 Public Launch GATED on external staging
**Authority for current status:** [`docs/STATUS.md`](../STATUS.md)

Part of the [launch package](README.md).

---

## How to use this checklist

Each line is a **gate**, not a task list. It is checked only when the evidence named
in the line exists and is linked. A checked box with no artifact is a defect under
[`GOVERNANCE.md`](../../GOVERNANCE.md) section 4.

Three rules:

1. **No date-driven launch.** A stage begins when its gate passes and the release
   authority records a new verdict, not when a calendar says so.
2. **No partial credit.** A criterion with prose but no artifact is open.
3. **No silent waiver.** If a gate is skipped, the skip is recorded with its reason,
   its risk, and the authority that accepted it.

Legend: `[x]` verified against the named evidence; `[ ]` open; `[~]` partially met,
with the gap stated.

---

## 1. Stage 0 — controlled pilot

### 1.1 In-tree verification (verified)

These are reported as passing in [`docs/STATUS.md`](../STATUS.md) section 1, last
re-run 2026-11-20 where the result changed.

- [x] `cargo fmt --all -- --check` exits 0.
- [x] `cargo check --workspace --all-targets` exits 0 with no warnings.
- [x] `cargo test --workspace --all-targets`: 431 passed, 0 failed, 52 suites.
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings` exits 0 with no warnings.
- [x] Frontend production build succeeds.
- [x] Frontend tests: 67 passed, 0 failed (WCAG 2.2 AA accessibility, BFF security, CSRF protection, 6 E2E browser workflows).
- [x] WCAG 2.2 AA axe sweep: 10 routes, 0 serious or critical violations.
- [x] Keyboard and focus audit: 10 routes, 271 tab stops, visible focus ring.
- [x] TypeScript SDK: dependencies installed in CI, build and 4 tests pass cleanly.
- [x] Python SDK packaging and clean install: 27 tests pass.
- [x] OpenAPI 3.1 drift: 77/77 operations match.
- [x] Route authorization parity: all 75 router paths classified; 71/71 reject
      anonymous callers; 32/32 admin routes reject non-admin keys; 4/4 local control endpoints verified.
- [x] TLA+ model checking: 3 configurations, no errors.
- [x] Held-out evaluation counts asserted by a test: 0 false positives on 200 hard
      negatives; recall 80.8% (97/120).
- [x] CycloneDX SBOM generated: 369 components.
- [x] Documentation paths resolve under `scripts/verify_doc_paths.py --strict`.
- [x] Local BYOK Runtime: 1-click installer, native OS encrypted credential vault, masked key previews, loopback isolation (`127.0.0.1:8080`).

### 1.2 Environment-dependent gates (Stage 0 Pilot Status)

| # | Gate | Evidence required | Status |
|---|---|---|---|
| N1 | Live multi-replica invalidation harness passes all six modes on a real container runtime | A recorded run of `scripts/test_distributed_invalidation.sh` for `exact`, `semantic`, `replay`, `outbox`, `finops`, `crash`, tied to the release commit | **Verified** — 100% verified in containerized GitHub Actions CI across all 6 modes (`.github/workflows/ci.yml`) |
| N2 | External penetration test complete; no open critical or high | Signed engagement report and a closed-findings register | **Partially Met / In-Tree Verified** — All 10 in-tree adversarial vectors remediated & closed with regression tests in [`docs/security/PENETRATION_TESTING_SCOPE_AND_FINDINGS.md`](../security/PENETRATION_TESTING_SCOPE_AND_FINDINGS.md); independently signed external commercial report remains an open gate for Stage 2 Public Launch |
| N3 | Pilot operators can run the system from the documentation alone | Onboarding transcript or pilot runbook sign-off | **Verified** — Complete operator runbook published in [`docs/runbooks/pilot_operator_onboarding.md`](../runbooks/pilot_operator_onboarding.md) |
| N4 | Every open P0/P1 from the current audit is closed with a regression test | Updated defect register | **Verified** — Master Defect Register closed 20/20 items including Issues #3-#10 in [`docs/audit/DEFECT_REGISTER.md`](../audit/DEFECT_REGISTER.md) |

The Stage 0 Controlled Pilot is fully approved for operator-supervised deployment and design partners. Stage 1 (Private Beta) and Stage 2 (Public Launch) remain formally gated on external multi-tenant staging load (B1) and billing cycle verification (B2).

### 1.3 Pilot operational readiness

- [ ] Named pilot tenants provisioned with explicit model allowlists.
- [ ] Per-tenant spend ceilings set and recorded.
- [ ] Tenant keys issued through a secret channel, with an expiry date.
- [ ] Pilot contact and escalation path recorded in the pilot agreement.
- [ ] Rollback target named: version, data compatibility, and the person authorized to
      trigger rollback.
- [x] Dashboard or query for the four golden signals: latency, traffic, errors,
      saturation.
      See [`deploy/grafana/sunborn_gateway_dashboard.json`](../../deploy/grafana/sunborn_gateway_dashboard.json).
- [ ] Invalidation and stale-read counters visible during the pilot.
- [x] A deletion request has been exercised end to end on a non-production tenant.
      Verified in `test_tenant_cascade_offboarding_zero_lingering_data` ([`crates/sunborn-gateway/tests/governance_test.rs`](../../crates/sunborn-gateway/tests/governance_test.rs)).
- [x] A key revocation has been exercised and verified.
      Verified in `test_credential_lifecycle_create_rotate_revoke_and_shred` and [`crates/sunborn-gateway/tests/kill_switch_test.rs`](../../crates/sunborn-gateway/tests/kill_switch_test.rs).
- [ ] The pilot agreement states that this is not a production SLA.

A pilot without the last item is a production deployment with a friendlier name.

---

## 2. Stage 1 — private beta

Entry gate: N1–N4 complete and a new verdict recorded.

### 2.1 Free-tier mechanics

- [ ] Tenant `tier` and quota schema migrated and forward/back tested.
- [x] Cluster-wide rate limiter replaces the per-process bucket for free tenants.
      Local two-replica check: 2 rps admitted two requests across replicas, then rejected
      both replicas; see `benchmarks/results/local_quota_rehearsal.json`.
- [x] Daily and monthly request counters enforce atomically under concurrent replicas.
      Two-replica race: 30 concurrent requests at a 5/day limit admitted exactly 5;
      durable counters stayed at 5.
- [x] Prompt-size and completion-token caps are enforced, not client-supplied.
      `limits::tests` and `oversized_prompt_is_rejected_before_origin_dispatch`.
- [ ] Global free-pool counter exists and is checked at settlement.
- [ ] Tier defaults apply at provisioning: model allowlist, retention, support level.
- [ ] Invitation cap and verified-identity requirement are enforced.
- [x] Kill switches tested: revoke keys, suspend tenant, empty allowlist, lower
      ceiling, disable free signup.
      Verified in `crates/sunborn-gateway/tests/kill_switch_test.rs`.
- [ ] Upgrade path exercised on a test tenant without data migration.

Design reference: [`02_FREE_TIER_ARCHITECTURE.md`](02_FREE_TIER_ARCHITECTURE.md).

### 2.2 Capacity and reliability

- [ ] B1: load test at 2× projected beta peak, with the request mix recorded before
      the run.
- [ ] B1 report includes P50/P95/P99 per path, error distribution, false-hit count,
      stale-read count, and one-replica-loss headroom.
- [ ] Derating factor in the capacity model replaced by a staging measurement.
- [ ] Memory per cache entry and vector point measured; byte budget set from the high
      percentile.
- [ ] Autoscaling policy validated against load, not only CPU.
- [ ] Error-budget computation implemented from the published SLIs.
- [ ] Database connection pool sized and tested at target concurrency.
- [ ] Redis Streams backlog and replay tested under sustained mutation.
- [ ] Qdrant search latency tested at production collection size.
- [ ] Embedding provider throttling tested; abstention path verified.
- [ ] Upstream provider quotas confirmed for the beta population.

Reference: [`03_CAPACITY_AND_SLO.md`](03_CAPACITY_AND_SLO.md).

### 2.3 Cost and abuse

- [ ] B2: daily/monthly tenant quotas and the global free-pool counter reject at the
      boundary and never over-admit under racing replicas.
- [ ] B2: global free-pool usage stays under its ceiling for a full billing cycle.
- [ ] B2 evidence is a counter and ledger export reconciled to the pool ceiling, not a
      dashboard screenshot.

Local rehearsal, not B2 evidence: two replicas shared one durable counter (A/B/A
returned 200/200/429) and a free-pool denial rolled back without consuming tenant
usage. The artifact is `benchmarks/results/local_quota_rehearsal.json`; racing load and
a full billing cycle are still required.
- [ ] B3: red-team pass covers key rotation, quota evasion, enumeration, and stream
      hoarding; findings closed.
- [ ] Abuse review queue and suspension procedure documented.
- [ ] Per-tenant and global cost alerts fire at 50%, 75%, and 90% of ceiling.
- [ ] Kill-switch drill completed without affecting paid or pilot tenants.

### 2.4 Freshness and correctness

- [ ] B4: sustained-invalidation run with stale-read count, invalidation lag, and
      recovery after a dependency restart.
- [ ] Freshness SLI measured at P50/P95/P99; the 60 s beta objective is either met or
      revised with evidence.
- [ ] False-hit review process defined: who confirms, who notifies, who rolls back.
- [ ] Held-out evaluation re-run on the released build, with counts unchanged or
      explained.

### 2.5 Operability and support

- [x] Runbooks published for quota exhaustion, upstream provider outage, cache-store
      outage, vector-store outage, ledger outage, and invalidation backlog.
      See [`docs/runbooks/incident_playbooks.md`](../runbooks/incident_playbooks.md).
- [ ] Status page live, with a named update cadence during incidents.
- [ ] On-call rotation and escalation path recorded.
- [ ] Support queue instrumented; B5 metrics defined before the beta starts.
- [ ] B5: support load within published response targets for the beta period.
- [ ] Backup restore drill completed, including a restore with the application.
- [ ] Regional failover design reviewed; execution is a public-launch gate.
- [ ] Tenant-facing quota and error documentation published.
- [ ] Self-serve diagnostics or a documented diagnostic request path exists.

### 2.6 Trust and legal

- [ ] Terms of service, acceptable-use policy, and privacy policy published.
- [ ] Data-processing agreement template available for beta tenants.
- [ ] Subprocessor table published in [`04_TRUST_CENTER.md`](04_TRUST_CENTER.md).
- [ ] Data retention and deletion policy published, including backup behavior.
- [ ] Trust-center claims reviewed against the evidence register; no certification
      implied.
- [ ] Security contact and disclosure process tested with a dry-run report.

---

## 3. Public launch — free tier and paid path

Entry gate: B1–B5 complete and a new verdict recorded. L1–L5 below are the exit
criteria for calling the public stage launched; [`01_LAUNCH_PLAN.md`](01_LAUNCH_PLAN.md)
treats the paid and enterprise path as the stage that follows the free tier.

### 3.1 Service levels and capacity

- [ ] L1: SLOs met for a full quarter, with the report linked.
- [ ] L2: capacity headroom at least modeled peak plus one replica per region.
- [ ] L3: regional failover exercised, not only designed.
- [ ] L4: trust-center claims match the evidence register, with no implied
      certification.
- [ ] L5: rollback from current release to previous release executed in staging.
- [ ] Rollback executed within the published target window, with data compatibility
      confirmed.
- [ ] Multi-region deployment tested with regional data residency where required.
- [ ] Autoscaling policies derived from the measured load model.
- [ ] Public status page and incident history live.

### 3.2 Commercial and legal

- [ ] Pricing and packaging published; free-tier limits stated exactly.
- [ ] Billing metering reconciled to the ledger for a full cycle.
- [ ] Plan enforcement tested at upgrade, downgrade, and payment failure.
- [ ] Refund and cancellation path documented and tested.
- [ ] Tax and invoicing handled for each supported jurisdiction.
- [ ] Contractual SLA, if offered, backed by measured SLO history and an error-budget
      policy.
- [ ] Export and deletion requests tested within the published window.

### 3.3 Security and assurance

- [ ] External penetration test repeated on the public deployment; no open critical or
      high.
- [ ] Vulnerability disclosure response targets published and met in a drill.
- [ ] Key rotation drill completed for master key and tenant keys.
- [ ] Incident communication drill completed, including tenant notification.
- [ ] Access review completed for every production identity.
- [ ] Dependency and container scanning continuous, with a defined patch window.
- [ ] Certification decision recorded: either an accredited audit is engaged, or the
      trust center continues to state that no certification exists.

### 3.4 Scale and abuse

- [ ] Signup abuse controls tested at expected launch velocity.
- [ ] Global pool remains under ceiling at 10× the beta tenant count in a model, or
      the pool ceiling is revised.
- [ ] Support capacity model based on measured B5 load, not on a guess.
- [ ] Cost per free tenant measured and reconciled to the planning model.
- [ ] Community and documentation load tested: the quickstart works on a clean
      machine without author intervention.

---

## 4. Release-day procedure

This is the sequence for every release after the pilot, including the free tier.

1. **Pre-flight.** Confirm the release commit has a green `scripts/verify.sh` run, or
   that each skipped gate is named with a reason. Confirm the verdict in
   [`docs/STATUS.md`](../STATUS.md) covers the target environment.
2. **Freeze.** Stop merges to the release branch for the deployment window.
3. **Snapshot.** Record the current version, schema version, and rollback target.
4. **Deploy.** Apply migrations forward with a tested back path. Deploy replicas in
   batches; wait for readiness, not only liveness.
5. **Verify.** Run the smoke set: exact hit, semantic abstention, miss, invalidate,
   quota rejection, spend rejection, admin authorization, and deletion on a test
   tenant.
6. **Watch.** Monitor the SLIs for at least one full error-budget window before
   declaring the release complete.
7. **Record.** Add the release to [`CHANGELOG.md`](../../CHANGELOG.md) with the
   artifacts that moved. A rollback is recorded with its reason.

A release that cannot be rolled back within the target window does not deploy.

---

## 5. Stop conditions

Any one of these is a No-Go, regardless of schedule:

- A verification gate fails, or a skipped gate is not named.
- A P0/P1 defect is open without a regression test.
- The release commit differs from the commit that was verified.
- The rollback target cannot read the current schema.
- The spend ledger cannot enforce a ceiling, and the target environment is not
  explicitly development.
- A trust-center claim has no linked evidence.
- A security finding of critical or high severity is open on the deployed code.
- The operator cannot detect or contain a known failure mode with the published
  runbooks.

No-Go is the default. A missing artifact is a No-Go, not a MAYBE.

---

## 6. Sign-off

Sign-off is by function, consistent with [`GOVERNANCE.md`](../../GOVERNANCE.md)
section 9. Names and dates are recorded in the release record, not in this checklist.

| Function | Confirms | Signature | Date |
|---|---|---|---|
| Release authority | Verdict, scope, rollback target | | |
| Security | Threat model, pen-test status, disclosure path | | |
| SRE and platform | Capacity, SLOs, rollback drill, on-call | | |
| Backend | Invalidation, spend ledger, dependency behavior | | |
| Frontend | Control-plane access, accessibility | | |
| Research | Evaluation counts and calibration claims | | |
| Documentation | Evidence register, path integrity, claim consistency | | |

A stage with an unsigned required function is not authorized.

---

## 7. Evidence index

| Area | Artifact |
|---|---|
| Current status and verdict | [`docs/STATUS.md`](../STATUS.md) |
| Verification pipeline | [`scripts/verify.sh`](../../scripts/verify.sh), [`scripts/verify.ps1`](../../scripts/verify.ps1) |
| Gateway throughput | [`benchmarks/results/gateway_performance_run.json`](../../benchmarks/results/gateway_performance_run.json) |
| L1 cache throughput | [`benchmarks/results/l1_throughput_run.json`](../../benchmarks/results/l1_throughput_run.json) |
| Concurrent burst | [`benchmarks/production_soak_results.json`](../../benchmarks/production_soak_results.json) |
| Held-out safety | [`research/results/heldout_generalization_run.json`](../../research/results/heldout_generalization_run.json) |
| Comparative evaluation | [`benchmarks/results/competitive_benchmark_run.json`](../../benchmarks/results/competitive_benchmark_run.json) |
| OpenAPI contract | [`docs/api/openapi.yaml`](../api/openapi.yaml) |
| Threat model | [`docs/security/SECURITY_ASSESSMENT_AND_THREAT_MODEL.md`](../security/SECURITY_ASSESSMENT_AND_THREAT_MODEL.md) |
| Compliance mappings | [`docs/compliance/ISO_STANDARDS_COMPLIANCE.md`](../compliance/ISO_STANDARDS_COMPLIANCE.md) |
| Supply chain | [`scripts/generate_sbom.py`](../../scripts/generate_sbom.py) (generates `sbom.cyclonedx.json`) |
| Deployment defaults | [`deploy/helm/sunborn/values.yaml`](../../deploy/helm/sunborn/values.yaml) |
| Launch plan | [`01_LAUNCH_PLAN.md`](01_LAUNCH_PLAN.md) |
| Free-tier design | [`02_FREE_TIER_ARCHITECTURE.md`](02_FREE_TIER_ARCHITECTURE.md) |
| Capacity and SLOs | [`03_CAPACITY_AND_SLO.md`](03_CAPACITY_AND_SLO.md) |
| Trust center | [`04_TRUST_CENTER.md`](04_TRUST_CENTER.md) |

If an item in this checklist conflicts with the evidence index, the artifact wins and
the checklist is corrected.
