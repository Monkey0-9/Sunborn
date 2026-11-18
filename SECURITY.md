# Security Policy

Sunborn is a security-sensitive component: it sits in the request path of production LLM traffic and holds
tenant credentials, cached model output, and encryption keys. We take reports seriously and we prefer to be
told about a problem early.

## Supported versions

| Version | Status | Notes |
| :--- | :--- | :--- |
| `1.0.x` | Security fixes accepted | Current release line. Release verdict is **CONDITIONAL GO — controlled pilot**; see [`docs/STATUS.md`](docs/STATUS.md). |
| `< 1.0` | Not supported | Pre-release candidates; upgrade before reporting. |

We do not maintain parallel security branches for older minors. Fixes land on `main` and are described in
[`CHANGELOG.md`](CHANGELOG.md).

## Reporting a vulnerability

**Do not open a public issue for a vulnerability.**

- Preferred: use GitHub's private vulnerability reporting on this repository
  (Security → Advisories → Report a vulnerability). This keeps the report, the discussion, and the fix
  private until a coordinated disclosure date is agreed.
- If private reporting is unavailable, open an issue that says only that you have a security report and how
  to reach you. Do not include reproduction steps, payloads, or affected endpoints in that issue.

Please include, where you can:

- Affected component and version (`crates/*`, `frontend/`, `sdk/python/`, `deploy/`, or the container image).
- A minimal reproduction, or the exact request sequence, with secrets redacted.
- Observed versus expected behaviour, and the impact you believe it has.
- Whether the issue requires a specific configuration (`SUNBORN_ENV`, backend selection, key setup).

## What to expect

We will acknowledge a report as soon as we can, tell you whether we can reproduce it, and agree a
disclosure timeline with you. We do **not** operate a paid bug bounty, and we cannot promise a response SLA.
If you need contractual response guarantees, that is a commercial arrangement — none exists publicly today.

We will credit reporters in the fix's changelog entry unless you ask us not to.

## Scope

**In scope**

- Gateway authentication, authorization, tenant isolation, and the route access contract
  (`crates/sunborn-gateway/`).
- Cache admission and reuse-safety logic, including certificate issuance and verification
  (`crates/sunborn-semantic/`, `crates/sunborn-identity/`, `crates/sunborn-execution/`).
- Cryptographic handling: envelope encryption, key derivation, constant-time comparison, zeroization.
- The Next.js BFF proxy and its origin pinning (`frontend/src/lib/proxy-policy.ts`).
- Invalidation and outbox correctness where it can leave stale or cross-tenant data readable
  (`crates/sunborn-invalidation/`, `crates/sunborn-db/`).
- Deployment manifests and secret rendering (`deploy/`).

**Out of scope**

- Vulnerabilities in upstream model providers, PostgreSQL, Redis/Dragonfly, Qdrant, or Kubernetes.
- Findings that require an already-compromised host, cluster-admin access, or a leaked master key.
- Missing hardening headers or TLS configuration on a deployment you control, unless the chart's defaults
  actively prevent a secure configuration.
- Rate-limit or cost-exhaustion behaviour that only applies with authentication disabled in `development`
  mode.

## Security model in brief

The design decisions a reviewer should check first:

- **Fail closed.** In `SUNBORN_ENV=production` the process refuses to start without its database, cache,
  vector store, and distinct keys, and returns HTTP 503 rather than serving with a degraded durable
  dependency. `development` mode is explicitly the only permissive mode.
- **Non-accelerator authority.** A model or accelerator cannot self-attest a cache admission. Admission is
  issued by the gateway after guardrails and carries a signed certificate; expired, tampered, or invalid
  certificates evict the entry and fall through to origin execution.
- **Key separation.** Master KEK, admin key, certificate signing key, and learning HMAC secret are verified
  distinct at startup.
- **Tenant isolation.** Cache keys, vector points, and graph lineage are namespaced by tenant and
  application; point IDs are derived from the tenant and prompt digest.
- **Constant-time comparison** for API keys and tokens (`subtle`), **memory zeroization** (`zeroize`),
  and **AES-256-GCM envelope encryption** under per-tenant data keys.
- **GDPR Art. 17 / CCPA erasure** by destroying the tenant data key, which renders stored records and
  embeddings unrecoverable without rewriting them.
- **Route-level authorization contract.** Every router path must be classified in an explicit access table;
  adding an endpoint without declaring its access model fails the build
  (`crates/sunborn-gateway/tests/route_contract_parity_test.rs`).

Full threat model and controls mapping:
[`docs/security/SECURITY_ASSESSMENT_AND_THREAT_MODEL.md`](docs/security/SECURITY_ASSESSMENT_AND_THREAT_MODEL.md) ·
[`docs/compliance/ISO_STANDARDS_COMPLIANCE.md`](docs/compliance/ISO_STANDARDS_COMPLIANCE.md).

## Known assurance gaps

We state these plainly so nobody mistakes the current state for a certified product:

- **No external penetration test has been completed.** The release verdict is gated on one
  ([`docs/STATUS.md`](docs/STATUS.md) §3).
- **The ISO/IEC and IEEE documents are control-mapping self-assessments**, not certifications by an
  accredited body.
- **The semantic reuse bound is not yet α ≤ 0.001.** With 200 hard negatives and zero observed false hits,
  the exact Clopper–Pearson 99.9% upper bound on the false-hit rate is 3.39%. Operate accordingly.

## Operator hardening checklist

If you deploy Sunborn, these are not optional:

1. Generate high-entropy keys; never reuse one key for two purposes. Provide them via a secret manager, not
   a values file.
2. Keep `/metrics` off the public internet.
3. Terminate TLS in front of the gateway and restrict ingress to the control plane.
4. Run PostgreSQL as the system of record, with backups of the WAL archive.
5. Set `SUNBORN_ENV=production` outside local evaluation — `development` disables the fail-closed checks.
6. Pin container images by digest; the SBOM is at [`sbom.cyclonedx.json`](sbom.cyclonedx.json).
