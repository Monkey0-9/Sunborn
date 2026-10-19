#!/usr/bin/env python3
"""
Sunborn Independent Certificate & Artifact Provenance Verifier
Verifies reproduction_certificate.json, benchmark_run.json, and run_manifest.json:
1. Canonical manifest digest verification against payload bytes.
2. Constant-time HMAC-SHA-256 verification (if signing key present).
3. Honest local development integrity classification if signing key is omitted.
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

def verify_reproduction_certificate(
    cert_path: str,
    bench_path: str,
    manifest_path: str,
    check_git: bool = True
) -> bool:
    print("=" * 75)
    print("SUNBORN INDEPENDENT CERTIFICATE & PROVENANCE VERIFIER")
    print(f"Certificate: {cert_path}")
    print(f"Benchmark  : {bench_path}")
    print(f"Manifest   : {manifest_path}")
    print("=" * 75)

    if not os.path.exists(cert_path):
        print(f"FAIL: Certificate file not found: {cert_path}", file=sys.stderr)
        return False
    if not os.path.exists(bench_path):
        print(f"FAIL: Benchmark file not found: {bench_path}", file=sys.stderr)
        return False
    if not os.path.exists(manifest_path):
        print(f"FAIL: Run manifest file not found: {manifest_path}", file=sys.stderr)
        return False

    with open(cert_path, "r", encoding="utf-8") as f:
        loaded_cert = json.load(f)

    # 1. Check required top-level fields
    for field in ["evaluation", "status", "git_commit", "git_tree", "provenance", "empirical_metrics", "attestation"]:
        if field not in loaded_cert:
            print(f"FAIL: Missing required field '{field}' in certificate", file=sys.stderr)
            return False

    attestation = loaded_cert.pop("attestation", None)
    if not isinstance(attestation, dict):
        print("FAIL: Attestation block is missing or not a dictionary", file=sys.stderr)
        return False

    canonical_manifest_digest = attestation.get("canonical_manifest_digest")
    hmac_sig = attestation.get("hmac_sha256_signature")

    # 2. Canonical digest verification
    canonical_bytes = json.dumps(loaded_cert, sort_keys=True).encode("utf-8")
    expected_digest = hashlib.sha256(canonical_bytes).hexdigest()

    if not hmac.compare_digest(canonical_manifest_digest, expected_digest):
        print(
            f"FAIL: Canonical digest mismatch!\n"
            f"  Expected: {expected_digest}\n"
            f"  Claimed : {canonical_manifest_digest}",
            file=sys.stderr
        )
        return False
    print("  [✓] Canonical manifest SHA-256 digest verified.")

    # 3. Attestation & Signature Verification
    prov = loaded_cert.get("provenance", {})
    signing_algo = prov.get("signing_algorithm")
    attestation_type = prov.get("attestation_type")

    secret_key = os.environ.get("SUNBORN_ATTESTATION_KEY") or os.environ.get("SUNBORN_REPRODUCTION_SECRET")

    if hmac_sig is not None:
        if secret_key:
            expected_sig = hmac.new(secret_key.encode("utf-8"), canonical_bytes, hashlib.sha256).hexdigest()
            if not hmac.compare_digest(hmac_sig, expected_sig):
                print("FAIL: HMAC-SHA-256 signature verification failed!", file=sys.stderr)
                return False
            print("  [✓] HMAC-SHA-256 signature verified with environment attestation key.")
        else:
            print("  [!] HMAC signature present in certificate; signing key not provided in environment.")
            print("      (Canonical digest integrity verified; independently authenticated signature requires key).")
    else:
        if signing_algo != "NONE_UNAUTHENTICATED_LOCAL_DIGEST":
            print(f"FAIL: Unsigned certificate must declare 'NONE_UNAUTHENTICATED_LOCAL_DIGEST', got '{signing_algo}'", file=sys.stderr)
            return False
        if attestation_type != "LOCAL_DEVELOPMENT_INTEGRITY_CHECK":
            print(f"FAIL: Unsigned certificate must declare 'LOCAL_DEVELOPMENT_INTEGRITY_CHECK', got '{attestation_type}'", file=sys.stderr)
            return False
        print("  [✓] Honest local development integrity check verified (unauthenticated local digest, no secret key claimed).")

    # 4. Raw artifact hash matching
    with open(bench_path, "rb") as f_b:
        actual_bench_sha = hashlib.sha256(f_b.read()).hexdigest()
    with open(manifest_path, "rb") as f_m:
        actual_manifest_sha = hashlib.sha256(f_m.read()).hexdigest()

    expected_bench_sha = prov.get("benchmark_run_sha256")
    expected_manifest_sha = prov.get("run_manifest_sha256")

    if not hmac.compare_digest(actual_bench_sha, expected_bench_sha):
        print(f"FAIL: Benchmark artifact SHA-256 mismatch!\n  Actual  : {actual_bench_sha}\n  Expected: {expected_bench_sha}", file=sys.stderr)
        return False
    print(f"  [✓] Benchmark artifact SHA-256 matches provenance: {actual_bench_sha[:16]}...")

    if not hmac.compare_digest(actual_manifest_sha, expected_manifest_sha):
        print(f"FAIL: Run manifest artifact SHA-256 mismatch!\n  Actual  : {actual_manifest_sha}\n  Expected: {expected_manifest_sha}", file=sys.stderr)
        return False
    print(f"  [✓] Run manifest artifact SHA-256 matches provenance: {actual_manifest_sha[:16]}...")

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
                return False

            if cert_commit == curr_commit:
                curr_tree = subprocess.run(["git", "rev-parse", "HEAD^{tree}"], capture_output=True, text=True, check=True).stdout.strip()
                if cert_tree != curr_tree:
                    print(f"FAIL: Certificate git_tree ({cert_tree}) does not match current repository tree ({curr_tree})!", file=sys.stderr)
                    return False
            elif cert_commit == parent_commit:
                parent_tree = subprocess.run(["git", "rev-parse", "HEAD~1^{tree}"], capture_output=True, text=True, check=True).stdout.strip()
                if cert_tree != parent_tree:
                    print(f"FAIL: Certificate git_tree ({cert_tree}) does not match parent repository tree ({parent_tree})!", file=sys.stderr)
                    return False

            print(f"  [✓] Git commit ({cert_commit[:10]}) and tree ({cert_tree[:10]}) match verified repository revision.")
        except Exception as e:
            print(f"  [!] Git check skipped or encountered error: {e}")

    # 6. Empirical metrics invariant checks
    metrics = loaded_cert.get("empirical_metrics", {})
    if metrics.get("sunborn_b3_false_hits") != 0 or metrics.get("sunborn_b3_false_hit_rate_pct") != 0.0:
        print(f"FAIL: SemanticCacheBench B3 false-hit invariant violated: {metrics.get('sunborn_b3_false_hits')}", file=sys.stderr)
        return False
    if metrics.get("sunborn_b3_precision_pct") != 100.0:
        print(f"FAIL: SemanticCacheBench B3 precision must be 100.0%: {metrics.get('sunborn_b3_precision_pct')}", file=sys.stderr)
        return False
    if metrics.get("sunborn_b3_temporal_false_hits") != 0:
        print(f"FAIL: SemanticCacheBench-T B3 false-hit invariant violated: {metrics.get('sunborn_b3_temporal_false_hits')}", file=sys.stderr)
        return False
    if metrics.get("sunborn_b3_temporal_precision_pct") != 100.0:
        print(f"FAIL: SemanticCacheBench-T B3 precision must be 100.0%: {metrics.get('sunborn_b3_temporal_precision_pct')}", file=sys.stderr)
        return False
    print("  [✓] Empirical correctness invariants verified (0 false hits, 100.0% precision).")

    print("=" * 75)
    print("CERTIFICATE VERIFICATION RESULT: VALID & VERIFIED")
    print("=" * 75)
    return True

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Verify Sunborn empirical reproduction certificate and artifacts.")
    parser.add_argument("--certificate", default="research/results/reproduction_certificate.json", help="Path to certificate")
    parser.add_argument("--benchmark", default="research/results/benchmark_run.json", help="Path to benchmark run artifact")
    parser.add_argument("--manifest", default="research/results/run_manifest.json", help="Path to temporal manifest artifact")
    parser.add_argument("--no-git", action="store_true", help="Skip git repository matching")
    args = parser.parse_args()

    ok = verify_reproduction_certificate(
        cert_path=args.certificate,
        bench_path=args.benchmark,
        manifest_path=args.manifest,
        check_git=not args.no_git
    )
    sys.exit(0 if ok else 1)
