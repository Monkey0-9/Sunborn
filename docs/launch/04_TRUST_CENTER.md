# Trust Center

**Owner:** Security, with Documentation
**Status:** self-hosted software today; no hosted service and no certification
**Authority for current status:** [`docs/STATUS.md`](../STATUS.md)

Part of the [launch package](README.md).

---

## 1. What this trust center covers

This page answers the questions a security reviewer, procurement team, or design
partner asks before sending real traffic:

- Where does data go, and what is stored?
- Who can read it, and under whose keys?
- What happens on deletion, mutation, and incident?
- Which third parties are involved?
- Which certifications exist, and which do not?

The short answer to the certification question: **none exist.** The project publishes
control mappings and self-assessments, not certificates. A certification mark will
appear here only after an accredited body issues one, and the certificate will be
linked.

---

## 2. Deployment scope

| Mode | Who operates it | Who is the data controller | Status |
|---|---|---|---|
| Self-hosted | The customer | The customer | Supported; this is the only mode in use today |
| Managed pilot | The project, for named design partners | Defined in the pilot agreement | Not a public service |
| Managed free tier | The project | Defined in the terms of service | Not launched; gated by [`ROADMAP.md`](../../ROADMAP.md) |

In a self-hosted deployment the project has no access to the deployment, its keys, its
logs, or its tenants. The operator hardening checklist in [`SECURITY.md`](../../SECURITY.md)
is mandatory, not advisory. In a managed deployment the project would be a processor;
that arrangement does not exist yet.

---

## 3. Data categories and flow

| Data | Where it lives | Encrypted at rest | Notes |
|---|---|---|---|
| Prompt and completion content | PostgreSQL and cache entries | Yes, AES-256-GCM envelope encryption under a per-tenant data key | The cache holds the answer and its attestation |
| Embeddings and vector points | Qdrant or the in-memory vector store | Depends on the operator's storage layer; not encrypted by the gateway | Point IDs are derived from tenant and prompt digest |
| Cache identity and exact keys | Memory and Dragonfly/Redis | Per deployment; transport security is operator-configured | Keys are tenant-namespaced |
| Causal lineage edges | In-memory graph and PostgreSQL | Per deployment; PostgreSQL lineage is deleted by the tenant cascade | Used for invalidation, not for analytics |
| API credentials | PostgreSQL | Key material is stored as a SHA-256 hash, not plaintext | Expiry and revocation are supported |
| Spend reservations and ledger | PostgreSQL | Per deployment | Used for quota and billing; no prompt content |
| Quota limits and usage | PostgreSQL | Per deployment | Tenant ID, window, and request count; enrollment is explicit; usage windows are pruned after 12 months |
| Request metadata and decisions | Logs and metrics | Per deployment | Decision path, tenant, identifiers; not prompt bodies by default |
| Audit events | PostgreSQL | Per deployment | Administrative actions and destructive operations |

Flow on a request: the gateway authenticates the key, resolves the tenant, checks the
model allowlist and quotas, computes a canonical cache identity, and either serves a
verified cache entry or calls the upstream provider. On a miss, the provider receives
the prompt. That is the most important disclosure on this page: **caching reduces how
often a prompt leaves the deployment, but it does not eliminate provider processing on
a miss or on an abstention.**

Provider retention, training, and residency terms are the provider's, not this
project's. A customer that cannot send a prompt to a provider has to configure the
gateway accordingly or not use that workload.

---

## 4. Data handling principles

### 4.1 Purpose limitation

Prompts and completions are processed to serve the request, operate the cache, and
enforce quotas. They are not used to train models. There is no secondary use in the
codebase, and a managed service would state the same limitation in its terms.

### 4.2 Minimization

- Logs record decisions and identifiers, not prompt bodies, unless an operator
  explicitly enables content logging for its own deployment.
- A redaction filter strips high-entropy tokens and payment-card patterns before log
  emission ([`crates/sunborn-identity/src/privacy.rs`](../../crates/sunborn-identity/src/privacy.rs)).
