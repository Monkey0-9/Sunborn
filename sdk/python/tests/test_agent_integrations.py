"""
Unit tests for Sunborn LangChain and LlamaIndex agent integrations.
"""

import unittest
from unittest.mock import MagicMock

from sunborn.integrations.langchain import SunbornLangChainCache, Generation
from sunborn.integrations.llamaindex import SunbornLlamaIndexCache


class TestAgentIntegrations(unittest.TestCase):
    def test_langchain_cache_lookup_hit_and_miss(self):
        mock_client = MagicMock()
        cache = SunbornLangChainCache(client=mock_client)

        # 1. Miss scenario
        mock_client.chat_completion.return_value = {
            "choices": [{"message": {"content": "Generated answer"}}],
            "sunborn_cached": False,
        }
        res = cache.lookup("What is retrieval?", "gpt-4o")
        self.assertIsNone(res)

        # 2. Hit scenario
        mock_client.chat_completion.return_value = {
            "choices": [{"message": {"content": "Cached answer from Sunborn"}}],
            "sunborn_cached": True,
        }
        res_hit = cache.lookup("What is retrieval?", "gpt-4o")
        self.assertIsNotNone(res_hit)
        self.assertEqual(len(res_hit), 1)
        self.assertEqual(res_hit[0].text, "Cached answer from Sunborn")

    def test_langchain_cache_update_and_clear(self):
        mock_client = MagicMock()
        mock_client.tenant_id = "default-tenant"
        cache = SunbornLangChainCache(client=mock_client)
        cache.update("Test prompt", "gpt-4o", [Generation(text="Answer")])
        mock_client.chat_completion.assert_called_once()

        cache.clear()
        mock_client.invalidate_graph_rtc.assert_called_once()

    def test_llamaindex_cache_get_and_put(self):
        mock_client = MagicMock()
        cache = SunbornLlamaIndexCache(client=mock_client)

        mock_client.chat_completion.return_value = {
            "choices": [{"message": {"content": "LlamaIndex hit"}}],
            "sunborn_cached": True,
        }
        val = cache.get("What is vector indexing?")
        self.assertEqual(val, "LlamaIndex hit")

        cache.put("Key", "Value")
        self.assertTrue(cache.delete("Key"))


if __name__ == "__main__":
    unittest.main()
