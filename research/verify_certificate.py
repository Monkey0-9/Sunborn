#!/usr/bin/env python3
"""
Sunborn Independent Certificate & Artifact Provenance Verifier
Verifies reproduction_certificate.json, benchmark_run.json, and run_manifest.json:
1. Canonical manifest digest verification against payload bytes.
2. Explicit attestation verification outcomes:
   - AUTHENTICATED: Signature verified using trusted secret key.
   - INTEGRITY_VERIFIED_UNAUTHENTICATED: Digest and artifact checks pass, with no signature claimed.
   - AUTHENTICATION_UNVERIFIED: Signature exists, but no verification key provided in environment.
   - INVALID: A required integrity, schema, live-evidence, or signature check failed.
3. Strict live gateway evidence gate:
   - Requires provenance_type == 'LIVE_GATEWAY_DECISION_PATH'
   - Requires all 124 expected queries present with execution_mode == 'live_gateway_rpc'
   - Asserts exact suite size and query uniqueness (zero duplicate or omitted query IDs).
4. Raw artifact SHA-256 hash matching against provenance references.
5. Git commit and tree hash provenance verification against repository state.
6. Empirical correctness invariants verification (zero false-hit rate, 100% precision).
"""

import os
import sys
import json
import hmac
import hashlib
import argparse
import subprocess

STATUS_AUTHENTICATED = "AUTHENTICATED"
STATUS_INTEGRITY_VERIFIED_UNAUTHENTICATED = "INTEGRITY_VERIFIED_UNAUTHENTICATED"
STATUS_AUTHENTICATION_UNVERIFIED = "AUTHENTICATION_UNVERIFIED"
STATUS_INVALID = "INVALID"

EXPECTED_BENCHMARK_SUITE_SIZE = 124

def get_expected_query_ids(dataset_path: str = None) -> set[str]:
    """Derives the expected query ID set from the versioned benchmark dataset."""
    if dataset_path is None:
        dataset_path = os.path.abspath(
            os.path.join(os.path.dirname(__file__), "..", "benchmarks", "semantic_cache_bench.py")
        )

    # 1. Deterministic AST extraction (dependency-free, fast, no external libraries)
    if os.path.exists(dataset_path):
        try:
            import ast
            with open(dataset_path, "r", encoding="utf-8") as f:
                tree = ast.parse(f.read())
            for node in tree.body:
                if isinstance(node, ast.Assign):
                    for target in node.targets:
                        if isinstance(target, ast.Name) and target.id == "BENCHMARK_SUITE":
                            ids = set()
                            if isinstance(node.value, ast.List):
                                for elt in node.value.elts:
                                    if isinstance(elt, ast.Dict):
                                        for k, v in zip(elt.keys, elt.values):
                                            if isinstance(k, ast.Constant) and k.value == "id":
                                                if isinstance(v, ast.Constant):
                                                    ids.add(v.value)
                            if ids:
                                return ids
        except Exception:
            pass

    # 2. Module import fallback
    try:
        sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))
        from benchmarks.semantic_cache_bench import BENCHMARK_SUITE
        return {item["id"] for item in BENCHMARK_SUITE}
    except Exception as e:
        raise RuntimeError(f"Could not derive expected query IDs from dataset at {dataset_path}: {e}")