- Metrics are counters and histograms, not content.
- Spend records contain cost and tenant identifiers, not prompt text.

Redaction reduces risk; it is not a guarantee that no sensitive string can ever reach a
log. Operators handling regulated data should keep content logging off and restrict log
access.

### 4.3 Tenant isolation

- Cache keys are prefixed by tenant; vector searches filter on tenant metadata; point
  IDs are deterministic from tenant and prompt digest.
- The tenant is resolved from the verified credential, not from a caller-supplied
  header.
- Cross-tenant reads are treated as a critical defect under
  [`GOVERNANCE.md`](../../GOVERNANCE.md) section 7.

Isolation is enforced in code and tested adversarially. It is not a deployment
convention that an operator can disable with a flag.

### 4.4 Encryption

| Layer | Mechanism | Status |
|---|---|---|
| At rest | AES-256-GCM envelope encryption for prompt/completion content and durable cache records; per-tenant data key wrapped by a master key | Implemented; vector and lineage stores rely on tenant-scoped deletion plus operator storage encryption |
| Key derivation | HKDF-SHA256 with tenant-specific domain separation | Implemented |
| Key providers | AWS KMS, HashiCorp Vault Transit, local AEAD | Implemented; local AEAD is rejected for production readiness |
| In transit | Operator-terminated TLS in front of the gateway | Operator responsibility; the gateway does not terminate TLS itself |
| Key separation | Master KEK, admin key, certificate signing key, and learning HMAC must be distinct at startup | Implemented |
| Secret comparison | Constant-time comparison; memory zeroization | Implemented |

The gateway's own listener is HTTP. A deployment that exposes it without TLS in front
is misconfigured. The Helm chart and manifests are designed for an ingress or service
mesh to terminate TLS; that configuration is part of the deployment review.

### 4.5 Deletion and retention

Deletion is a tenant cascade with a verification pass, not a best-effort row delete:

- The tenant data key is destroyed, making envelope-encrypted records unreadable,
  including ciphertext in backups that still hold it.
- Tenant-namespaced cache entries, vector points, execution records, learning events,
  quota limits and counters, database rows, and API credentials are deleted explicitly,
  because vector points and lineage are not all envelope-encrypted by the gateway.
- A read-after-delete verification pass counts residual rows, cache keys, vector
  points, execution records, and learning events across stores. It returns
  `partial_failure` if a count is non-zero or a check errors, rather than assuming a
  successful API response means the data is gone.
- Cache entries also expire by TTL and eviction; invalidation removes entries whose
  dependencies mutated.
- Log and backup retention are deployment configuration. A backup that still holds a
  destroyed key is unreadable, but reads of other backup records and the backup's own
  expiry follow the operator's retention policy.

**What is not claimed:** instant erasure from every backup medium. Crypto-shredding
makes the data unreadable; it does not remove bytes from an immutable backup before
that backup expires. A regulated deployment must set backup retention accordingly.

---

## 5. Access control

| Control | Behavior |
|---|---|
| Authentication | API keys are hashed at rest, support expiry, and can be revoked individually or per tenant |
| Authorization | Every route is classified in an explicit access contract; adding an unclassified route fails the build |
| Admin separation | Administrative routes require an admin credential; tenant credentials are rejected |
| Dual control | Destructive operations require a distinct second admin key in production |
| Least privilege | Model allowlists and per-tenant quotas limit what a credential can spend and call |
| Audit | Administrative and destructive actions produce durable audit records |

The routes classified as public are `/health`, `/ready`, `/metrics`,
`/v1/auth/login`, and `/v1/ceremony/public-keys`. `/metrics` is public in the access
contract only so probes and monitoring can reach it; it must not be exposed on the
public internet, and the Helm verification gate asserts it is excluded from the public
ingress.

For a managed service, support access would be break-glass, time-bounded, logged, and
approved by the customer. That process is not implemented because no managed service
operates today.

---

## 6. Subprocessors

