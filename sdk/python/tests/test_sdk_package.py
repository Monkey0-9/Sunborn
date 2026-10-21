import asyncio
import os
import sys
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

# Ensure sdk/python is in sys.path when running tests directly without pre-installed wheel
_sdk_root = str(Path(__file__).resolve().parent.parent)
if _sdk_root not in sys.path:
    sys.path.insert(0, _sdk_root)

import sunborn
from sunborn import (
    AsyncSunbornClient,
    CachePolicy,
    ExecutionTier,
    FallbackPolicy,
    SunbornAuthError,
    SunbornClient,
    SunbornConnectionError,
    SunbornError,
    SunbornForbiddenError,
    SunbornNotFoundError,
    SunbornRateLimitError,
    compute_context_digest,
)


class TestSunbornPackage(unittest.TestCase):
    def test_package_exports_and_version(self):
        self.assertTrue(hasattr(sunborn, "__version__"))
        self.assertEqual(sunborn.__version__, "0.1.0rc1")
        self.assertEqual(CachePolicy.ALLOW_STOCHASTIC, "allow-stochastic")
        self.assertEqual(FallbackPolicy.FAIL_CLOSED, "FAIL_CLOSED")
        self.assertEqual(ExecutionTier.SEMANTIC, "semantic")

    def test_sync_client_context_manager(self):
        with SunbornClient(api_key="test-key-ctx") as client:
            self.assertEqual(client.api_key, "test-key-ctx")
            self.assertEqual(client.tenant_id, "default-tenant")
            self.assertFalse(client.is_closed)
        self.assertTrue(client.is_closed)
        with self.assertRaises(RuntimeError):
            client._post("/requests", {})

    def test_async_client_context_manager(self):
        async def _test():
            async with AsyncSunbornClient(api_key="test-key-async-ctx") as client:
                self.assertEqual(client.api_key, "test-key-async-ctx")
                self.assertEqual(client.base_url, "http://localhost:8080/v1")
                self.assertFalse(client.is_closed)
            self.assertTrue(client.is_closed)

        asyncio.run(_test())

    def test_exception_subclassing_and_rfc7807(self):
        auth_err = SunbornAuthError("Invalid API key", status_code=401, problem_details={"type": "https://sunborn.ai/errors/unauthorized"})
        self.assertIsInstance(auth_err, RuntimeError)
        self.assertIsInstance(auth_err, SunbornError)
        self.assertEqual(auth_err.status_code, 401)
        self.assertEqual(auth_err.problem_details["type"], "https://sunborn.ai/errors/unauthorized")

        conn_err = SunbornConnectionError("Connection refused")
        self.assertIsInstance(conn_err, ConnectionError)
        self.assertIsInstance(conn_err, SunbornError)

        rate_err = SunbornRateLimitError("Rate limit exceeded", retry_after=5.0)
        self.assertEqual(rate_err.retry_after, 5.0)
        self.assertEqual(rate_err.status_code, 429)

    def test_compute_context_digest_contract(self):
        digest1 = compute_context_digest("gpt-4o", system_prompt="Sys A", rag_chunk_ids=["chunk-2", "chunk-1"])
        digest2 = compute_context_digest("gpt-4o", system_prompt="Sys A", rag_chunk_ids=["chunk-1", "chunk-2"])
        # Sorting guarantees determinism
        self.assertEqual(digest1, digest2)
        self.assertEqual(len(digest1), 64)


if __name__ == "__main__":
    unittest.main()
