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
The following automated status checks must pass green before merge authorization:
1. `verify-ci / test-workspace`: All 195+ Rust workspace tests pass (`cargo test --workspace`).
2. `verify-ci / frontend-test`: All 25+ Next.js unit tests pass (`npm test`).
3. `verify-ci / openapi-drift`: Zero API drift verified (`python scripts/check_openapi_drift.py`).
4. `verify-security / semgrep-sast`: Zero high/critical static analysis findings.
5. `verify-security / npm-audit`: Zero high/critical supply chain vulnerabilities in `frontend/package-lock.json`.
6. `verify-security / k8s-manifests`: Zero deployable placeholder secrets (`python scripts/check_k8s_manifests.py`).
7. `verify-research / cert-verification`: Research artifact hash verification passes (`python research/verify_certificate.py`).

---

## 2. Release Tagging & Attestation Workflow

* Production release candidates are tagged strictly in the format `vMAJOR.MINOR.PATCH`.
* Release triggers [`.github/workflows/release.yml`](workflows/release.yml) under the `release-approval` environment requiring formal manual sign-off.
* Container builds generate SLSA Level 3 provenance (`provenance: mode=max`) and CycloneDX SBOMs published directly to GitHub Container Registry (`ghcr.io`).
