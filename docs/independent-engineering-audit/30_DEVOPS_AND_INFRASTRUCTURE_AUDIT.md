# 30 — DevOps & Infrastructure Audit

---

## 1. CI/CD

| Workflow | Real? | Gaps |
|---|---|---|
| `ci.yml` | YES — fmt, clippy -D warnings, cargo test, frontend build/test, SDK, openapi drift, helm | no `--locked`; no coverage; does not run `verify.sh`; `research/reproduce.sh` destructive |
| `security.yml` | PARTIAL — cargo audit (suppressed), trufflehog, npm audit --omit=dev, semgrep (deprecated action) | no container scan; skips dev CVEs |
| `release.yml` | YES buildx push + SBOM + provenance | no cosign, no SLSA attestation, outdated actions, no digest pinning |

`branch_protection.md` names status checks (`verify-ci / test-workspace`, …) that **do not exist** as job ids. Protection as documented is unsatisfiable.

## 2. Scripts

| Script | Verdict |
|---|---|
| `verify.sh` (11 stages) | mostly genuine; stage 11 cosmetic; prints "ALL PASSED" unconditionally |
| `verify.ps1` | diverges (stage 7 differs); same cosmetic ending |
| `check_openapi_drift.py` | genuine, best script |
| `check_k8s_manifests.py` | genuine but shallow (non-recursive; skips multi-region/helm) |
| `audit_licenses.py` | **cosmetic** — never reads a license; `PERMITTED_LICENSES` unused |
| `generate_sbom.py` | **non-deterministic** (random UUID/timestamp); no hashes/deps graph |
| `ci_load_test.py` | **no-op** — unreachable target prints success and exits 0; not wired anywhere |
| `verify_doc_paths.py` | genuine but scans only one doc; not in CI |
| `start-all.sh/.ps1` | dev convenience; `.sh` omits `SUNBORN_ADMIN_KEY`; no cleanup trap |
| `test_python_sdk.sh` | genuine; mutates tree; requires `uv` |

## 3. Containers

- Gateway: `rust:1.80-bookworm` builder → distroless nonroot, HEALTHCHECK, OCI labels. No `--locked`, no cargo cache mount.
- Frontend: `node:20-alpine` → `node:20-alpine`, runs `npm start` with full `node_modules` (no standalone output).
- compose: all ports bound `127.0.0.1` (good); healthchecks; **dev_insecure_* secrets**; no resource limits/caps/read-only.
- prod compose: fail-closed `${VAR:?}` (good); same missing hardening.

## 4. Kubernetes / Helm

- Helm: resources, runAsNonRoot, readOnlyRootFilesystem (gateway), drop ALL caps, HPA, PDB, NetworkPolicy; secret template **genuinely fails closed** on placeholders/reuse.
- Gaps: no podAntiAffinity/topologySpread (replicas can co-locate); no seccomp for frontend; frontend PDB absent; ServiceMonitor requires CRD.
- Raw k8s: `secrets.yaml` is comment-only and excluded from kustomize → **a fresh apply yields pods that cannot start**. Frontend `readOnlyRootFilesystem: false`. Image tag `v0.1.0` vs Helm/CI `0.1.0`.
- Multi-region: placeholder digests, fake `arn:aws:kms:...` ARNs, `.ai` hosts → **not runnable** (README says so).

## 5. Observability

- Prometheus counters are process-local atomics.
- No OTel exporter wired despite a spec doc.
- No dashboards/alert rules shipped.

## 6. Supply chain

- `audit.toml` ignores RUSTSEC-2026-0459 and RUSTSEC-2023-0071.
- No Dependabot/CodeQL/SECURITY.md.
- SBOM non-deterministic.

## 7. Verdict

CI mostly real; scripts partly cosmetic; containers/K8s unhardened; supply chain claims unsupported. DevOps score: **5/10**.
