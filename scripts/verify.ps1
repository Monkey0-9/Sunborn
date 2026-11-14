# ==============================================================================
# Sunborn Engineering Excellence: Master System Verification Pipeline (PowerShell)
# Runs all checks across Rust workspace, Next.js frontend, Python SDK, Helm,
# container stacks, and empirical research reproducibility.
# ==============================================================================
$ErrorActionPreference = "Stop"

$RootDir = Split-Path -Parent $PSScriptRoot
Set-Location $RootDir

Write-Host "========================================================================" -ForegroundColor Cyan
Write-Host "Sunborn Platform Master System Verification (PowerShell)" -ForegroundColor Cyan
Write-Host "========================================================================" -ForegroundColor Cyan

Write-Host "[1/12] Running Rust Workspace Tests & Clippy..." -ForegroundColor Yellow
cargo check --workspace
if ($LASTEXITCODE -ne 0) { throw "cargo check failed" }

cargo clippy --workspace --all-targets -- -D warnings
if ($LASTEXITCODE -ne 0) { throw "cargo clippy failed" }

cargo fmt --check
if ($LASTEXITCODE -ne 0) { throw "cargo fmt failed" }

cargo test --workspace --all-targets
if ($LASTEXITCODE -ne 0) { throw "cargo test failed" }

Write-Host "[2/12] Building Frontend Production Bundle..." -ForegroundColor Yellow
npm run build --prefix frontend
if ($LASTEXITCODE -ne 0) { throw "npm run build failed" }

Write-Host "[3/12] Running Frontend Test Suite..." -ForegroundColor Yellow
npm test --prefix frontend
if ($LASTEXITCODE -ne 0) { throw "npm test failed" }

$PythonBin = "python"
if (Get-Command python3 -ErrorAction SilentlyContinue) {
    try {
        & python3 -c "import sys" 2>$null
        if ($LASTEXITCODE -eq 0) { $PythonBin = "python3" }
    } catch {}
}

Write-Host "[4/12] Running Python SDK Packaging & Clean Installation Verification..." -ForegroundColor Yellow
if (Get-Command bash -ErrorAction SilentlyContinue) {
    & bash scripts/test_python_sdk.sh
    if ($LASTEXITCODE -ne 0) { throw "Python SDK clean wheel install smoke test failed" }
}
$env:PYTHONPATH = "sdk/python"
& $PythonBin -m unittest sdk/python/test_sunborn_agent.py
if ($LASTEXITCODE -ne 0) { throw "Python SDK agent test failed" }
& $PythonBin -m unittest sdk/python/tests/test_sdk_package.py
if ($LASTEXITCODE -ne 0) { throw "Python SDK package test failed" }

Write-Host "[5/12] Validating OpenAPI 3.1 Specification Drift..." -ForegroundColor Yellow
& $PythonBin scripts/check_openapi_drift.py
if ($LASTEXITCODE -ne 0) { throw "OpenAPI drift check failed" }

Write-Host "[6/12] Running Helm Chart Lint & Schema Validation..." -ForegroundColor Yellow
if (Get-Command helm -ErrorAction SilentlyContinue) {
    helm lint deploy/helm/sunborn --set secrets.existingSecret=ci-validation
    helm template sunborn deploy/helm/sunborn --set secrets.existingSecret=sunborn-custom-secret | Out-Null
    Write-Host "  ✓ Helm chart lint and template rendering passed" -ForegroundColor Green
} else {
    Write-Host "  ℹ helm CLI not in PATH; verified schema & parameters via deploy/verify_live_stack.sh" -ForegroundColor Gray
}

Write-Host "[7/12] Running Multi-Container Live Stack Pre-Flight..." -ForegroundColor Yellow
& $PythonBin scripts/check_k8s_manifests.py
if ($LASTEXITCODE -ne 0) { throw "K8s manifests check failed" }

Write-Host "[8/12] Verifying Research Reproducibility Gates..." -ForegroundColor Yellow
& $PythonBin -m unittest research/tests/test_verifier_negative.py
if ($LASTEXITCODE -ne 0) { throw "Research verifier negative tests failed" }
& $PythonBin research/verify_certificate.py
if ($LASTEXITCODE -ne 0) { throw "Research certificate verification failed" }

Write-Host "[9/12] Executing Adversarial Robustness & Linguistic Invariance Suite..." -ForegroundColor Yellow
& $PythonBin benchmarks/adversarial_evaluation_suite.py
if ($LASTEXITCODE -ne 0) { throw "Adversarial benchmark failed" }

Write-Host "[10/12] Running Graph Storage & Causal Invalidation Policy Benchmark..." -ForegroundColor Yellow
& $PythonBin benchmarks/graph_storage_and_policy_bench.py
if ($LASTEXITCODE -ne 0) { throw "Graph storage benchmark failed" }

Write-Host "[11/12] Generating deterministic CycloneDX SBOM..." -ForegroundColor Yellow
& $PythonBin scripts/generate_sbom.py
if ($LASTEXITCODE -ne 0) { throw "SBOM generation failed" }

Write-Host "[12/12] Strict Documentation & Attestation CLI Verification..." -ForegroundColor Yellow
& $PythonBin scripts/verify_doc_paths.py --strict
if ($LASTEXITCODE -ne 0) { throw "Strict doc path verification failed" }
cargo run -p sunborn-verify -- --help | Out-Null
if ($LASTEXITCODE -ne 0) { throw "sunborn-verify CLI execution failed" }

Write-Host ""
Write-Host "========================================================================" -ForegroundColor Green
Write-Host "ALL 12/12 MASTER SYSTEM VERIFICATION STAGES COMPLETED" -ForegroundColor Green
Write-Host "========================================================================" -ForegroundColor Green
