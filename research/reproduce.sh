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

SPAWNED_GATEWAY_PID=""
cleanup() {
    if [[ -n "${SPAWNED_GATEWAY_PID}" ]]; then
        kill "${SPAWNED_GATEWAY_PID}" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

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
    if [[ ! -f frontend/.next/BUILD_ID ]]; then
        echo "[*] Compiling Next.js production bundle for frontend test runner..."
        npm run build --prefix frontend
    fi
    npm test --prefix frontend
fi
echo "==> All unit, integration, and contract tests PASSED."

echo ""
echo "[3/5] Executing Live Gateway Performance & Latency Verification..."
PERF_LOG=$(mktemp)
cargo test --test performance_test -- --nocapture | tee "${PERF_LOG}"
echo "------------------------------------------------------------------------"
echo "Empirical Latency Decomposition (ACM/IEEE Discipline):"
echo "  [Table I]  In-Process CPU Engine Overhead (Zero-Network L1 Baseline):"
grep -A 5 "\[Table I\]" "${PERF_LOG}" | grep -E "Throughput|Latency P50|Latency P95|Latency P99" | sed 's/^/             /' || true
echo "  [Table II] Full Axum Gateway Stack (Auth, Routing, JCS Canonicalization, Cache):"
grep -A 8 "\[Table II\]" "${PERF_LOG}" | grep -E "Throughput|Latency P50|Latency P95|Latency P99" | sed 's/^/             /' || true
echo "------------------------------------------------------------------------"

# Strict numerical assertion on gateway latency SLO (< 5.0ms P99)
GATEWAY_P99_US=$(grep -A 8 "\[Table II\]" "${PERF_LOG}" | grep -oE "Latency P99:\s+[0-9]+" | awk '{print $3}' || echo "0")
rm -f "${PERF_LOG}"

if [[ -z "${GATEWAY_P99_US}" || "${GATEWAY_P99_US}" -eq 0 ]]; then
    echo "ERROR: Failed to extract P99 latency from performance test output" >&2
    exit 1
fi

if [[ "${GATEWAY_P99_US}" -ge 5000 ]]; then
    echo "ERROR: Gateway latency SLO breached! P99 is ${GATEWAY_P99_US} µs (must be < 5000 µs / 5.0 ms)" >&2
    exit 1
fi

echo "==> Gateway latency SLO verified: Full stack P99 is ${GATEWAY_P99_US} µs (< 5000 µs / 5.0 ms)."

echo ""
echo "[4/5] Executing Security, Chaos & Governance Compliance Tests..."
cargo test --test security_test -- --nocapture
cargo test --test chaos_test -- --nocapture
cargo test --test governance_test -- --nocapture
cargo test --test disaster_recovery_test -- --nocapture
cargo test --test simulation_test -- --nocapture
cargo test --test learning_controller_test -- --nocapture
cargo test --test production_startup_test -- --nocapture
cargo test --test adversarial_security_test -- --nocapture
echo "==> Zero cross-tenant leakage, tenant spoofing prevention, circuit-breaker fallback, DST Non-Accelerator Authority, production startup fail-closed, adversarial security, and 5-layer learning controller verified."

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

# Ensure live gateway is available for benchmark evaluation
SPAWNED_GATEWAY_PID=""
GATEWAY_HEALTHY=0
if curl -s http://127.0.0.1:8080/health >/dev/null 2>&1; then
    echo "[*] Sunborn Gateway already active and healthy at http://127.0.0.1:8080"
    GATEWAY_HEALTHY=1
else
    echo "[*] Launching Sunborn Gateway daemon for live HTTP benchmark evaluation..."
    cargo build --bin sunborn-gateway
    ./target/debug/sunborn-gateway >/dev/null 2>&1 &
    SPAWNED_GATEWAY_PID=$!
    for i in {1..50}; do
        if curl -s http://127.0.0.1:8080/health >/dev/null 2>&1; then
            GATEWAY_HEALTHY=1
            echo "[*] Live gateway ready at http://127.0.0.1:8080 (startup verified in ${i}00ms)"
            break
        fi
        sleep 0.1
    done
fi

if [[ "${GATEWAY_HEALTHY}" -ne 1 ]]; then
    echo "FATAL: Sunborn Gateway failed to become healthy at http://127.0.0.1:8080 within startup deadline (5.0s)." >&2
    exit 1
fi

echo "[*] Executing SemanticCacheBench with --require-live..."
export SUNBORN_REQUIRE_LIVE=1
${PYTHON_BIN} benchmarks/semantic_cache_bench.py --require-live
${PYTHON_BIN} benchmarks/capacity_soak.py
${PYTHON_BIN} benchmarks/semantic_cache_bench_t.py

if [[ -n "${SPAWNED_GATEWAY_PID}" ]]; then
    kill "${SPAWNED_GATEWAY_PID}" 2>/dev/null || true
    SPAWNED_GATEWAY_PID=""
fi

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
import os
import hmac
import hashlib
import platform
import subprocess
from datetime import datetime, timezone

with open('${BENCHMARK_ARTIFACT}', 'rb') as f:
    bench_bytes = f.read()
    bench_sha = hashlib.sha256(bench_bytes).hexdigest()
    bench_data = json.loads(bench_bytes.decode('utf-8'))

with open('${MANIFEST_ARTIFACT}', 'rb') as f:
    manifest_bytes = f.read()
    manifest_sha = hashlib.sha256(manifest_bytes).hexdigest()
    manifest_data = json.loads(manifest_bytes.decode('utf-8'))

rustc_ver = subprocess.run(['rustc', '--version'], capture_output=True, text=True).stdout.strip()
cargo_ver = subprocess.run(['cargo', '--version'], capture_output=True, text=True).stdout.strip()
git_tree = subprocess.run(['git', 'rev-parse', 'HEAD^{tree}'], capture_output=True, text=True).stdout.strip()

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
print(f\"  [Benchmark] Embedding Backend        : {meta.get('embedding_backend', 'N/A')}\")
print(f\"  [Benchmark] Decision Engine          : {meta.get('decision_engine', 'N/A')}\")
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

# Enforce Git revision matching
bench_meta = bench_data.get('metadata', {})
if bench_meta.get('git_commit') != '${GIT_SHA}' or bench_meta.get('git_tree') != git_tree:
    print(f\"FATAL: Benchmark artifact revision ({bench_meta.get('git_commit')}, {bench_meta.get('git_tree')}) does not match current git repository (${GIT_SHA}, {git_tree})\", file=sys.stderr)
    sys.exit(1)

# Enforce strict live gateway evidence contract
prov_type = bench_meta.get('provenance_type')
if prov_type != 'LIVE_GATEWAY_DECISION_PATH':
    print(f\"FATAL: Benchmark artifact provenance_type '{prov_type}' != 'LIVE_GATEWAY_DECISION_PATH' (offline simulation does not satisfy live evidence gate)\", file=sys.stderr)
    sys.exit(1)

traces = bench_data.get('b3_evaluation_trace', [])
if len(traces) != 124:
    print(f\"FATAL: Benchmark evaluation trace count {len(traces)} != expected suite size 124\", file=sys.stderr)
    sys.exit(1)

seen_ids = set()
for idx, tr in enumerate(traces):
    qid = tr.get('id')
    if not qid:
        print(f\"FATAL: Trace entry {idx} is missing 'id'\", file=sys.stderr)
        sys.exit(1)
    if qid in seen_ids:
        print(f\"FATAL: Duplicate query ID '{qid}' found in evaluation trace\", file=sys.stderr)
        sys.exit(1)
    seen_ids.add(qid)
    mode = tr.get('execution_mode')
    if mode != 'live_gateway_rpc':
        print(f\"FATAL: Trace query '{qid}' execution_mode '{mode}' != 'live_gateway_rpc'\", file=sys.stderr)
        sys.exit(1)

# Derive expected IDs from versioned dataset and verify exact set equality
import ast
expected_ids = set()
with open('benchmarks/semantic_cache_bench.py', 'r', encoding='utf-8') as f_ds:
    tree_ds = ast.parse(f_ds.read())
for node in tree_ds.body:
    if isinstance(node, ast.Assign):
        for target in node.targets:
            if isinstance(target, ast.Name) and target.id == 'BENCHMARK_SUITE':
                for elt in node.value.elts:
                    for k, v in zip(elt.keys, elt.values):
                        if isinstance(k, ast.Constant) and k.value == 'id':
                            expected_ids.add(v.value)

if len(expected_ids) != 124:
    print(f\"FATAL: Expected dataset size is {len(expected_ids)}, expected 124\", file=sys.stderr)
    sys.exit(1)

missing_ids = expected_ids - seen_ids
unexpected_ids = seen_ids - expected_ids
if missing_ids or unexpected_ids:
    print(f\"FATAL: Query ID set mismatch against expected dataset: missing={sorted(list(missing_ids))}, unexpected={sorted(list(unexpected_ids))}\", file=sys.stderr)
    sys.exit(1)

# Ensure meaningful semantic reuse (non-degenerate cache)
derived_b3_tp = sum(1 for t in traces if t.get('reused') and t.get('ground_truth'))
if derived_b3_tp <= 0:
    print(f\"FATAL: Degenerate cache result: Sunborn B3 produced {derived_b3_tp} true positive reuses (meaningful reuse required)\", file=sys.stderr)
    sys.exit(1)

secret_key = os.environ.get('SUNBORN_ATTESTATION_KEY') or os.environ.get('SUNBORN_REPRODUCTION_SECRET')
if secret_key:
    attestation_type = 'PRODUCTION_INDEPENDENT_ATTESTATION'
    signing_algo = 'HMAC-SHA-256'
else:
    attestation_type = 'LOCAL_DEVELOPMENT_INTEGRITY_CHECK'
    signing_algo = 'NONE_UNAUTHENTICATED_LOCAL_DIGEST'

attestation_payload = {
    'evaluation': 'Sunborn Empirical Scientific Reproduction',
    'status': 'PASSED',
    'git_commit': '${GIT_SHA}',
    'git_branch': '${GIT_BRANCH}',
    'git_tree': git_tree,
    'timestamp_utc': datetime.now(timezone.utc).isoformat(),
    'environment': {
        'os': platform.system(),
        'release': platform.release(),
        'arch': platform.machine(),
        'python': platform.python_version(),
        'rustc': rustc_ver,
        'cargo': cargo_ver
    },
    'provenance': {
        'attestation_type': attestation_type,
        'signing_algorithm': signing_algo,
        'benchmark_run_sha256': bench_sha,
        'run_manifest_sha256': manifest_sha,
        'embedding_backend': meta.get('embedding_backend', 'N/A'),
        'decision_engine': meta.get('decision_engine', 'N/A'),
        'decision_engine_mode': meta.get('decision_engine_mode', 'rust_parity_calibrated_decision_engine'),
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

canonical_bytes = json.dumps(attestation_payload, sort_keys=True).encode('utf-8')
canonical_digest = hashlib.sha256(canonical_bytes).hexdigest()

if secret_key:
    sig = hmac.new(secret_key.encode('utf-8'), canonical_bytes, hashlib.sha256).hexdigest()
else:
    sig = None

certificate = dict(attestation_payload)
certificate['attestation'] = {
    'canonical_manifest_digest': canonical_digest,
    'hmac_sha256_signature': sig
}

with open('${CERTIFICATE_ARTIFACT}', 'w') as f_cert:
    json.dump(certificate, f_cert, indent=2)

print(f\"  Certificate Generated    : ${CERTIFICATE_ARTIFACT} ({attestation_type})\")
"

echo "[*] Executing negative verification test suite (proving offline & invalid rejection)..."
${PYTHON_BIN} -m unittest research/tests/test_verifier_negative.py

echo "[*] Executing independent certificate and artifact verification..."
${PYTHON_BIN} research/verify_certificate.py \
    --certificate "${CERTIFICATE_ARTIFACT}" \
    --benchmark "${BENCHMARK_ARTIFACT}" \
    --manifest "${MANIFEST_ARTIFACT}"
echo "------------------------------------------------------------------------"
echo "==> SemanticCacheBench, SemanticCacheBench-T & Capacity Soak verification completed successfully."

echo ""
echo "========================================================================"
echo "REPRODUCIBILITY EVALUATION RESULT: VERIFIED & CONFIRMED"
echo "All 5 verification gates passed with fresh empirical evidence."
echo "Artifact Certificate: ${CERTIFICATE_ARTIFACT}"
echo "========================================================================"
