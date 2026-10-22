#!/usr/bin/env bash
# ==============================================================================
# Sunborn Engineering Excellence: Master System Verification Pipeline
# Runs all checks across Rust workspace, Next.js frontend, Python SDK, Helm,
# container stacks, and empirical research reproducibility.
# ==============================================================================
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "${ROOT_DIR}"

echo "========================================================================"
echo "Sunborn Platform Master System Verification"
echo "========================================================================"

echo "[1/8] Running Rust Workspace Tests & Clippy..."
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo test --workspace --all-targets

echo "[2/8] Running Frontend Test Suite..."
npm test --prefix frontend

echo "[3/8] Building Frontend Production Bundle..."
npm run build --prefix frontend

echo "[4/8] Running Python SDK Packaging & Clean Installation Verification..."
bash scripts/test_python_sdk.sh
PYTHONPATH=sdk/python python3 -m unittest sdk/python/test_sunborn_agent.py
PYTHONPATH=sdk/python python3 -m unittest sdk/python/tests/test_sdk_package.py

echo "[5/8] Validating OpenAPI 3.1 Specification Drift..."
python3 scripts/check_openapi_drift.py

echo "[6/8] Running Helm Chart Lint & Schema Validation..."
helm lint deploy/helm/sunborn --set secrets.existingSecret=ci-validation
helm template sunborn deploy/helm/sunborn --set secrets.existingSecret=sunborn-custom-secret > /dev/null

echo "[7/8] Running Multi-Container Live Stack Pre-Flight..."
bash deploy/verify_live_stack.sh

echo "[8/8] Verifying Research Reproducibility Gates..."
bash research/reproduce.sh

echo ""
echo "========================================================================"
echo "ALL CONFIGURED CHECKS COMPLETED; REVIEW THE RELEASE DOSSIER FOR OPEN EVIDENCE GATES"
echo "========================================================================"
