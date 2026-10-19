#!/usr/bin/env python3
"""
Unit tests proving negative verification cases for Sunborn reproduction gates:
1. Rejection of offline simulation artifacts under strict live verification.
2. Rejection of mixed execution modes (offline_calibrated_simulation in trace).
3. Rejection of truncated evaluation traces (len != 124).
4. Rejection of duplicate or omitted query IDs.
5. Correct 4-state authentication semantics (AUTHENTICATED, INTEGRITY_VERIFIED_UNAUTHENTICATED, AUTHENTICATION_UNVERIFIED, INVALID).
"""

import os
import sys
import json
import hmac
import shutil
import hashlib
import tempfile
import unittest

# Ensure research module can be imported
sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), "..")))
from verify_certificate import (
    verify_reproduction_certificate,
    STATUS_AUTHENTICATED,
    STATUS_INTEGRITY_VERIFIED_UNAUTHENTICATED,
    STATUS_AUTHENTICATION_UNVERIFIED,
    STATUS_INVALID,
)

class NegativeVerifierTests(unittest.TestCase):
    def setUp(self):
        self.test_dir = tempfile.mkdtemp(prefix="sunborn_neg_test_")
        self.real_cert_path = os.path.abspath(
            os.path.join(os.path.dirname(__file__), "..", "results", "reproduction_certificate.json")
        )
        self.real_bench_path = os.path.abspath(
            os.path.join(os.path.dirname(__file__), "..", "results", "benchmark_run.json")
        )
        self.real_manifest_path = os.path.abspath(
            os.path.join(os.path.dirname(__file__), "..", "results", "run_manifest.json")
        )

        self.cert_copy = os.path.join(self.test_dir, "reproduction_certificate.json")
        self.bench_copy = os.path.join(self.test_dir, "benchmark_run.json")
        self.manifest_copy = os.path.join(self.test_dir, "run_manifest.json")

        shutil.copy(self.real_cert_path, self.cert_copy)
        shutil.copy(self.real_bench_path, self.bench_copy)
        shutil.copy(self.real_manifest_path, self.manifest_copy)

    def tearDown(self):
        shutil.rmtree(self.test_dir, ignore_errors=True)

    def _sync_cert_hashes(self, bench_data=None, manifest_data=None, signature=None, signing_algo=None, att_type=None):
        if bench_data is not None:
            with open(self.bench_copy, "w", encoding="utf-8") as f:
                json.dump(bench_data, f, indent=2)
        if manifest_data is not None:
            with open(self.manifest_copy, "w", encoding="utf-8") as f:
                json.dump(manifest_data, f, indent=2)

        with open(self.bench_copy, "rb") as f:
            bench_sha = hashlib.sha256(f.read()).hexdigest()
        with open(self.manifest_copy, "rb") as f:
            manifest_sha = hashlib.sha256(f.read()).hexdigest()

        with open(self.cert_copy, "r", encoding="utf-8") as f:
            cert = json.load(f)

        cert.pop("attestation", None)
        cert["provenance"]["benchmark_run_sha256"] = bench_sha
        cert["provenance"]["run_manifest_sha256"] = manifest_sha

        if signing_algo is not None:
            cert["provenance"]["signing_algorithm"] = signing_algo
        if att_type is not None:
            cert["provenance"]["attestation_type"] = att_type

        canonical_bytes = json.dumps(cert, sort_keys=True).encode("utf-8")
        canonical_digest = hashlib.sha256(canonical_bytes).hexdigest()

        cert["attestation"] = {
            "canonical_manifest_digest": canonical_digest,
            "hmac_sha256_signature": signature,
        }

        with open(self.cert_copy, "w", encoding="utf-8") as f:
            json.dump(cert, f, indent=2)

    def test_baseline_clean_artifact_passes(self):
        outcome = verify_reproduction_certificate(
            cert_path=self.cert_copy,
            bench_path=self.bench_copy,
            manifest_path=self.manifest_copy,
            check_git=True,
            require_live=True,
            require_auth=False,
        )
        self.assertEqual(outcome, STATUS_INTEGRITY_VERIFIED_UNAUTHENTICATED)

    def test_offline_simulation_artifact_is_strictly_rejected(self):
        with open(self.bench_copy, "r", encoding="utf-8") as f:
            bench_data = json.load(f)

        # Mutate provenance_type to simulate offline fallback artifact
        bench_data["metadata"]["provenance_type"] = "FROZEN_OFFLINE_BENCHMARK_EVALUATION"
        self._sync_cert_hashes(bench_data=bench_data)

        # 1. Strict live verification MUST REJECT offline artifact
        outcome_live = verify_reproduction_certificate(
            cert_path=self.cert_copy,
            bench_path=self.bench_copy,
            manifest_path=self.manifest_copy,
            check_git=True,
            require_live=True,
            require_auth=False,
        )
        self.assertEqual(outcome_live, STATUS_INVALID)

        # 2. Permissive offline mode accepts it
        outcome_offline = verify_reproduction_certificate(
            cert_path=self.cert_copy,
            bench_path=self.bench_copy,
            manifest_path=self.manifest_copy,
            check_git=True,
            require_live=False,
            require_auth=False,
        )
        self.assertEqual(outcome_offline, STATUS_INTEGRITY_VERIFIED_UNAUTHENTICATED)

    def test_simulated_execution_mode_in_trace_is_strictly_rejected(self):
        with open(self.bench_copy, "r", encoding="utf-8") as f:
            bench_data = json.load(f)

        # Corrupt single query execution mode to simulated fallback
        bench_data["b3_evaluation_trace"][5]["execution_mode"] = "offline_calibrated_simulation"
        self._sync_cert_hashes(bench_data=bench_data)

        outcome = verify_reproduction_certificate(
            cert_path=self.cert_copy,
            bench_path=self.bench_copy,
            manifest_path=self.manifest_copy,
            check_git=True,
            require_live=True,
        )
        self.assertEqual(outcome, STATUS_INVALID)

    def test_suite_size_mismatch_is_strictly_rejected(self):
        with open(self.bench_copy, "r", encoding="utf-8") as f:
            bench_data = json.load(f)

        # Truncate evaluation traces to 123 items instead of 124
        bench_data["b3_evaluation_trace"] = bench_data["b3_evaluation_trace"][:123]
        self._sync_cert_hashes(bench_data=bench_data)

        outcome = verify_reproduction_certificate(
            cert_path=self.cert_copy,
            bench_path=self.bench_copy,
            manifest_path=self.manifest_copy,
            check_git=True,
            require_live=True,
        )
        self.assertEqual(outcome, STATUS_INVALID)

    def test_duplicate_query_id_is_strictly_rejected(self):
        with open(self.bench_copy, "r", encoding="utf-8") as f:
            bench_data = json.load(f)

        # Inject duplicate ID
        bench_data["b3_evaluation_trace"][1]["id"] = bench_data["b3_evaluation_trace"][0]["id"]
        self._sync_cert_hashes(bench_data=bench_data)

        outcome = verify_reproduction_certificate(
            cert_path=self.cert_copy,
            bench_path=self.bench_copy,
            manifest_path=self.manifest_copy,
            check_git=True,
            require_live=True,
        )
        self.assertEqual(outcome, STATUS_INVALID)

    def test_authentication_status_semantics(self):
        test_key = "test_signing_key_secret_2026"

        with open(self.cert_copy, "r", encoding="utf-8") as f:
            cert = json.load(f)
        cert.pop("attestation", None)
        canonical_bytes = json.dumps(cert, sort_keys=True).encode("utf-8")
        test_sig = hmac.new(test_key.encode("utf-8"), canonical_bytes, hashlib.sha256).hexdigest()

        # Case A: Signature present, key NOT provided in environment -> AUTHENTICATION_UNVERIFIED
        self._sync_cert_hashes(
            signature=test_sig,
            signing_algo="HMAC-SHA-256",
            att_type="PRODUCTION_INDEPENDENT_ATTESTATION"
        )
        old_env = os.environ.pop("SUNBORN_ATTESTATION_KEY", None)
        old_env2 = os.environ.pop("SUNBORN_REPRODUCTION_SECRET", None)
        try:
            outcome = verify_reproduction_certificate(
                cert_path=self.cert_copy,
                bench_path=self.bench_copy,
                manifest_path=self.manifest_copy,
                require_live=True,
                require_auth=False,
            )
            self.assertEqual(outcome, STATUS_AUTHENTICATION_UNVERIFIED)

            # Case B: Signature present, key missing, but --require-auth is TRUE -> INVALID
            outcome_req = verify_reproduction_certificate(
                cert_path=self.cert_copy,
                bench_path=self.bench_copy,
                manifest_path=self.manifest_copy,
                require_live=True,
                require_auth=True,
            )
            self.assertEqual(outcome_req, STATUS_INVALID)

            # Case C: Signature present, key provided in environment -> AUTHENTICATED
            os.environ["SUNBORN_ATTESTATION_KEY"] = test_key
            # Recompute signature because provenance changed slightly in sync
            with open(self.cert_copy, "r", encoding="utf-8") as f:
                c_now = json.load(f)
            c_now.pop("attestation", None)
            valid_sig = hmac.new(test_key.encode("utf-8"), json.dumps(c_now, sort_keys=True).encode("utf-8"), hashlib.sha256).hexdigest()
            self._sync_cert_hashes(
                signature=valid_sig,
                signing_algo="HMAC-SHA-256",
                att_type="PRODUCTION_INDEPENDENT_ATTESTATION"
            )

            outcome_auth = verify_reproduction_certificate(
                cert_path=self.cert_copy,
                bench_path=self.bench_copy,
                manifest_path=self.manifest_copy,
                require_live=True,
                require_auth=True,
            )
            self.assertEqual(outcome_auth, STATUS_AUTHENTICATED)
        finally:
            if old_env is not None:
                os.environ["SUNBORN_ATTESTATION_KEY"] = old_env
            else:
                os.environ.pop("SUNBORN_ATTESTATION_KEY", None)
            if old_env2 is not None:
                os.environ["SUNBORN_REPRODUCTION_SECRET"] = old_env2
            else:
                os.environ.pop("SUNBORN_REPRODUCTION_SECRET", None)

if __name__ == "__main__":
    unittest.main()
