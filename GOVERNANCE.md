# Engineering Governance

Sunborn is governed by evidence, not assertion. This document defines how technical
decisions are made, what a change must prove before it merges, who may authorize a
release, and how security, privacy, and data handling are controlled. It applies to
every change in this repository, including documentation.

The operating assumption is simple: the most expensive defect is a confident claim
with no artifact behind it. Every rule below exists to make that defect hard to ship.

---

## 1. Principles

1. **Evidence over assertion.** A quantitative claim must name the artifact that
   produced it. Prose that contradicts an artifact is a defect, not a difference of
   opinion.
2. **One source of truth for status.** [`docs/STATUS.md`](docs/STATUS.md) is
   authoritative. Older audits, scores, and readiness statements are retained as
   history and are not current status.
3. **Fail closed.** In production mode the gateway refuses to start, or returns HTTP
   503, rather than serve traffic through a degraded dependency. Releases follow the
   same rule: an unverified gate blocks, it does not warn.
4. **Reversibility first.** A change that cannot be rolled back needs a written
   rollback plan before it is approved.
5. **Least privilege.** Every route, token, key, and deployment identity carries the
   minimum authority it needs. New authority is a reviewed decision, not a default.
6. **Blameless learning.** Incidents are analyzed for causes and missing controls,
   not for individuals.

---

## 2. Decision rights

| Decision | Authority | Mechanism |
|---|---|---|
| Change to a public request/response contract | Gateway maintainers and API reviewers | OpenAPI change plus drift gate; see [`docs/api/openapi.yaml`](docs/api/openapi.yaml) |
| Change to authentication, authorization, or key handling | Security reviewers, with a second approver | Threat-model update and adversarial tests |
| Change to the cache admission or invalidation logic | Semantic and invalidation maintainers | Held-out evaluation and regression tests |
| Database schema change | Data maintainers | Versioned migration in [`migrations/`](migrations/), forward-and-back tested |
| New external dependency | Architecture reviewers | Supply-chain review; SBOM regenerated |
| New public route | Gateway maintainers | Route authorization contract updated, or the build fails |
| Release verdict | Release authority | Evidence register in [`docs/STATUS.md`](docs/STATUS.md) |
| Emergency security fix | Security on-call | Applied first, reviewed and retroactively documented after containment |

No single person holds both the authority to change a security control and the
authority to approve that change unreviewed.

---

## 3. Change classes

Every pull request is classified. The class determines the minimum gates.

| Class | Examples | Required before merge |
|---|---|---|
| **C1 - Documentation** | README, runbooks, comments, link fixes | Path verifier, reviewer, no contradiction with an artifact |
| **C2 - Routine code** | Internal refactor, test, non-contract behavior | Format, compile, tests, clippy, reviewer |
| **C3 - Sensitive code** | Auth, crypto, tenant isolation, cache correctness, public API | C2 plus security review, regression test naming the failure mode, and an updated artifact where a claim changes |
| **C4 - Release** | Version tag, production configuration, deployment | C2/C3 plus the full verification pipeline and a release-authority verdict |

A pull request that changes a measured claim without changing its artifact is C1 at
best and is rejected. A pull request that changes an artifact without changing the
prose that quotes it is incomplete.

---

## 4. The evidence rule

This is the rule that separates a demonstrable system from a persuasive one.

- **Every number has a home.** Throughput, latency, precision, recall, test counts,
  and component counts live in checked-in JSON or generated reports. The README links
  to the file; it does not restate a number that no file contains.
- **Tests assert the artifacts.** Where a claim matters, a test reads the artifact and
  fails if the code and the artifact disagree. The held-out evaluation counts are one
  example; the route authorization contract is another.
- **Absence of evidence is stated.** When a property is not established, the
  documentation says so next to the property, not in a footnote. See
  [`README.md`](README.md) section 5.
- **History is labeled.** Superseded audits and scores carry a banner and are not
  cited as current. When two documents disagree, [`docs/STATUS.md`](docs/STATUS.md)
  wins and the older document is corrected or marked historical.

A reviewer may block any change on the grounds that it makes a claim the repository
cannot support. That block does not require a second opinion.

---

## 5. Release governance

A release is a claim about what the system does, made to people who will depend on it.
It is authorized only against the evidence register.

1. **Verdict.** The current verdict, its conditions, and its blockers are recorded in
   [`docs/STATUS.md`](docs/STATUS.md). The verdict is one of: NO-GO, CONDITIONAL GO
   (named scope), or GO.
2. **Scope.** A verdict authorizes a named environment and audience. A controlled
   pilot verdict does not authorize public traffic.
3. **Blockers are explicit.** Environment-dependent gaps, such as a live multi-replica
   run or an external security assessment, are listed as blockers rather than
   described as future work.
4. **Rollback is written.** Every release names the version to return to, the data
   compatibility statement, and the person authorized to trigger rollback.
