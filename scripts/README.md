# Sunborn Engineering Scripts (`scripts/`)

Operational automation, CI/CD gates, distribution packaging, demo runners, and repository health utilities.

## Key Scripts

- **`verify_public_launch.py`**: Executes the 8/8 comprehensive public launch gates.
- **`check_repo_hygiene.py`**: Scans tracked files for scratch artifacts, temporary build caches, and forbidden secrets.
- **`generate_sbom.py`**: Generates and checks deterministic CycloneDX v1.5 Software Bill of Materials (SBOM).
- **`package-distribution.ps1`**: Clean-machine Windows desktop distribution packager with SHA-256 and SHA-512 checksums.
- **`demos/`**: Standalone interactive Python demonstrations showcasing the 5 core research innovations.

## Verification

```bash
python scripts/check_repo_hygiene.py
python scripts/verify_public_launch.py
python scripts/generate_sbom.py --check
```
