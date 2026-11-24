# Sunborn Engineering Scripts (`scripts/`)

Operational automation, CI/CD quality gates, distribution packaging, formal verification runners, and interactive demonstrations for Sunborn.

## Script Catalog

### 1. Launch & Verification Gates
- **`verify_public_launch.py`**: Executes the 8/8 comprehensive in-tree launch verification gates (doc paths, launch claims, OpenAPI drift, 6 pillars, Python SDK, CycloneDX SBOM, Grafana telemetry, K8s manifests).
- **`verify.sh` / `verify.ps1`**: Executes the full 12-gate build and test verification pipeline.
- **`check_repo_hygiene.py`**: Scans tracked files for scratch artifacts, temporary build caches, and forbidden secrets.
- **`check_launch_docs.py`**: Verifies link integrity across the launch package and enforces strict claim boundaries.
- **`verify_doc_paths.py`**: Scans documentation files to ensure all backticked repository paths exist on disk.
- **`check_openapi_drift.py`**: Validates OpenAPI 3.1 specification parity against registered Axum gateway routes.
- **`check_k8s_manifests.py`**: Audits Kubernetes manifests for secret isolation and security standards.
- **`check_runbook_endpoints.py`**: Validates alert runbook URLs against incident response headings.
- **`generate_sbom.py`**: Generates and checks deterministic CycloneDX v1.5 Software Bill of Materials (`sbom.cyclonedx.json`).
- **`verify_india_sovereign.py`**: Validates India DPDP Act 2023 and CERT-In sovereign compliance rules and currency formatting.
- **`test_semgrep_gate.py`**: Static analysis security gate enforcing security rules.

### 2. Desktop Packaging & Distribution
- **`package-distribution.ps1`**: Builds clean-machine Windows desktop distribution bundles with SHA-256 and SHA-512 checksums.
- **`install-sunborn.ps1` / `install-sunborn.sh`**: 1-click installer configuring the OS credential vault, persistent WAL directories, and desktop shortcuts.
- **`sunborn-desktop.ps1` / `sunborn-desktop.bat`**: 1-click launcher starting loopback gateway and opening the cockpit.
- **`uninstall-sunborn.ps1`**: Uninstalls desktop files, shortcuts, and registers removal in Windows Add/Remove Programs.
- **`verify-desktop-package.ps1`**: Validates desktop installation package integrity.

### 3. Cluster Operations & Local Runners
- **`docker_launch.py`**: Automated launcher with pre-flight port checks and health polling.
- **`start-all.ps1` / `start-all.sh`**: Bootstraps PostgreSQL, DragonflyDB, Qdrant, and Sunborn Gateway.
- **`dev-up.sh` & `dev-smoke.sh`**: Development stack bootstrap and smoke testing.
- **`run_tlaplus.sh` / `run_tlaplus.ps1`**: Automated runner executing all 6 TLA+ formal verification specifications.
- **`test_distributed_invalidation.sh`**: Containerized multi-replica invalidation harness testing all 6 distributed modes (`exact`, `semantic`, `replay`, `outbox`, `finops`, `crash`).
- **`test_python_sdk.sh`**: Sets up isolated virtual environment and runs Python SDK test suites.

### 4. Demos & Showcases
- **`client_showcase.py`**: Interactive terminal verification suite demonstrating all 6 architectural pillars.
- **`run_all_demos.py`**: Interactive runner executing the full suite of research demonstrations under `scripts/demos/`.

## Common Commands

```bash
# Verify repo hygiene (no scratch files or secrets)
python scripts/check_repo_hygiene.py

# Run in-tree public launch verification
python scripts/verify_public_launch.py --strict

# Run document link and claims check
python scripts/check_launch_docs.py

# Run CycloneDX SBOM check
python scripts/generate_sbom.py --check
```
