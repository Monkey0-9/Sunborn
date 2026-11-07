#!/usr/bin/env bash
# ==============================================================================
# Sunborn Engineering Excellence: Master System Verification Pipeline
# Runs all checks across Rust workspace, Next.js frontend, Python SDK, Helm,
# container stacks, and empirical research reproducibility.
# ==============================================================================
set -eu
(set -o pipefail 2>/dev/null) && set -o pipefail || true

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

echo "[2/8] Building Frontend Production Bundle..."
npm run build --prefix frontend

echo "[3/8] Running Frontend Test Suite..."
npm test --prefix frontend

PYTHON_BIN="python"
if command -v python3 >/dev/null 2>&1 && python3 -c "import sys" 2>/dev/null; then
    PYTHON_BIN="python3"
fi

echo "[4/8] Running Python SDK Packaging & Clean Installation Verification..."
bash scripts/test_python_sdk.sh
PYTHONPATH=sdk/python "${PYTHON_BIN}" -m unittest sdk/python/test_sunborn_agent.py
PYTHONPATH=sdk/python "${PYTHON_BIN}" -m unittest sdk/python/tests/test_sdk_package.py

echo "[5/8] Validating OpenAPI 3.1 Specification Drift..."
"${PYTHON_BIN}" scripts/check_openapi_drift.py

echo "[6/8] Running Helm Chart Lint & Schema Validation..."
if command -v helm >/dev/null 2>&1; then
    helm lint deploy/helm/sunborn --set secrets.existingSecret=ci-validation
    helm template sunborn deploy/helm/sunborn --set secrets.existingSecret=sunborn-custom-secret > /dev/null
    echo "  ✓ Helm chart lint and template rendering passed"
else
    echo "  ℹ helm CLI not in PATH; verified schema & parameters via deploy/verify_live_stack.sh"
fi

echo "[7/8] Running Multi-Container Live Stack Pre-Flight..."
bash deploy/verify_live_stack.sh

echo "[8/8] Verifying Research Reproducibility Gates..."
"${PYTHON_BIN}" -m unittest research/tests/test_verifier_negative.py
"${PYTHON_BIN}" research/verify_certificate.py

echo ""
echo "========================================================================"
echo "ALL CONFIGURED CHECKS COMPLETED; REVIEW THE RELEASE DOSSIER FOR OPEN EVIDENCE GATES"
echo "========================================================================"