5. **The changelog is a record, not a pitch.** [`CHANGELOG.md`](CHANGELOG.md) records
   what changed and which artifacts moved. Withdrawn claims stay visible with the
   reason they were withdrawn.

The full verification pipeline is [`scripts/verify.sh`](scripts/verify.sh) and its
PowerShell equivalent [`scripts/verify.ps1`](scripts/verify.ps1). A green local run is
necessary and not sufficient: gates that need external infrastructure are called out
when they were not executed.

---

## 6. Security and incident governance

- **Disclosure.** Vulnerabilities are reported privately according to
  [`SECURITY.md`](SECURITY.md). Public issues are not used for undisclosed
  vulnerabilities.
- **Severity.** Findings are graded by exploitability and blast radius, not by the
  effort required to fix them. A credential-exfiltration path is critical even if the
  fix is a one-line normalization.
- **Containment before ceremony.** On-call may contain an active incident without
  prior approval. The change is reviewed and documented immediately afterward.
- **Dual control.** Destructive administrative endpoints require a second
  authorization. This is enforced in code, not in a policy document.
- **Post-incident.** Every material incident produces a blameless review with: the
  timeline, the contributing causes, the control that failed, the regression test that
  now covers it, and the owner of any residual risk.
- **No silent degradation.** A dependency outage must surface as an error, not as a
  quieter or less safe mode of operation. This is why the gateway fails closed in
  production.

The threat model and security assessment are maintained in
[`docs/security/SECURITY_ASSESSMENT_AND_THREAT_MODEL.md`](docs/security/SECURITY_ASSESSMENT_AND_THREAT_MODEL.md).

---

## 7. Data and privacy governance

- **Tenant isolation.** Cache keys, vector records, encryption keys, spend ledgers, and
  administrative views are namespaced by tenant. Cross-tenant reads are a critical
  defect.
- **Purpose limitation.** Customer prompts and completions are processed to serve the
  request and operate the cache. They are not used to train models.
- **Minimization.** Logs record decisions and identifiers, not prompt bodies, unless a
  tenant explicitly enables content logging for its own use.
- **Deletion.** A deletion request removes the tenant's material and its encryption
  keys, so any residual ciphertext is unrecoverable. Deletion is verified, not
  assumed.
- **Retention.** Retention windows are configuration, documented in the deployment
  guide, and applied uniformly. A cache entry never outlives the data it depends on
  when invalidation has run correctly.
- **Subprocessors.** The managed service's data subprocessors are listed, or
  explicitly recorded as none while no managed service operates, in
  [`docs/launch/04_TRUST_CENTER.md`](docs/launch/04_TRUST_CENTER.md). Self-hosted
  deployments run no subprocessors that the operator did not choose.

---

## 8. Review standards

A reviewer asks, in this order:

1. **Does it do what it says?** The change matches its description, and the tests
   exercise the actual behavior rather than a mock of it.
2. **What happens when it fails?** The failure path is defined, and it is safe.
3. **What does it enable an attacker to do?** Untrusted input, authorization, secrets,
   paths, outbound requests, and deserialization are examined explicitly.
4. **Does it make a claim it cannot support?** Numbers trace to artifacts; new
   artifacts are generated by scripts, not typed by hand.
5. **Can it be operated?** Logs, metrics, and a runbook entry exist for the new
   failure mode.
6. **Is it accessible?** New or changed interface surfaces meet the project's WCAG 2.2
   AA bar, verified by the automated a11y and keyboard audits.
7. **Is it smaller than it needs to be?** Unrelated changes are split out. A review is
   not a place to discover a second feature.

Reviewer time is a shared resource. A change that is not ready for review is returned
with the specific gate it has not met, not with a general request to improve.

---

## 9. Functional ownership

Ownership is defined by function, not by roster size. [`CODEOWNERS`](CODEOWNERS) maps
each area to the group accountable for it. Groups are durable responsibilities; the
people in them change.

| Function | Accountable for |
|---|---|
| Architecture | System boundaries, ADRs, cross-cutting invariants |
| Backend | Rust crates, gateway, cache, semantic engine, invalidation, persistence |
| Frontend | Control plane, accessibility, BFF security |
| SRE and platform | Deployment, capacity, observability, incident response |
| Security | Threat model, auth and crypto review, disclosure handling |
| Research | Evaluation datasets, calibration, benchmark reproducibility |
| Documentation | Evidence register, path integrity, consistency with artifacts |

A change with no accountable owner does not merge. Ownership transfers are recorded in
[`CODEOWNERS`](CODEOWNERS), not agreed verbally.

---

## 10. Amendments

This document changes by pull request like any other artifact. A governance change is
a C3 change: it requires review by at least one architecture reviewer and one security
reviewer, because governance is a security control. Amendments are dated in
[`CHANGELOG.md`](CHANGELOG.md).

Where this document and a runbook disagree, this document sets the rule and the
runbook is corrected. Where this document and [`docs/STATUS.md`](docs/STATUS.md)
disagree about current status, `docs/STATUS.md` wins, because status is evidence and
this document is policy.