### 6.1 Self-hosted

The project engages no subprocessors for a self-hosted deployment. The operator
chooses its cloud, database, cache, vector store, model providers, and observability
vendors. Those are the operator's subprocessors, not the project's.

The reference stack uses PostgreSQL, DragonflyDB, Qdrant, and optional MinIO and
Redpanda containers ([`deploy/docker-compose.yml`](../../deploy/docker-compose.yml)).
For a managed deployment those would be infrastructure choices; for self-hosted they
are the operator's choices.

### 6.2 Managed service

No managed service operates today, so the subprocessor list is empty. Before any
managed traffic, this table must be published with, at minimum:

| Field | Why it is required |
|---|---|
| Legal entity and address | Controller/processor identification |
| Purpose | What the vendor does with the data |
| Data categories | Content, metadata, credentials, or billing |
| Processing region | Residency and transfer analysis |
| DPA status | Whether a data-processing agreement is in place |
| Security evidence | The vendor's report or certification relied upon |

The categories expected are cloud infrastructure, managed PostgreSQL, managed Redis,
managed vector search, model providers, observability, email, support tooling, and
billing. Vendor names will be added when contracts exist, not before.

---

## 7. Compliance status

This table is deliberately conservative. "Mapped" means the controls are described
against a framework. It does not mean certified, audited, or attested.

| Framework | Status | Evidence | What it does not mean |
|---|---|---|---|
| ISO/IEC 27001:2022 | Control-mapping self-assessment | [`docs/compliance/ISO_STANDARDS_COMPLIANCE.md`](../compliance/ISO_STANDARDS_COMPLIANCE.md) | No ISMS certificate and no accredited audit |
| ISO/IEC 27002:2022 | Control mapping | same | Not a certification |
| ISO/IEC/IEEE 29119 | Test-process mapping | same, plus [`scripts/verify.sh`](../../scripts/verify.sh) | Not an accredited test-process certification |
| NIST SP 800-218 (SSDF) | Practices mapped; SBOM and dependency audit in CI | [`sbom.cyclonedx.json`](../../sbom.cyclonedx.json), [`.github/workflows/ci.yml`](../../.github/workflows/ci.yml) | Not an SSDF attestation |
| OWASP ASVS 4.0 Level 3 | Internal control mapping and adversarial tests | [`docs/security/SECURITY_ASSESSMENT_AND_THREAT_MODEL.md`](../security/SECURITY_ASSESSMENT_AND_THREAT_MODEL.md) | Not an external ASVS assessment |
| OWASP API Top 10 | Route authorization contract, rate limits, input handling | [`crates/sunborn-gateway/tests/route_contract_parity_test.rs`](../../crates/sunborn-gateway/tests/route_contract_parity_test.rs) | Not a pen-test result |
| WCAG 2.2 AA | 10 control-plane routes; 0 serious/critical axe violations; keyboard audit | [`docs/STATUS.md`](../STATUS.md) section 1 | A control-plane audit, not a full product accessibility certification |
| SOC 2 Type I / II | Not obtained; not in progress | — | No report exists |
| HIPAA | Not claimed; no BAA | — | Do not send protected health information under a claim of compliance |
| PCI DSS | Out of scope | — | Do not send cardholder data |
| FedRAMP | Not pursued | — | No authorization |
| GDPR | Designed for erasure, minimization, and processor obligations | [`GOVERNANCE.md`](../../GOVERNANCE.md) section 7 | No public DPA for a hosted service because none exists |

The security posture is fail-closed by construction and reviewed internally. It has
not been tested by an external penetration-test firm; that engagement is a release
blocker in [`docs/STATUS.md`](../STATUS.md) section 3.

---

## 8. Security testing and assurance

