# Sunborn Deep Research & R5 Standards Traceability — Phase 0–28

# Sunborn Deep Research, Standards Traceability, Novelty Gates and Phase 0–28 Revision

**Status:** Living Engineering & Research Decision Record. Concrete mappings from ISO/IEEE/NIST standards to source code and test suites are maintained in [Standards Traceability Matrix](file:///home/unknownuser/Sunbron/docs/architecture/standards_traceability_matrix.md). All verification gates are actively exercised via `research/reproduce.sh`.

## 1. Executive assessment

Sunborn's core problem is not “similarity search” alone. It is safe reuse of computation when correctness depends on request meaning, tenant/security context, model and tool versions, mutable knowledge, temporal state, policy, provenance, and the cost of recomputation. The existing 29-phase architecture covers the major systems, but a research-grade plan needs enforceable evidence, falsifiable novelty claims, clean baselines, independent reproduction, and a no-look-ahead protocol.

**Important standards distinction:** “IEEE/ACM Level 5” is not a single universal certification or publication grade. Sunborn will use an explicit internal target called **Research/Engineering Maturity R5** (defined below) and map applicable work to the exact standards, venue rules, and control objectives. Do not claim IEEE/ACM compliance, certification, novelty, or acceptance merely because a checklist exists.

## 2. Authoritative standards and frameworks to map

 1. **ISO/IEC/IEEE 29148:2018 — Requirements engineering** (current version confirmed by ISO in 2024): unique, verifiable requirements; lifecycle traceability; validation of requirements and related processes. [https://www.iso.org/standard/72089.html](<https://www.iso.org/standard/72089.html>)
 2. **IEEE 1012-2024 — System, Software, and Hardware Verification and Validation** (active; published 2025): lifecycle V&V, evidence, reviews, testing and integrity-level tailoring. [https://standards.ieee.org/ieee/1800.2/7324/](<https://standards.ieee.org/ieee/1800.2/7324/>)
 3. **ISO/IEC 25010:2023 — Product quality model**: use the model to structure quality requirements and acceptance measures rather than treating “quality” as one score. [https://www.iso.org/standard/78176.html](<https://www.iso.org/standard/78176.html>)
 4. **NIST AI RMF 1.0 and NIST AI 600-1 GenAI Profile**: govern/map/measure/manage risks across the AI lifecycle, including generative-AI-specific risks. These are frameworks, not product certifications. [https://www.nist.gov/itl/ai-risk-management-framework](<https://www.nist.gov/itl/ai-risk-management-framework>) and [https://doi.org/10.6028/NIST.AI.600-1](<https://doi.org/10.6028/NIST.AI.600-1>)
 5. **NIST SP 800-218 SSDF 1.1**: integrate secure development practices, vulnerability prevention and response into the lifecycle. [https://csrc.nist.gov/pubs/sp/800/218/final](<https://csrc.nist.gov/pubs/sp/800/218/final>)
 6. **OWASP GenAI / LLM Top 10**: use the current canonical release as a threat-input checklist for prompt injection, sensitive information disclosure, supply chain, data/model poisoning, improper output handling, excessive agency, vector/embedding weaknesses and unbounded consumption. Recheck the current version at implementation/release. [https://genai.owasp.org/resource/owasp-top-10-for-llm-applications-2025/](<https://genai.owasp.org/resource/owasp-top-10-for-llm-applications-2025/>) and [https://owasp.github.io/www-project-top-10-for-large-language-model-applications/](<https://owasp.github.io/www-project-top-10-for-large-language-model-applications/>)
 7. **OpenTelemetry semantic conventions**: standardize trace/metric/log naming and correlate cache decisions, retrieval, model/tool execution, invalidation and user-visible outcome. GenAI conventions evolve; pin a version and record compatibility. [https://opentelemetry.io/docs/specs/semconv/](<https://opentelemetry.io/docs/specs/semconv/>) and [https://github.com/open-telemetry/semantic-conventions-genai](<https://github.com/open-telemetry/semantic-conventions-genai>)
 8. **SLSA provenance**: preserve verifiable information describing where, when and how release artifacts were built. [https://slsa.dev/spec/v1.0/provenance](<https://slsa.dev/spec/v1.0/provenance>)
 9. **ACM Artifact Review and Badging**: build artifacts that reviewers can access and run, with complete instructions, source, data/tools/traces as permitted, environment capture and claims supported by reproducible artifacts. Venue-specific rules take precedence. [https://www.acm.org/publications/policies/artifact-review-and-badging-current](<https://www.acm.org/publications/policies/artifact-review-and-badging-current>)
10. **ACM MMSys artifact review guidance** (practical example, not a universal rule for every ACM venue): self-contained environments, accessible/runnable code, explicit testing instructions and reproducibility evaluation. [https://2026.acmmmsys.org/artifact-review-badging](<https://2026.acmmmsys.org/artifact-review-badging>)

Standards are copyrighted; this plan summarizes their purposes and links to official sources rather than reproducing normative clauses. The team must check the licensed full text and select applicable clauses/controls for the target context.

## 3. Define R5 operationally (internal target, not a standards label)

Sunborn reaches internal **Research/Engineering Maturity R5** only when all are true:

* Requirements and risks are versioned, uniquely identified, verifiable, and traced to design, implementation, tests, and acceptance evidence.
* Correctness is defined across C0 exact, C1 semantic, C2 freshness, C3 security, C4 tool/external state, C5 policy; unknown state fails conservatively.
* Every major technology has alternatives, workload-specific spike/kill test, measured decision, and ADR.
* Evaluation uses predeclared hypotheses, leakage-resistant splits, frozen baselines, repeated runs, uncertainty intervals, ablations, sensitivity analysis, and preserved negative/inconclusive results.
* Performance and savings claims include quality/safety guardrails and total-system cost, not hit rate alone.
* Adversarial security, tenant isolation, failure injection, rollback, DR and data deletion are tested end-to-end.
* Builds/artifacts have pinned dependencies, SBOM, provenance and traceable release identity.
* A clean-room reviewer can rerun the benchmark from documented, licensed artifacts; claim-to-evidence links and limitations are explicit.
* Independent review has no unresolved critical/high issue unless a named risk owner records an explicit exception.
* Release and paper decisions are evidence-based; no claim of external certification or publication acceptance is implied.

## 4. Research gap: treat as hypotheses to verify, not proven novelty

A literature review must search academic databases and proceedings (ACM DL, IEEE Xplore, USENIX, arXiv plus backward/forward citation chasing) and include semantic caching, LLM response caching, RAG freshness, cache invalidation, execution/DAG reuse, memoization, approximate nearest-neighbor retrieval, selective prediction/abstention, calibration, cache poisoning, workload/cost modeling and reproducible benchmark design. Search strings, date ranges, inclusion/exclusion criteria, deduplication, screening decisions and a related-work matrix must be saved. Do not say “no prior work” until this review is complete.

Candidate contribution hypotheses:

* **H1 — State-complete reuse identity:** include a typed, auditable compatibility vector for request, tenant/security, model/prompt/config, retriever/index, policy, tool permissions/external state and knowledge epoch. Test whether it reduces unsafe false reuse against simpler keys.
* **H2 — Risk-constrained utility decision:** optimize expected net utility subject to quality, freshness, policy and security constraints, with explicit abstain/revalidate/origin actions. Compare against fixed similarity thresholds and standard semantic-cache baselines.
* **H3 — Invalidation completeness and point-in-time replay:** use versioned dependencies/events and a point-in-time state snapshot to make stale reuse explainable and reproducible. Measure missed invalidations, propagation lag and unnecessary invalidation.
* **H4 — Partial execution reuse under typed dependency signatures:** reuse only pure/deterministic or proven-idempotent artifacts whose dependency, permission and external-state signatures match. Measure recomputation saved and correctness under mutations.
* **H5 — Joint quality/cost evaluation:** benchmark quality loss, false-hit risk, freshness, latency, compute/token cost, storage/index/event/telemetry overhead and cost per successful task under the same workloads.
* **H6 — Adversarial robustness and isolation:** test semantic collisions, cross-tenant confusion, prompt/document poisoning, policy changes and model/index migration as first-class benchmark dimensions.
* **H7 — Reproducibility and benchmark contribution:** publish a versioned workload/metric schema, configuration hashes, immutable raw results, and independently executable harness, subject to data/license/privacy constraints.

For every hypothesis: state null and alternative; define unit of analysis; primary metric and guardrails; baseline; dataset and splits; sample/seed plan; exclusions; statistical method; multiple-comparison handling where applicable; stopping rule; negative-result policy; artifact location. Separate exploratory from confirmatory results.

## 5. Non-negotiable technical invariants

 1. Authorization and tenant scope are established before exact or semantic lookup.
 2. Semantic similarity is candidate generation, never a proof of equivalence.
 3. Unknown/incomplete compatibility, freshness, policy or dependency state cannot produce an unqualified hit.
 4. All decision inputs are point-in-time snapshots; no future data, post-outcome labels, future invalidation events or test-set tuning may leak into a decision.
 5. Every hit has a machine-readable reason, compatibility evidence, state/version identity and trace correlation.
 6. Derived stores are rebuildable/reconcilable from authoritative state; cache outage must not change truth or authorization.
 7. Tool reuse is gated by purity/idempotency, arguments/schema, permission scope, side-effect status and external-state version.
 8. Invalidation is tested for commit/publish gaps, duplicates, reordering, replay, DLQ recovery and missed-event reconciliation.
 9. PII, secrets, private prompts and tenant data must not leak through cache keys, telemetry, benchmark exports or artifacts.
10. Cost accounting includes misses, refreshes, shadow execution, vector/graph/event/storage/observability and operational overhead.

## 6. Phase-by-phase strengthened exit contract

Each phase has three mandatory dimensions: (a) output artifact(s), (b) verification/evidence, (c) explicit gate decision. A phase is not complete because its code compiles or because a milestone exists. Thresholds are derived in Phase 0 from representative workloads and risk analysis; never invent them after observing results.

* **P0 Requirements/architecture:** requirements traceability matrix, architecture/context/trust-boundary diagrams, NFR/SLO and workload model, threat/failure models, consistency/tenancy model, research protocol, ADR template, risk register, V&V plan. No coding.
* **P1 Gateway foundation:** clean build, deterministic envelope, typed errors, API contract, CI and local bootstrap, cancellation/timeouts/backpressure tests.
* **P2 Identity/security:** deny-by-default authorization matrix, tenant context, quota/abuse controls, secret/PII redaction, threat-control-test mapping.
* **P3 Canonicalization:** versioned canonical representation, golden corpus, property/fuzz tests, false-equivalence measurement, migration semantics.
* **P4 Identity/versioning:** complete compatibility vector, collision/separation tests, point-in-time snapshot and migration/invalidation plan.
* **P5 Exact cache:** concurrency/stampede, TTL/clock skew, eviction, corruption/outage/fallback tests, latency/resource baseline.
* **P6 Semantic cache:** frozen evaluation set, precision/recall, false-hit and latency measurements, mandatory policy/tenant filters and migration tests.
* **P7 Decision engine:** explainable state machine, reason codes, calibrated uncertainty, abstention and origin fallback, leakage-resistant threshold selection.
* **P8 Admission:** quality/security/freshness/dependency/cost/retention rules, rejection reasons and poisoning/PII tests.
* **P9 Execution reuse:** typed DAG artifacts, side-effect safety, permission/external-state signatures, partial recomputation and replay tests.
* **P10 Dependencies:** completeness/orphan detection, update/delete impact tests, graph-vs-index benchmark and ADR.
* **P11 Invalidation:** transactional outbox, idempotency/order/replay/DLQ/reconciliation and measured lag.
* **P12 Freshness/consistency:** explicit states and budgets, clock skew/event lag tests, conservative UNKNOWN handling.
* **P13 Shadow evaluation:** paired fresh-vs-reused evaluation, sampling-bias/privacy controls, drift detection and response drill.
* **P14 Benchmark platform:** data cards, licensing, split/leakage checks, pinned runner, config/data hashes, immutable artifacts and clean-room instructions.
* **P15 Baselines:** B0–B6 same workloads, tuning budget, compute budget and metrics; frozen configs/results before confirmatory runs.
* **P16 Research:** predeclared hypotheses, ablations, uncertainty/statistical analysis, sensitivity, negative results and threats to validity.
* **P17 Security:** current threat catalog, exploit/fix/retest evidence, supply-chain scans, residual risk and exceptions.
* **P18 Tenancy:** adversarial isolation across caches, events, UI, APIs, logs/traces, exports and deletion; zero unauthorized cross-tenant disclosure in defined tests.
* **P19 Observability:** version-pinned OpenTelemetry conventions, SLOs, low-cardinality/redacted telemetry, decision explanations and measured telemetry overhead.
* **P20 FinOps:** auditable total-cost model, cost/success, break-even and sensitivity analysis; savings claim requires quality parity/guardrails.
* **P21 Reliability:** chaos matrix, bounded retries, fallback, recovery/reconciliation and retest of every material finding.
* **P22 Deployment:** pinned/signed artifacts, least privilege, probes, resource controls, canary/rollback, SBOM and provenance.
* **P23 DR:** measured RPO/RTO, backup integrity, restore/replay/rebuild and signed rehearsal.
* **P24 Performance:** representative P50/P95/P99/P99.9, throughput, resource and quality results with reproducible before/after comparison.
* **P25 Capacity:** load/soak/spike, saturation boundaries, growth/cost model, overload degradation and headroom rationale.
* **P26 Governance:** control/evidence matrix, data lifecycle/deletion across derived stores, policy/model inventory, audit and incident ownership.
* **P27 Release acceptance:** independent go/no-go, unresolved-risk list, V&V evidence, rollback/DR drill, provenance and reproducibility check.
* **P28 Research/publication:** venue-specific author checklist, complete related work, formal model, methods, claims-to-evidence matrix, limitations, artifact instructions and independent reproduction attempt.

## 7. Measurement protocol

Primary metrics must include: unsafe false-hit rate (with defined denominator), task/answer quality, freshness violations, unauthorized disclosure rate, invalidation propagation lag, cache decision latency, end-to-end latency, origin work avoided, total cost per successful task, and recovery outcomes. Report raw counts and denominators, confidence intervals, workload composition, model/provider versions, cache warm/cold state, concurrency, hardware/software environment, seeds, and all relevant exclusions. Do not optimize a single scalar without explicit safety constraints.

For statistical analysis, predefine primary outcomes and analysis before confirmatory runs; use paired comparisons where the same requests/workloads can be replayed; report effect sizes and uncertainty, not p-values alone; control multiple comparisons for broad ablation grids; distinguish practical significance from statistical significance. If data are dependent or clustered by tenant/session/source, choose methods that respect clustering. Preserve raw run-level data and scripts.

## 8. Technology selection rules

* **PostgreSQL** is authoritative for durable metadata and control-plane state.
* **Dragonfly** is an exact/ephemeral cache candidate; validate semantics, operational support and failure behavior.
* **Qdrant** is a semantic candidate index; it must remain derived and rebuildable.
* **Neo4j is not a foregone conclusion.** Benchmark it against PostgreSQL adjacency/recursive queries or a purpose-built indexed dependency table on realistic fan-out, update rate, traversal depth, memory, operational cost and recovery.
* **Redpanda/Kafka-compatible transport** is not the source of truth; use transactional outbox and reconciliation.
* **Object storage** holds immutable benchmark/replay artifacts with manifests and hashes.
* **Rust modular monolith first**; split services only when measured scaling, failure, security or deployment boundaries justify it.
* **Next.js/TypeScript frontend** is a control plane, not a correctness/security boundary. All sensitive mutations are authorized server-side.

## 9. Phase control and Linear rules

* Exactly one phase-gate issue per phase P0–P28, linked to its milestone and master root.
* Phase N gate blocks Phase N+1 gate; phase gate cannot close until its assigned deliverables and required evidence are reviewed.
* Phase gate links to key deliverables using explicit blockers; avoid a meaningless fully serial dependency graph for independent leaf work.
* Every leaf issue has one workstream parent, one phase milestone, a priority/owner role, testable acceptance criteria and evidence expectations.
* Do not create replacements for duplicate-marked issues; canonical issue remains source of truth.
* Cross-cutting workstream parent issues should not imply a single phase is their true execution window; leaf issues carry phase milestones.
* Milestone progress and issue status are not proof of deployment or standards conformity.
* Owners and dates must be set by actual project participants after capacity review; do not fabricate assignees or deadlines.

## 10. Research sources

* ISO/IEC/IEEE 29148:2018: [https://www.iso.org/standard/72089.html](<https://www.iso.org/standard/72089.html>)
* IEEE 1012-2024: [https://standards.ieee.org/ieee/1800.2/7324/](<https://standards.ieee.org/ieee/1800.2/7324/>)
* ISO/IEC 25010:2023: [https://www.iso.org/standard/78176.html](<https://www.iso.org/standard/78176.html>)
* NIST AI RMF: [https://www.nist.gov/itl/ai-risk-management-framework](<https://www.nist.gov/itl/ai-risk-management-framework>)
* NIST AI 600-1: [https://doi.org/10.6028/NIST.AI.600-1](<https://doi.org/10.6028/NIST.AI.600-1>)
* NIST SSDF SP 800-218: [https://csrc.nist.gov/pubs/sp/800/218/final](<https://csrc.nist.gov/pubs/sp/800/218/final>)
* OWASP GenAI Top 10: [https://owasp.github.io/www-project-top-10-for-large-language-model-applications/](<https://owasp.github.io/www-project-top-10-for-large-language-model-applications/>)
* OpenTelemetry semantic conventions: [https://opentelemetry.io/docs/specs/semconv/](<https://opentelemetry.io/docs/specs/semconv/>)
* OpenTelemetry GenAI conventions: [https://github.com/open-telemetry/semantic-conventions-genai](<https://github.com/open-telemetry/semantic-conventions-genai>)
* SLSA provenance: [https://slsa.dev/spec/v1.0/provenance](<https://slsa.dev/spec/v1.0/provenance>)
* ACM Artifact Review and Badging: [https://www.acm.org/publications/policies/artifact-review-and-badging-current](<https://www.acm.org/publications/policies/artifact-review-and-badging-current>)
* ACM MMSys 2026 artifact guidance: [https://2026.acmmmsys.org/artifact-review-badging](<https://2026.acmmmsys.org/artifact-review-badging>)

## 11. Explicit limitations

This synthesis does not establish novelty; a systematic literature review and experiments are required. It does not prove source code, tests, CI, deployments, controls, or evidence artifacts exist. It does not certify Sunborn against a standard. It does not guarantee IEEE/ACM acceptance. Venue, jurisdiction, data license, privacy, and institutional ethics requirements must be reviewed for the actual use case.