def verify_reproduction_certificate(
    cert_path: str,
    bench_path: str,
    manifest_path: str,
    dataset_path: str = None,
    check_git: bool = True,
    require_live: bool = True,
    require_auth: bool = False,
) -> str:
    print("=" * 75)
    print("SUNBORN INDEPENDENT CERTIFICATE & PROVENANCE VERIFIER")
    print(f"Certificate : {cert_path}")
    print(f"Benchmark   : {bench_path}")
    print(f"Manifest    : {manifest_path}")
    print(f"Require Live: {require_live}")
    print(f"Require Auth: {require_auth}")
    print("=" * 75)

    if not os.path.exists(cert_path):
        print(f"FAIL: Certificate file not found: {cert_path}", file=sys.stderr)
        return STATUS_INVALID
    if not os.path.exists(bench_path):
        print(f"FAIL: Benchmark file not found: {bench_path}", file=sys.stderr)
        return STATUS_INVALID
    if not os.path.exists(manifest_path):
        print(f"FAIL: Run manifest file not found: {manifest_path}", file=sys.stderr)
        return STATUS_INVALID

    try:
        with open(cert_path, "r", encoding="utf-8") as f:
            loaded_cert = json.load(f)
        with open(bench_path, "r", encoding="utf-8") as f_b:
            bench_data = json.load(f_b)
        with open(manifest_path, "r", encoding="utf-8") as f_m:
            manifest_data = json.load(f_m)
    except Exception as e:
        print(f"FAIL: Failed to parse JSON artifacts: {e}", file=sys.stderr)
        return STATUS_INVALID

    # 1. Check required top-level fields
    for field in ["evaluation", "status", "git_commit", "git_tree", "provenance", "empirical_metrics", "attestation"]:
        if field not in loaded_cert:
            print(f"FAIL: Missing required field '{field}' in certificate", file=sys.stderr)
            return STATUS_INVALID

    attestation = loaded_cert.pop("attestation", None)
    if not isinstance(attestation, dict):
        print("FAIL: Attestation block is missing or not a dictionary", file=sys.stderr)
        return STATUS_INVALID

    canonical_manifest_digest = attestation.get("canonical_manifest_digest")
    hmac_sig = attestation.get("hmac_sha256_signature")

    # 2. Canonical digest verification
    canonical_bytes = json.dumps(loaded_cert, sort_keys=True).encode("utf-8")
    expected_digest = hashlib.sha256(canonical_bytes).hexdigest()

    if not canonical_manifest_digest or not hmac.compare_digest(canonical_manifest_digest, expected_digest):
        print(
            f"FAIL: Canonical digest mismatch!\n"
            f"  Expected: {expected_digest}\n"
            f"  Claimed : {canonical_manifest_digest}",
            file=sys.stderr
        )
        return STATUS_INVALID
    print("  [✓] Canonical manifest SHA-256 digest verified.")

    # 3. Raw artifact hash matching
    prov = loaded_cert.get("provenance", {})
    with open(bench_path, "rb") as f_b_raw:
        actual_bench_sha = hashlib.sha256(f_b_raw.read()).hexdigest()
    with open(manifest_path, "rb") as f_m_raw:
        actual_manifest_sha = hashlib.sha256(f_m_raw.read()).hexdigest()

    expected_bench_sha = prov.get("benchmark_run_sha256")
    expected_manifest_sha = prov.get("run_manifest_sha256")

    if not expected_bench_sha or not hmac.compare_digest(actual_bench_sha, expected_bench_sha):
        print(f"FAIL: Benchmark artifact SHA-256 mismatch!\n  Actual  : {actual_bench_sha}\n  Expected: {expected_bench_sha}", file=sys.stderr)
        return STATUS_INVALID
    print(f"  [✓] Benchmark artifact SHA-256 matches provenance: {actual_bench_sha[:16]}...")

    if not expected_manifest_sha or not hmac.compare_digest(actual_manifest_sha, expected_manifest_sha):
        print(f"FAIL: Run manifest artifact SHA-256 mismatch!\n  Actual  : {actual_manifest_sha}\n  Expected: {expected_manifest_sha}", file=sys.stderr)
        return STATUS_INVALID
    print(f"  [✓] Run manifest artifact SHA-256 matches provenance: {actual_manifest_sha[:16]}...")

    # 4. Strict Live Gateway Evidence Gate & Query Completeness
    if require_live:
        bench_meta = bench_data.get("metadata", {})
        prov_type = bench_meta.get("provenance_type")
        if prov_type != "LIVE_GATEWAY_DECISION_PATH":
            print(
                f"FAIL: Strict live evidence required, but benchmark provenance_type is '{prov_type}' "
                f"(must be 'LIVE_GATEWAY_DECISION_PATH'). Offline simulation does not satisfy live evidence gate.",
                file=sys.stderr
            )
            return STATUS_INVALID

        expected_ids = get_expected_query_ids(dataset_path)
        if len(expected_ids) != EXPECTED_BENCHMARK_SUITE_SIZE:
            print(
                f"FAIL: Versioned benchmark dataset defines {len(expected_ids)} queries, expected {EXPECTED_BENCHMARK_SUITE_SIZE}",
                file=sys.stderr
            )
            return STATUS_INVALID

        traces = bench_data.get("b3_evaluation_trace", [])
        if len(traces) != EXPECTED_BENCHMARK_SUITE_SIZE:
            print(
                f"FAIL: Live evidence required exactly {EXPECTED_BENCHMARK_SUITE_SIZE} evaluation traces, but found {len(traces)}",
                file=sys.stderr
            )
            return STATUS_INVALID

        seen_ids = set()
        duplicate_ids = []
        for idx, tr in enumerate(traces):
            qid = tr.get("id")
            if not qid:
                print(f"FAIL: Trace entry at index {idx} is missing 'id'", file=sys.stderr)
                return STATUS_INVALID
            if qid in seen_ids:
                duplicate_ids.append(qid)
            seen_ids.add(qid)

            mode = tr.get("execution_mode")
            if mode != "live_gateway_rpc":
                print(
                    f"FAIL: Query '{qid}' has execution_mode '{mode}' "
                    f"(all {EXPECTED_BENCHMARK_SUITE_SIZE} queries must have execution_mode == 'live_gateway_rpc').",
                    file=sys.stderr
                )
                return STATUS_INVALID

        if duplicate_ids:
            print(f"FAIL: Duplicate query IDs found in evaluation trace: {sorted(duplicate_ids)}", file=sys.stderr)
            return STATUS_INVALID

        missing_ids = expected_ids - seen_ids
        unexpected_ids = seen_ids - expected_ids
        if missing_ids or unexpected_ids:
            print(
                f"FAIL: Query ID mismatch against expected versioned benchmark dataset!\n"
                f"  Missing IDs ({len(missing_ids)}): {sorted(list(missing_ids))[:10]}\n"
                f"  Unexpected IDs ({len(unexpected_ids)}): {sorted(list(unexpected_ids))[:10]}",
                file=sys.stderr
            )
            return STATUS_INVALID

        print(f"  [✓] Query completeness verified: exact 124-query ID set matches versioned dataset (zero missing, unexpected, or duplicate IDs).")
        print(f"  [✓] Live gateway decision path verified: all 124 queries executed via live_gateway_rpc.")

    # 5. Git provenance checks
    if check_git:
        try:
            curr_commit = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
            cert_commit = loaded_cert.get("git_commit")
            cert_tree = loaded_cert.get("git_tree")

            if not cert_commit or not cert_tree:
                print("FAIL: Certificate is missing 'git_commit' or 'git_tree' fields!", file=sys.stderr)
                return STATUS_INVALID

            # Verify that cert_commit matches HEAD or is a direct ancestor of HEAD
            is_valid_commit = False
            if cert_commit == curr_commit:
                is_valid_commit = True
            else:
                ancestor_check = subprocess.run(
                    ["git", "merge-base", "--is-ancestor", cert_commit, "HEAD"],
                    capture_output=True
                )
                if ancestor_check.returncode == 0:
                    is_valid_commit = True

            if not is_valid_commit:
                print(f"FAIL: Certificate git_commit ({cert_commit}) is not present in repository history for HEAD ({curr_commit})!", file=sys.stderr)
                return STATUS_INVALID

            # Verify that cert_tree matches the actual git tree of cert_commit in the git object database
            tree_res = subprocess.run(["git", "rev-parse", f"{cert_commit}^{{tree}}"], capture_output=True, text=True)
            if tree_res.returncode != 0:
                print(f"FAIL: Failed to resolve tree object for certificate commit {cert_commit} in git database!", file=sys.stderr)
                return STATUS_INVALID
            actual_tree = tree_res.stdout.strip()

            if cert_tree != actual_tree:
                print(f"FAIL: Certificate git_tree ({cert_tree}) does not match git tree ({actual_tree}) of commit {cert_commit}!", file=sys.stderr)
                return STATUS_INVALID

            print(f"  [✓] Git commit ({cert_commit[:10]}) and tree ({cert_tree[:10]}) match verified repository revision.")
        except Exception as e:
            print(f"FAIL: Git provenance verification failed closed: {e}", file=sys.stderr)
            return STATUS_INVALID
    else:
        print("  [!] WARNING: Git repository provenance check bypassed via explicit --no-git flag (reduced verification scope: repository revision unverified).")

    # 6. Empirical Metrics Independent Recomputation & Cross-Verification
    # A. Recompute B3 and B2 metrics directly from benchmark_run.json raw traces
    bench_traces = bench_data.get("b3_evaluation_trace", [])
    if not bench_traces:
        print("FAIL: Benchmark artifact missing 'b3_evaluation_trace'", file=sys.stderr)
        return STATUS_INVALID

    # Validate that every trace contains necessary raw fields
    for idx, tr in enumerate(bench_traces):
        if "ground_truth" not in tr:
            print(f"FAIL: Trace entry at index {idx} ({tr.get('id', 'unknown')}) missing 'ground_truth' label", file=sys.stderr)
            return STATUS_INVALID
        if "b2_reused" not in tr:
            print(f"FAIL: Trace entry at index {idx} ({tr.get('id', 'unknown')}) missing 'b2_reused' field required for independent baseline recomputation", file=sys.stderr)
            return STATUS_INVALID

    derived_b3_tp = sum(1 for t in bench_traces if (t.get("b3_reused") if "b3_reused" in t else t.get("reused")) and t.get("ground_truth"))
    derived_b3_fp = sum(1 for t in bench_traces if (t.get("b3_reused") if "b3_reused" in t else t.get("reused")) and not t.get("ground_truth"))
    derived_b3_tn = sum(1 for t in bench_traces if not (t.get("b3_reused") if "b3_reused" in t else t.get("reused")) and not t.get("ground_truth"))
    derived_b3_fn = sum(1 for t in bench_traces if not (t.get("b3_reused") if "b3_reused" in t else t.get("reused")) and t.get("ground_truth"))
    derived_b3_total = len(bench_traces)

    derived_b3_prec = round((derived_b3_tp / (derived_b3_tp + derived_b3_fp) * 100.0) if (derived_b3_tp + derived_b3_fp) > 0 else 100.0, 2)
    derived_b3_fhr = round((derived_b3_fp / derived_b3_total * 100.0) if derived_b3_total > 0 else 0.0, 2)

    derived_b2_tp = sum(1 for t in bench_traces if t.get("b2_reused") and t.get("ground_truth"))
    derived_b2_fp = sum(1 for t in bench_traces if t.get("b2_reused") and not t.get("ground_truth"))
    derived_b2_tn = sum(1 for t in bench_traces if not t.get("b2_reused") and not t.get("ground_truth"))
    derived_b2_fn = sum(1 for t in bench_traces if not t.get("b2_reused") and t.get("ground_truth"))
    derived_b2_total = len(bench_traces)

    derived_b2_prec = round((derived_b2_tp / (derived_b2_tp + derived_b2_fp) * 100.0) if (derived_b2_tp + derived_b2_fp) > 0 else 100.0, 2)
    derived_b2_fhr = round((derived_b2_fp / derived_b2_total * 100.0) if derived_b2_total > 0 else 0.0, 2)

    # Recompute B3 cost savings from validated cost model inputs
    cost_model = bench_data.get("metadata", {}).get("cost_model", {})
    origin_call_usd = float(cost_model.get("origin_call_usd", 0.0))
    cache_lookup_usd = float(cost_model.get("cache_lookup_usd", 0.0))
    if origin_call_usd <= 0.0 or cache_lookup_usd < 0.0:
        print(f"FAIL: Invalid cost model inputs: origin_call_usd={origin_call_usd}, cache_lookup_usd={cache_lookup_usd}", file=sys.stderr)
        return STATUS_INVALID

    b0_cost = derived_b3_total * origin_call_usd
    b3_cost = sum(
        cache_lookup_usd if (t.get("b3_reused") if "b3_reused" in t else t.get("reused")) else origin_call_usd
        for t in bench_traces
    )
    derived_b3_cost_savings = round(max(0.0, (1.0 - (b3_cost / b0_cost)) * 100.0), 2)

    # B. Recompute temporal metrics directly from run_manifest.json raw traces
    manifest_traces = manifest_data.get("raw_execution_traces", [])
    if not manifest_traces:
        print("FAIL: Manifest artifact missing 'raw_execution_traces'", file=sys.stderr)
        return STATUS_INVALID

    for idx, tr in enumerate(manifest_traces):
        if "ground_truth" not in tr or "b2_reused" not in tr or "b3_reused" not in tr:
            print(f"FAIL: Manifest trace at index {idx} ({tr.get('id', 'unknown')}) missing required evaluation fields", file=sys.stderr)
            return STATUS_INVALID

    derived_tb3_tp = sum(1 for t in manifest_traces if t.get("b3_reused") and t.get("ground_truth"))
    derived_tb3_fp = sum(1 for t in manifest_traces if t.get("b3_reused") and not t.get("ground_truth"))
    derived_tb3_tn = sum(1 for t in manifest_traces if not t.get("b3_reused") and not t.get("ground_truth"))
    derived_tb3_fn = sum(1 for t in manifest_traces if not t.get("b3_reused") and t.get("ground_truth"))
    derived_tb3_total = len(manifest_traces)

    derived_tb3_prec = round((derived_tb3_tp / (derived_tb3_tp + derived_tb3_fp) * 100.0) if (derived_tb3_tp + derived_tb3_fp) > 0 else 100.0, 2)
    derived_tb3_fhr = round((derived_tb3_fp / derived_tb3_total * 100.0) if derived_tb3_total > 0 else 0.0, 2)

    derived_tb2_tp = sum(1 for t in manifest_traces if t.get("b2_reused") and t.get("ground_truth"))
    derived_tb2_fp = sum(1 for t in manifest_traces if t.get("b2_reused") and not t.get("ground_truth"))
    derived_tb2_tn = sum(1 for t in manifest_traces if not t.get("b2_reused") and not t.get("ground_truth"))
    derived_tb2_fn = sum(1 for t in manifest_traces if not t.get("b2_reused") and t.get("ground_truth"))
    derived_tb2_total = len(manifest_traces)

    derived_tb2_prec = round((derived_tb2_tp / (derived_tb2_tp + derived_tb2_fp) * 100.0) if (derived_tb2_tp + derived_tb2_fp) > 0 else 100.0, 2)
    derived_tb2_fhr = round((derived_tb2_fp / derived_tb2_total * 100.0) if derived_tb2_total > 0 else 0.0, 2)

    # C. Validate that Artifact Baseline Summaries agree with raw traces
    bench_baselines = bench_data.get("baselines", {})
    bench_b3 = bench_baselines.get("B3_SunbornCalibrated", {})
    bench_b2 = bench_baselines.get("B2_NaiveCosine", {})

    if bench_b3.get("false_hits") != derived_b3_fp:
        print(f"FAIL: Benchmark B3 summary false_hits ({bench_b3.get('false_hits')}) != derived ({derived_b3_fp})", file=sys.stderr)
        return STATUS_INVALID
    if round(float(bench_b3.get("false_hit_rate_pct", -1)), 2) != derived_b3_fhr:
        print(f"FAIL: Benchmark B3 summary false_hit_rate_pct ({bench_b3.get('false_hit_rate_pct')}) != derived ({derived_b3_fhr})", file=sys.stderr)
        return STATUS_INVALID
    if round(float(bench_b3.get("precision_pct", -1)), 2) != derived_b3_prec:
        print(f"FAIL: Benchmark B3 summary precision_pct ({bench_b3.get('precision_pct')}) != derived ({derived_b3_prec})", file=sys.stderr)
        return STATUS_INVALID
    if round(float(bench_b3.get("cost_savings_pct", -1)), 2) != derived_b3_cost_savings:
        print(f"FAIL: Benchmark B3 summary cost_savings_pct ({bench_b3.get('cost_savings_pct')}) != derived ({derived_b3_cost_savings})", file=sys.stderr)
        return STATUS_INVALID

    if bench_b2.get("false_hits") != derived_b2_fp:
        print(f"FAIL: Benchmark B2 summary false_hits ({bench_b2.get('false_hits')}) != derived ({derived_b2_fp})", file=sys.stderr)
        return STATUS_INVALID
    if round(float(bench_b2.get("false_hit_rate_pct", -1)), 2) != derived_b2_fhr:
        print(f"FAIL: Benchmark B2 summary false_hit_rate_pct ({bench_b2.get('false_hit_rate_pct')}) != derived ({derived_b2_fhr})", file=sys.stderr)
        return STATUS_INVALID

    manifest_baselines = manifest_data.get("baselines", {})
    manifest_tb3 = manifest_baselines.get("B3_SunbornTemporal", {})
    manifest_tb2 = manifest_baselines.get("B2_NaiveCosine", {})

    if manifest_tb3.get("false_hits") != derived_tb3_fp:
        print(f"FAIL: Manifest B3 summary false_hits ({manifest_tb3.get('false_hits')}) != derived ({derived_tb3_fp})", file=sys.stderr)
        return STATUS_INVALID
    if round(float(manifest_tb3.get("precision_pct", -1)), 2) != derived_tb3_prec:
        print(f"FAIL: Manifest B3 summary precision_pct ({manifest_tb3.get('precision_pct')}) != derived ({derived_tb3_prec})", file=sys.stderr)
        return STATUS_INVALID

    if manifest_tb2.get("false_hits") != derived_tb2_fp:
        print(f"FAIL: Manifest B2 summary false_hits ({manifest_tb2.get('false_hits')}) != derived ({derived_tb2_fp})", file=sys.stderr)
        return STATUS_INVALID
    if round(float(manifest_tb2.get("false_hit_rate_pct", -1)), 2) != derived_tb2_fhr:
        print(f"FAIL: Manifest B2 summary false_hit_rate_pct ({manifest_tb2.get('false_hit_rate_pct')}) != derived ({derived_tb2_fhr})", file=sys.stderr)
        return STATUS_INVALID

    # Meaningful successful reuse assertion (not degenerate zero-reuse cache)
    if derived_b3_tp <= 0:
        print(f"FAIL: Degenerate cache result: Sunborn B3 produced 0 successful reuses (true positives = {derived_b3_tp}). Meaningful semantic reuse must be demonstrated.", file=sys.stderr)
        return STATUS_INVALID

    # Correctness Invariants: Zero false hits and 100% precision
    if derived_b3_fp != 0 or derived_b3_fhr != 0.0:
        print(f"FAIL: SemanticCacheBench B3 false-hit invariant violated: derived fp={derived_b3_fp}, rate={derived_b3_fhr}%", file=sys.stderr)
        return STATUS_INVALID
    if derived_b3_prec != 100.0:
        print(f"FAIL: SemanticCacheBench B3 precision invariant violated: derived prec={derived_b3_prec}%", file=sys.stderr)
        return STATUS_INVALID
    if derived_tb3_fp != 0:
        print(f"FAIL: SemanticCacheBench-T B3 temporal false-hit invariant violated: derived fp={derived_tb3_fp}", file=sys.stderr)
        return STATUS_INVALID
    if derived_tb3_prec != 100.0:
        print(f"FAIL: SemanticCacheBench-T B3 temporal precision invariant violated: derived prec={derived_tb3_prec}%", file=sys.stderr)
        return STATUS_INVALID

    # D. Cross-check certificate metrics against recomputed metrics (EXACT EQUALITY, NO TOLERANCE)
    cert_metrics = loaded_cert.get("empirical_metrics", {})
    expected_cert_mappings = [
        ("sunborn_b3_false_hits", derived_b3_fp, False),
        ("sunborn_b3_false_hit_rate_pct", derived_b3_fhr, True),
        ("sunborn_b3_precision_pct", derived_b3_prec, True),
        ("sunborn_b3_cost_savings_pct", derived_b3_cost_savings, True),
        ("sunborn_b3_temporal_false_hits", derived_tb3_fp, False),
        ("sunborn_b3_temporal_precision_pct", derived_tb3_prec, True),
        ("naive_b2_false_hits", derived_b2_fp, False),
        ("naive_b2_false_hit_rate_pct", derived_b2_fhr, True),
        ("naive_b2_temporal_false_hits", derived_tb2_fp, False),
        ("naive_b2_temporal_false_hit_rate_pct", derived_tb2_fhr, True),
    ]

    for key, expected_val, is_float in expected_cert_mappings:
        if key not in cert_metrics:
            print(f"FAIL: Certificate is missing empirical metric '{key}'", file=sys.stderr)
            return STATUS_INVALID
        cert_val = cert_metrics[key]
        if is_float:
            if round(float(cert_val), 2) != round(float(expected_val), 2):
                print(f"FAIL: Exact metric mismatch for '{key}': certificate claimed {cert_val}, but recomputed from source artifact is {expected_val}", file=sys.stderr)
                return STATUS_INVALID
        else:
            if int(cert_val) != int(expected_val):
                print(f"FAIL: Exact metric mismatch for '{key}': certificate claimed {cert_val}, but recomputed from source artifact is {expected_val}", file=sys.stderr)
                return STATUS_INVALID

    print(f"  [✓] Empirical metrics recomputed from raw traces: B3 TP={derived_b3_tp}, FP={derived_b3_fp}, Prec={derived_b3_prec}%, Savings={derived_b3_cost_savings}%; Naive B2 FP={derived_b2_fp} ({derived_b2_fhr}%).")
    print(f"  [✓] Artifact baseline summaries verified identical to raw trace recomputations.")
    print(f"  [✓] All 10 certificate empirical metrics cross-checked against independently recomputed metrics with exact equality.")

    # 7. Attestation & Signature Verification (Explicit 4-State Outcome Evaluation)
    signing_algo = prov.get("signing_algorithm")
    attestation_type = prov.get("attestation_type")
    secret_key = os.environ.get("SUNBORN_ATTESTATION_KEY") or os.environ.get("SUNBORN_REPRODUCTION_SECRET")

    outcome = STATUS_INVALID

    if hmac_sig is not None:
        if secret_key:
            expected_sig = hmac.new(secret_key.encode("utf-8"), canonical_bytes, hashlib.sha256).hexdigest()
            if hmac.compare_digest(hmac_sig, expected_sig):
                print("  [✓] HMAC-SHA-256 signature verified with trusted key.")
                outcome = STATUS_AUTHENTICATED
            else:
                print("FAIL: HMAC-SHA-256 signature verification failed (key mismatch or tampered signature)!", file=sys.stderr)
                outcome = STATUS_INVALID
        else:
            if require_auth:
                print("FAIL: Authenticated signature required (--require-auth), but no verification key provided in environment.", file=sys.stderr)
                outcome = STATUS_INVALID
            else:
                print("  [!] Signature exists in certificate, but no trusted verification key was provided in environment.")
                print("      (Canonical digest integrity verified; independently authenticated signature requires key).")
                outcome = STATUS_AUTHENTICATION_UNVERIFIED
    else:
        # No HMAC signature claimed
        if signing_algo != "NONE_UNAUTHENTICATED_LOCAL_DIGEST":
            print(f"FAIL: Unsigned certificate must declare signing_algorithm 'NONE_UNAUTHENTICATED_LOCAL_DIGEST', got '{signing_algo}'", file=sys.stderr)
            return STATUS_INVALID
        if attestation_type != "LOCAL_DEVELOPMENT_INTEGRITY_CHECK":
            print(f"FAIL: Unsigned certificate must declare attestation_type 'LOCAL_DEVELOPMENT_INTEGRITY_CHECK', got '{attestation_type}'", file=sys.stderr)
            return STATUS_INVALID
        if require_auth:
            print("FAIL: Authenticated signature required (--require-auth), but certificate is an unauthenticated local check.", file=sys.stderr)
            return STATUS_INVALID

        print("  [✓] Honest local development integrity check verified (unauthenticated local digest, no secret key claimed).")
        outcome = STATUS_INTEGRITY_VERIFIED_UNAUTHENTICATED

    print("=" * 75)
    print(f"CERTIFICATE VERIFICATION RESULT: {outcome}")
    print("=" * 75)
    return outcome