| Evidence | What it shows | Limit |
|---|---|---|
| 328+ Rust tests, 63 frontend tests, 27 Python tests, 4 TypeScript tests | Regression safety across the workspace | Tests cover expected behavior, not unknown attack paths |
| Adversarial security tests | Cross-tenant isolation, injection, tampering, quorum | A test suite, not a red-team engagement |
| Route authorization parity | Every route classified; anonymous and non-admin callers rejected | Runtime configuration can still widen exposure if misconfigured |
| TLA+ models | Epoch fencing and outbox safety under bounded crashes | A model of the protocol, not the deployment |
| CycloneDX SBOM | 369 components, deterministic inventory | An inventory, not an absence of vulnerabilities |
| `cargo audit` in CI | Known advisories against pinned dependencies | Does not cover zero-days or runtime configuration |
| Held-out evaluation | 0 false hits on 200 hard negatives; 20% recall | Upper bound on false-hit rate is 3.39%, not 0.1% |

The full evidence register is [`docs/STATUS.md`](../STATUS.md). Where this page and
that register disagree, the register wins.

---

## 9. Incident response and disclosure

- **Reporting.** Vulnerabilities are reported privately through GitHub's private
  vulnerability reporting, as described in [`SECURITY.md`](../../SECURITY.md). Public
  issues must not contain reproduction steps or payloads.
- **Acknowledgement.** Reports are acknowledged as soon as possible. There is no paid
  bounty and no contractual response SLA today.
- **Containment.** On-call may contain an active incident without prior approval; the
  change is reviewed and documented immediately afterward.
- **Notification.** A managed service would notify affected tenants through the
  channel they configured, with impact and safe action first. For the pilot, the
  named contact in the pilot agreement is the channel. Statutory notification
  obligations apply where the operator is the controller.
- **Post-incident.** A blameless review records the timeline, contributing causes, the
  failed control, the regression test added, and the owner of residual risk.
- **Status page.** Not live. It is required before private beta, not before a pilot.

No public incident history is published. Pilot incidents are handled under the pilot
agreement; self-hosted operators own their incident record.

---

## 10. Claims this page will not make

| Not claimed | Why |
|---|---|
| "Certified" or "compliant" for any framework | No accredited certification exists |
| "Bank-grade", "military-grade", or "unhackable" | Adjectives are not evidence |
| "Zero data retention" | Prompts reach the upstream provider on a miss or abstention |
| "No third parties" | Model providers are third parties; self-hosted operators choose them |
| "Anonymous" or "untraceable" | Tenant identity is bound to a credential |
| "GDPR certified" | GDPR is a legal obligation, not a certificate |
| A false-hit rate of 0.1% or lower | The 99.9% upper bound is 3.39% |
| Uptime above the measured objective | No hosted uptime history exists |

If a sales document, landing page, or proposal conflicts with this table, this table
wins and the other document is corrected. Withdrawn claims are recorded in
[`CHANGELOG.md`](../../CHANGELOG.md) with the reason.

---

## 11. Reviewer checklist

A reviewer can verify the core claims without contacting the project:

- [ ] Read [`SECURITY.md`](../../SECURITY.md) for disclosure and known gaps.
- [ ] Read [`docs/security/SECURITY_ASSESSMENT_AND_THREAT_MODEL.md`](../security/SECURITY_ASSESSMENT_AND_THREAT_MODEL.md)
      for the STRIDE model and control mapping.
- [ ] Read [`docs/compliance/ISO_STANDARDS_COMPLIANCE.md`](../compliance/ISO_STANDARDS_COMPLIANCE.md)
      and confirm it says "self-assessment", not "certified".
- [ ] Run `bash scripts/verify.sh` and confirm the gates that were skipped are named.
- [ ] Run `python scripts/verify_doc_paths.py --strict`.
- [ ] Inspect [`sbom.cyclonedx.json`](../../sbom.cyclonedx.json) for the dependency
      inventory.
- [ ] Confirm the release verdict in [`docs/STATUS.md`](../STATUS.md) is still
      CONDITIONAL GO — controlled pilot.

Questions that require a contract, a signed report, or a vendor relationship are
answered by saying that the artifact does not exist yet. That is the intended answer,
not a failure of this page.
