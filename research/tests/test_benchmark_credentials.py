import unittest
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))

from benchmarks.semantic_cache_bench import BENCHMARK_SUITE
from research.benchmark_credentials import build_benchmark_credentials


class BenchmarkCredentialTests(unittest.TestCase):
    def test_each_case_has_an_isolated_least_privilege_credential(self):
        credentials = build_benchmark_credentials()
        case_ids = [item["id"] for item in BENCHMARK_SUITE]

        self.assertEqual(len(credentials), len(case_ids))
        self.assertEqual(len({credential["app"] for credential in credentials}), len(case_ids))
        self.assertEqual(len({credential["key"] for credential in credentials}), len(case_ids))
        self.assertEqual(
            [credential["app"] for credential in credentials],
            [f"bench-{case_id}" for case_id in case_ids],
        )
        self.assertEqual(
            [credential["key"] for credential in credentials],
            [f"sunborn-benchmark-{case_id}" for case_id in case_ids],
        )
        self.assertTrue(
            all(
                credential["tenant"] == "enterprise-fintech"
                and credential["allowed_models"] == ["gpt-4o"]
                and credential["is_admin"] is False
                for credential in credentials
            )
        )


if __name__ == "__main__":
    unittest.main()
