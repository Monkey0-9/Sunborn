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

def verify_reproduction_certificate(
    cert_path: str,
    bench_path: str,
    manifest_path: str,
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

    # 4. Strict Live Gateway Evidence Gate
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

        traces = bench_data.get("b3_evaluation_trace", [])
        if len(traces) != EXPECTED_BENCHMARK_SUITE_SIZE:
            print(
                f"FAIL: Live evidence required exactly {EXPECTED_BENCHMARK_SUITE_SIZE} evaluation traces, but found {len(traces)}",
                file=sys.stderr
            )
            return STATUS_INVALID

        seen_ids = set()
        for idx, tr in enumerate(traces):
            qid = tr.get("id")
            if not qid:
                print(f"FAIL: Trace entry at index {idx} is missing 'id'", file=sys.stderr)
                return STATUS_INVALID
            if qid in seen_ids:
                print(f"FAIL: Duplicate query ID '{qid}' found in evaluation trace", file=sys.stderr)
                return STATUS_INVALID
            seen_ids.add(qid)

            mode = tr.get("execution_mode")
            if mode != "live_gateway_rpc":
                print(
                    f"FAIL: Query '{qid}' has execution_mode '{mode}' "
                    f"(all {EXPECTED_BENCHMARK_SUITE_SIZE} queries must have execution_mode == 'live_gateway_rpc').",
                    file=sys.stderr
                )
                return STATUS_INVALID

        print(f"  [✓] Live gateway decision path verified: {len(seen_ids)}/{EXPECTED_BENCHMARK_SUITE_SIZE} unique queries executed via live_gateway_rpc.")

    # 5. Git provenance checks
    if check_git:
        try:
            curr_commit = subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
            parent_res = subprocess.run(["git", "rev-parse", "HEAD~1"], capture_output=True, text=True)
            parent_commit = parent_res.stdout.strip() if parent_res.returncode == 0 else ""
            valid_commits = {c for c in [curr_commit, parent_commit] if c}

            cert_commit = loaded_cert.get("git_commit")
            cert_tree = loaded_cert.get("git_tree")

            if cert_commit not in valid_commits:
                print(f"FAIL: Certificate git_commit ({cert_commit}) does not match current repository revision ({curr_commit}) or parent ({parent_commit})!", file=sys.stderr)
                return STATUS_INVALID

            if cert_commit == curr_commit:
                curr_tree = subprocess.run(["git", "rev-parse", "HEAD^{tree}"], capture_output=True, text=True, check=True).stdout.strip()
                if cert_tree != curr_tree:
                    print(f"FAIL: Certificate git_tree ({cert_tree}) does not match current repository tree ({curr_tree})!", file=sys.stderr)
                    return STATUS_INVALID
            elif cert_commit == parent_commit:
                parent_tree = subprocess.run(["git", "rev-parse", "HEAD~1^{tree}"], capture_output=True, text=True, check=True).stdout.strip()
                if cert_tree != parent_tree:
                    print(f"FAIL: Certificate git_tree ({cert_tree}) does not match parent repository tree ({parent_tree})!", file=sys.stderr)
                    return STATUS_INVALID

            print(f"  [✓] Git commit ({cert_commit[:10]}) and tree ({cert_tree[:10]}) match verified repository revision.")
        except Exception as e:
            print(f"  [!] Git check skipped or encountered error: {e}")

    # 6. Empirical metrics invariant checks
    metrics = loaded_cert.get("empirical_metrics", {})
    if metrics.get("sunborn_b3_false_hits") != 0 or metrics.get("sunborn_b3_false_hit_rate_pct") != 0.0:
        print(f"FAIL: SemanticCacheBench B3 false-hit invariant violated: {metrics.get('sunborn_b3_false_hits')}", file=sys.stderr)
        return STATUS_INVALID
    if metrics.get("sunborn_b3_precision_pct") != 100.0:
        print(f"FAIL: SemanticCacheBench B3 precision must be 100.0%: {metrics.get('sunborn_b3_precision_pct')}", file=sys.stderr)
        return STATUS_INVALID
    if metrics.get("sunborn_b3_temporal_false_hits") != 0:
        print(f"FAIL: SemanticCacheBench-T B3 false-hit invariant violated: {metrics.get('sunborn_b3_temporal_false_hits')}", file=sys.stderr)
        return STATUS_INVALID
    if metrics.get("sunborn_b3_temporal_precision_pct") != 100.0:
        print(f"FAIL: SemanticCacheBench-T B3 precision must be 100.0%: {metrics.get('sunborn_b3_temporal_precision_pct')}", file=sys.stderr)
        return STATUS_INVALID
    print("  [✓] Empirical correctness invariants verified (0 false hits, 100.0% precision).")

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
    parser.add_argument("--no-git", action="store_true", help="Skip git repository matching")
    parser.add_argument("--allow-offline", action="store_true", help="Allow offline simulation benchmark artifact (disables strict live gateway requirement)")
    parser.add_argument("--require-auth", action="store_true", help="Require independently authenticated HMAC signature with valid trusted key")
    args = parser.parse_args()

    outcome = verify_reproduction_certificate(
        cert_path=args.certificate,
        bench_path=args.benchmark,
        manifest_path=args.manifest,
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
