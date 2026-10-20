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

echo "[1/7] Running Rust Workspace Tests & Clippy..."
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --check
cargo test --workspace --all-targets

echo "[2/7] Running Frontend Test Suite..."
npm test --prefix frontend

echo "[3/7] Building Frontend Production Bundle..."
npm run build --prefix frontend

echo "[4/7] Running Python SDK Test Suites..."
PYTHONPATH=sdk/python python3 -m unittest sdk/python/test_sunborn_agent.py
PYTHONPATH=sdk/python python3 -m unittest sdk/python/tests/test_sdk_package.py

echo "[5/7] Running Helm Chart Lint & Schema Validation..."
helm lint deploy/helm/sunborn

echo "[6/7] Running Multi-Container Live Stack Pre-Flight..."
bash deploy/verify_live_stack.sh

echo "[7/7] Verifying Research Reproducibility Gates..."
bash research/reproduce.sh

echo ""
echo "========================================================================"
echo "ALL 7 MASTER VERIFICATION PHASES PASSED WITH ZERO ERRORS"
echo "========================================================================"
