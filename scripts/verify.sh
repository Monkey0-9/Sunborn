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

CARGO_BIN="cargo"
if ! command -v cargo >/dev/null 2>&1; then
    if command -v cargo.exe >/dev/null 2>&1; then
        CARGO_BIN="cargo.exe"
    elif [ -f "$HOME/.cargo/bin/cargo" ]; then
        CARGO_BIN="$HOME/.cargo/bin/cargo"
    elif [ -f "$HOME/.cargo/bin/cargo.exe" ]; then
        CARGO_BIN="$HOME/.cargo/bin/cargo.exe"
    else
        WIN_CARGO=$(find /mnt/c/Users -maxdepth 4 -name "cargo.exe" 2>/dev/null | head -n 1)
        if [ -n "${WIN_CARGO}" ]; then
            CARGO_BIN="${WIN_CARGO}"
        fi
    fi
fi

NPM_BIN="npm"
if ! command -v npm >/dev/null 2>&1; then
    if command -v npm.cmd >/dev/null 2>&1; then
        NPM_BIN="npm.cmd"
    fi
fi

PYTHON_BIN="python"
if command -v python3 >/dev/null 2>&1 && python3 -c "import sys" 2>/dev/null; then
    PYTHON_BIN="python3"
elif command -v python >/dev/null 2>&1 && python -c "import sys" 2>/dev/null; then
    PYTHON_BIN="python"
elif command -v python.exe >/dev/null 2>&1; then
    PYTHON_BIN="python.exe"
fi

echo "[1/12] Running Rust Workspace Tests & Clippy..."
"${CARGO_BIN}" check --workspace
"${CARGO_BIN}" clippy --workspace --all-targets -- -D warnings
"${CARGO_BIN}" fmt --check
"${CARGO_BIN}" test --workspace --all-targets

echo "[2/12] Building Frontend Production Bundle..."
"${NPM_BIN}" run build --prefix frontend

echo "[3/12] Running Frontend Test Suite..."
"${NPM_BIN}" test --prefix frontend

echo "[4/12] Running Python SDK Packaging & Clean Installation Verification..."
bash scripts/test_python_sdk.sh
PYTHONPATH=sdk/python "${PYTHON_BIN}" -m unittest sdk/python/test_sunborn_agent.py
PYTHONPATH=sdk/python "${PYTHON_BIN}" -m unittest sdk/python/tests/test_sdk_package.py

echo "[5/12] Validating OpenAPI 3.1 Specification Drift..."
"${PYTHON_BIN}" scripts/check_openapi_drift.py

echo "[6/12] Running Helm Chart Lint & Schema Validation..."
if command -v helm >/dev/null 2>&1; then
    helm lint deploy/helm/sunborn --set secrets.existingSecret=ci-validation
    helm template sunborn deploy/helm/sunborn --set secrets.existingSecret=sunborn-custom-secret > /dev/null
    echo "  ✓ Helm chart lint and template rendering passed"
else
    echo "  ℹ helm CLI not in PATH; verified schema & parameters via deploy/verify_live_stack.sh"
fi

echo "[7/12] Running Multi-Container Live Stack Pre-Flight..."
bash deploy/verify_live_stack.sh

echo "[8/12] Verifying Research Reproducibility Gates..."
"${PYTHON_BIN}" -m unittest research/tests/test_verifier_negative.py
"${PYTHON_BIN}" research/verify_certificate.py

echo "[9/12] Executing Adversarial Robustness & Linguistic Invariance Suite..."
"${PYTHON_BIN}" benchmarks/adversarial_evaluation_suite.py

echo "[10/12] Running Graph Storage & Causal Invalidation Policy Benchmark..."
"${PYTHON_BIN}" benchmarks/graph_storage_and_policy_bench.py

echo "[11/12] Generating deterministic CycloneDX SBOM..."
"${PYTHON_BIN}" scripts/generate_sbom.py

echo "[12/12] Strict Documentation & Attestation CLI Verification..."
"${PYTHON_BIN}" scripts/verify_doc_paths.py --strict
cargo run -p sunborn-verify -- --help > /dev/null

echo ""
echo "========================================================================"
echo "ALL 12/12 MASTER SYSTEM VERIFICATION STAGES COMPLETED"
echo "========================================================================"
