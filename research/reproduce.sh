#!/usr/bin/env bash
# ==============================================================================
# Sunborn Empirical Reproducibility Package (Internal Empirical Evaluation)
# Executes complete test matrix, benchmarks, security validations, and verifies
# that empirical metrics match internal reproducibility standards.
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${WORKSPACE_ROOT}"

GIT_SHA=$(git rev-parse HEAD 2>/dev/null || echo "unknown")
GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo "unknown")
START_TIME=$(date -u +"%Y-%m-%dT%H:%M:%SZ")

echo "========================================================================"
echo "Sunborn Internal Empirical Reproducibility Suite"
echo "Git Commit  : ${GIT_SHA} (${GIT_BRANCH})"
echo "Timestamp   : ${START_TIME}"
echo "Environment : Linux $(uname -r) / Architecture: $(uname -m)"
echo "Host        : $(hostname 2>/dev/null || echo 'localhost')"
echo "========================================================================"

echo ""
echo "[1/5] Verifying Rust Workspace & Code Quality..."
cargo --version
rustc --version
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
echo "==> Code formatting and strict clippy checks PASSED."

echo ""
echo "[2/5] Running Core System Verification Test Suites..."
cargo test --workspace --all-targets
python3 -m unittest sdk/python/test_sunborn_agent.py
if command -v npm >/dev/null 2>&1; then
    npm test --prefix frontend
fi
echo "==> All unit, integration, and contract tests PASSED."

echo ""
echo "[3/5] Executing Live Gateway Performance & Latency Verification..."
PERF_LOG=$(mktemp)
cargo test --test performance_test -- --nocapture | tee "${PERF_LOG}"
echo "------------------------------------------------------------------------"
echo "Empirical Performance on Current Host Environment:"
grep -E "Throughput|Latency P50|Latency P95|Latency P99" "${PERF_LOG}" || true
echo "------------------------------------------------------------------------"

# Strict numerical assertion on gateway latency SLO (< 5.0ms P99)
GATEWAY_P99_US=$(grep -oE "Latency P99:\s+[0-9]+" "${PERF_LOG}" | awk '{print $3}' || echo "0")
rm -f "${PERF_LOG}"

if [[ -z "${GATEWAY_P99_US}" || "${GATEWAY_P99_US}" -eq 0 ]]; then
    echo "ERROR: Failed to extract P99 latency from performance test output" >&2
    exit 1
fi

if [[ "${GATEWAY_P99_US}" -ge 5000 ]]; then
    echo "ERROR: Gateway latency SLO breached! P99 is ${GATEWAY_P99_US} µs (must be < 5000 µs / 5.0 ms)" >&2
    exit 1
fi

echo "==> Local gateway latency SLO verified: P99 is ${GATEWAY_P99_US} µs (< 5000 µs / 5.0 ms)."

echo ""
echo "[4/5] Executing Security, Chaos & Governance Compliance Tests..."
cargo test --test security_test -- --nocapture
cargo test --test chaos_test -- --nocapture
cargo test --test governance_test -- --nocapture
cargo test --test disaster_recovery_test -- --nocapture
cargo test --test simulation_test -- --nocapture
cargo test --test learning_controller_test -- --nocapture
echo "==> Zero cross-tenant leakage, tenant spoofing prevention, circuit-breaker fallback, DST Non-Accelerator Authority, and 5-layer learning controller verified."

PYTHON_BIN="python3"
if [[ -x "${WORKSPACE_ROOT}/.venv/bin/python" ]]; then
    PYTHON_BIN="${WORKSPACE_ROOT}/.venv/bin/python"
fi

echo ""
echo "[5/5] Executing Semantic Cache Benchmark & Empirical Artifact Verification..."
BENCHMARK_ARTIFACT="research/results/benchmark_run.json"
MANIFEST_ARTIFACT="research/results/run_manifest.json"
CERTIFICATE_ARTIFACT="research/results/reproduction_certificate.json"

# Remove any stale results before running to guarantee fresh execution evidence
rm -f "${BENCHMARK_ARTIFACT}" "${MANIFEST_ARTIFACT}" "${CERTIFICATE_ARTIFACT}"

${PYTHON_BIN} benchmarks/semantic_cache_bench.py
${PYTHON_BIN} benchmarks/capacity_soak.py
${PYTHON_BIN} benchmarks/semantic_cache_bench_t.py

if [[ ! -f "${BENCHMARK_ARTIFACT}" ]]; then
    echo "ERROR: Benchmark artifact was not generated at ${BENCHMARK_ARTIFACT}" >&2
    exit 1
fi

if [[ ! -f "${MANIFEST_ARTIFACT}" ]]; then
    echo "ERROR: Temporal manifest artifact was not generated at ${MANIFEST_ARTIFACT}" >&2
    exit 1
fi

echo "------------------------------------------------------------------------"
echo "Fresh Dynamic Verification from ${BENCHMARK_ARTIFACT} and ${MANIFEST_ARTIFACT}:"
${PYTHON_BIN} -c "
import json
import sys
import platform
from datetime import datetime, timezone

with open('${BENCHMARK_ARTIFACT}') as f:
    bench_data = json.load(f)

with open('${MANIFEST_ARTIFACT}') as f:
    manifest_data = json.load(f)

