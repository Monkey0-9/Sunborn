# Sunborn Branch Protection & Release Governance Specification

**Document Identifier**: `SUNBORN-GOV-BP-2026.1`  
**Classification**: Engineering Governance & Security Compliance Contract  
**Standard Compliance**: NIST SP 800-218 SSDF v1.1, CIS GitHub Foundations Benchmark v1.0  
**Target Protected Branch**: `main`  

---

## 1. Branch Ruleset Contract for `main`

The `main` branch represents the cryptographically attested, auditable production source of truth. Direct commits, unreviewed pull requests, force-pushes, and branch deletions are strictly forbidden.

### 1.1 Mandatory Pull Request Approvals
* **Minimum Approvals**: At least **two (2)** independent engineering approvals are required for all changes.
* **Code Owner Reviews**: Approval from matching specialist teams defined in [`.github/CODEOWNERS`](CODEOWNERS) is mandatory. A non-security engineer cannot authorize changes to `crates/sunborn-identity/` or cryptographic shredding handlers.
* **Dismiss Stale Approvals**: Any new commit pushed to an open pull request immediately invalidates previous approvals.
* **Require Review from Last Committer**: Author cannot approve their own pull request.

### 1.2 Cryptographic Commit Signing & Linear History
* **Signed Commits Only**: All commits must carry a valid GPG, SSH, or S/MIME digital signature verified by GitHub. Unsigned commits are rejected by branch rulesets.
* **Enforce Linear History**: Merge commits are disabled. Pull requests must be integrated via `Squash and merge` or `Rebase and merge`.

### 1.3 Mandatory Status Checks (CI/CD Gates)
The following status checks correspond to real jobs in
[`workflows/ci.yml`](workflows/ci.yml) and [`workflows/security.yml`](workflows/security.yml)
and must pass green before merge authorization:
1. `backend-verification`: `cargo fmt --check`, `clippy -D warnings`, and the full Rust suite (252 tests).
2. `frontend-verification`: TypeScript check, production build, and the frontend suite (37 tests).
3. `research-reproducibility`: SDK and research artifact verification.
4. `contract-and-sdk-verification`: OpenAPI drift (`python scripts/check_openapi_drift.py`) and Python SDK tests.
5. `helm-and-manifest-verification`: `helm lint`/`template` and Kubernetes manifest checks.
6. `security-audit`: `cargo audit`, secret scan, npm audit, and Semgrep.

> Note: earlier revisions of this document named checks that do not exist
> (`verify-ci / test-workspace`, etc.). Those names are corrected above.

---

## 2. Release Tagging & Attestation Workflow

* Production release candidates are tagged strictly in the format `vMAJOR.MINOR.PATCH`.
* Release triggers [`.github/workflows/release.yml`](workflows/release.yml) under the `release-approval` environment requiring formal manual sign-off.
* Container builds generate build provenance (`provenance: mode=max`) and CycloneDX SBOMs published to GitHub Container Registry (`ghcr.io`). **Image signing (cosign) and SLSA attestation are not yet implemented**; see the independent audit for status.
