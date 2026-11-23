"""
Unit tests for Sunborn Unified CLI (sunborn doctor, bench, finops, shred).
"""

import io
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

_sdk_root = str(Path(__file__).resolve().parent.parent)
if _sdk_root not in sys.path:
    sys.path.insert(0, _sdk_root)

from sunborn.cli import _wilson_score_interval, main


class TestSunbornCLI(unittest.TestCase):
    def test_wilson_score_interval_math(self):
        low, high = _wilson_score_interval(100, 100)
        self.assertGreater(low, 0.95)
        self.assertAlmostEqual(high, 1.0, places=3)

        low_zero, high_zero = _wilson_score_interval(0, 0)
        self.assertEqual(low_zero, 0.0)
        self.assertEqual(high_zero, 0.0)

    def test_cli_version(self):
        with patch("sys.stdout", new=io.StringIO()) as fake_out:
            with self.assertRaises(SystemExit) as cm:
                main(["--version"])
            self.assertEqual(cm.exception.code, 0)
            self.assertIn("sunborn 1.0.0", fake_out.getvalue())

    @patch("urllib.request.urlopen")
    def test_cli_doctor_healthy(self, mock_urlopen):
        mock_resp = MagicMock()
        mock_resp.status = 200
        mock_resp.read.return_value = json.dumps({"active_nodes": 42}).encode("utf-8")
        mock_urlopen.return_value.__enter__.return_value = mock_resp

        with patch("sys.stdout", new=io.StringIO()) as fake_out:
            exit_code = main(["doctor", "--url", "http://localhost:8080/v1"])
            self.assertEqual(exit_code, 0)
            out = fake_out.getvalue()
            self.assertIn("HEALTHY", out)
            self.assertIn("Diagnostic Health Check", out)

    @patch("urllib.request.urlopen")
    def test_cli_doctor_offline(self, mock_urlopen):
        mock_urlopen.side_effect = ConnectionRefusedError("Gateway not listening")

        with patch("sys.stdout", new=io.StringIO()) as fake_out:
            exit_code = main(["doctor", "--url", "http://localhost:8080/v1"])
            self.assertEqual(exit_code, 2)
            out = fake_out.getvalue()
            self.assertIn("OFFLINE", out)

    @patch("sunborn.client.SunbornClient.chat_completion")
    def test_cli_bench(self, mock_chat):
        mock_chat.return_value = {
            "sunborn_cached": True,
            "choices": [{"message": {"content": "benchmark hit"}}]
        }

        with patch("sys.stdout", new=io.StringIO()) as fake_out:
            exit_code = main(["bench", "--requests", "5"])
            self.assertEqual(exit_code, 0)
            out = fake_out.getvalue()
            self.assertIn("Benchmark Results", out)
            self.assertIn("Cache Hit Rate", out)
            self.assertIn("Wilson 95% CI", out)

    @patch("sunborn.client.SunbornClient.get_finops_report")
    def test_cli_finops(self, mock_report):
        mock_report.return_value = {
            "total_queries": 1000,
            "cached_queries": 800,
            "avoided_tokens": 150000,
            "avoided_cost_usd": 1.25,
            "origin_cost_usd": 0.35,
        }

        with patch("sys.stdout", new=io.StringIO()) as fake_out:
            exit_code = main(["finops", "--tenant", "corp-1"])
            self.assertEqual(exit_code, 0)
            out = fake_out.getvalue()
            self.assertIn("FinOps Cost Governance Report", out)
            self.assertIn("Avoided Cost", out)
            self.assertIn("1.25", out)

    @patch("urllib.request.urlopen")
    def test_cli_shred(self, mock_urlopen):
        mock_resp = MagicMock()
        mock_resp.read.return_value = json.dumps({
            "tenant_id": "corp-target",
            "status": "shredded",
            "shred_latency_nanos": 420000,
            "verified_multi_store_erasure": True,
        }).encode("utf-8")
        mock_urlopen.return_value.__enter__.return_value = mock_resp

        with patch("sys.stdout", new=io.StringIO()) as fake_out:
            exit_code = main(["shred", "--tenant-id", "corp-target"])
            self.assertEqual(exit_code, 0)
            out = fake_out.getvalue()
            self.assertIn("SHREDDED", out)
            self.assertIn("420,000 ns", out)
            self.assertIn("0 residuals verified across all 5 tiers", out)


if __name__ == "__main__":
    unittest.main()