meta = bench_data.get('metadata', {})
baselines = bench_data.get('baselines', {})
b2 = baselines.get('B2_NaiveCosine')
b3 = baselines.get('B3_SunbornCalibrated')

t_baselines = manifest_data.get('baselines', {})
t_b2 = t_baselines.get('B2_NaiveCosine')
t_b3 = t_baselines.get('B3_SunbornTemporal')

if not b2 or not b3 or not t_b2 or not t_b3:
    print('ERROR: Missing required baselines in benchmark or manifest outputs', file=sys.stderr)
    sys.exit(1)

print(f\"  [Benchmark] Total Evaluation Queries : {meta.get('total_benchmark_queries', 'N/A')}\")
print(f\"  [Benchmark] Naive B2 False-Hit Rate  : {b2['false_hit_rate_pct']}% ({b2['false_hits']} false hits)\")
print(f\"  [Benchmark] Sunborn B3 False-Hit Rate: {b3['false_hit_rate_pct']}% ({b3['false_hits']} false hits)\")
print(f\"  [Benchmark] Sunborn B3 Precision     : {b3['precision_pct']}%\")
print(f\"  [Benchmark] Sunborn B3 Token Savings : {b3['cost_savings_pct']}%\")
print(f\"  [Temporal-T] Total Stateful Scenarios: {manifest_data.get('metadata', {}).get('total_temporal_scenarios', 'N/A')}\")
print(f\"  [Temporal-T] Naive B2 False-Hit Rate : {t_b2['false_hit_rate_pct']}% ({t_b2['false_hits']} false hits)\")
print(f\"  [Temporal-T] Sunborn B3 False-Hits   : {t_b3['false_hits']} ({t_b3['false_hit_rate_pct']}%)\")
print(f\"  [Temporal-T] Sunborn B3 Precision    : {t_b3['precision_pct']}%\")

# Strictly enforce empirical correctness invariants on both benchmark suites
if b3['false_hit_rate_pct'] != 0.0 or b3['false_hits'] != 0:
    print(f\"FATAL: Sunborn B3 false-hit rate invariant violated: {b3['false_hit_rate_pct']}%\", file=sys.stderr)
    sys.exit(1)

if b3['precision_pct'] != 100.0:
    print(f\"FATAL: Sunborn B3 precision must be exactly 100.0%: got {b3['precision_pct']}%\", file=sys.stderr)
    sys.exit(1)

if t_b3['false_hit_rate_pct'] != 0.0 or t_b3['false_hits'] != 0:
    print(f\"FATAL: SemanticCacheBench-T Sunborn B3 false-hit rate violated: {t_b3['false_hit_rate_pct']}%\", file=sys.stderr)
    sys.exit(1)

if t_b3['precision_pct'] != 100.0:
    print(f\"FATAL: SemanticCacheBench-T Sunborn B3 precision must be 100.0%: got {t_b3['precision_pct']}%\", file=sys.stderr)
    sys.exit(1)

if b2['false_hits'] <= 0 or t_b2['false_hits'] <= 0:
    print(f\"WARNING: Baseline B2 expected false hits on adversarial/temporal traps\", file=sys.stderr)

certificate = {
    'evaluation': 'Sunborn Internal Empirical Reproduction',
    'status': 'PASSED',
    'git_commit': '${GIT_SHA}',
    'git_branch': '${GIT_BRANCH}',
    'timestamp_utc': datetime.now(timezone.utc).isoformat(),
    'environment': {
        'os': platform.system(),
        'release': platform.release(),
        'arch': platform.machine(),
        'python': platform.python_version()
    },
    'empirical_metrics': {
        'sunborn_b3_false_hits': b3['false_hits'],
        'sunborn_b3_false_hit_rate_pct': b3['false_hit_rate_pct'],
        'sunborn_b3_precision_pct': b3['precision_pct'],
        'sunborn_b3_cost_savings_pct': b3['cost_savings_pct'],
        'sunborn_b3_temporal_false_hits': t_b3['false_hits'],
        'sunborn_b3_temporal_precision_pct': t_b3['precision_pct'],
        'naive_b2_false_hits': b2['false_hits'],
        'naive_b2_false_hit_rate_pct': b2['false_hit_rate_pct'],
        'naive_b2_temporal_false_hits': t_b2['false_hits'],
        'naive_b2_temporal_false_hit_rate_pct': t_b2['false_hit_rate_pct']
    }
}

with open('${CERTIFICATE_ARTIFACT}', 'w') as f_cert:
    json.dump(certificate, f_cert, indent=2)

print('  Verification Assertions  : PASSED (B3 zero false hits & 100% precision verified on static & temporal benchmarks)')
print(f\"  Certificate Generated    : ${CERTIFICATE_ARTIFACT}\")
"
echo "------------------------------------------------------------------------"
echo "==> SemanticCacheBench, SemanticCacheBench-T & Capacity Soak verification completed successfully."

echo ""
echo "========================================================================"
echo "REPRODUCIBILITY EVALUATION RESULT: VERIFIED & CONFIRMED"
echo "All 5 verification gates passed with fresh empirical evidence."
echo "Artifact Certificate: ${CERTIFICATE_ARTIFACT}"
echo "========================================================================"