def main():
    parser = argparse.ArgumentParser(description="Verify Sunborn empirical reproduction certificate and artifacts.")
    parser.add_argument("--certificate", default="research/results/reproduction_certificate.json", help="Path to certificate")
    parser.add_argument("--benchmark", default="research/results/benchmark_run.json", help="Path to benchmark run artifact")
    parser.add_argument("--manifest", default="research/results/run_manifest.json", help="Path to temporal manifest artifact")
    parser.add_argument("--dataset", default=None, help="Path to versioned benchmark dataset script/source")
    parser.add_argument("--no-git", action="store_true", help="Skip git repository matching")
    parser.add_argument("--allow-offline", action="store_true", help="Allow offline simulation benchmark artifact (disables strict live gateway requirement)")
    parser.add_argument("--require-auth", action="store_true", help="Require independently authenticated HMAC signature with valid trusted key")
    args = parser.parse_args()

    outcome = verify_reproduction_certificate(
        cert_path=args.certificate,
        bench_path=args.benchmark,
        manifest_path=args.manifest,
        dataset_path=args.dataset,
        check_git=not args.no_git,
        require_live=not args.allow_offline,
        require_auth=args.require_auth,
    )

    if outcome == STATUS_AUTHENTICATED:
        sys.exit(0)
    elif outcome == STATUS_INTEGRITY_VERIFIED_UNAUTHENTICATED:
        sys.exit(0)
    elif outcome == STATUS_AUTHENTICATION_UNVERIFIED:
        # Signature is present but unverified without key
        sys.exit(2)
    else:
        # STATUS_INVALID
        sys.exit(1)

if __name__ == "__main__":
    main()
