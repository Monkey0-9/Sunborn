"""Tests for the LangChain and LlamaIndex execution-reuse adapters."""

import os
import sys
import unittest
from unittest.mock import MagicMock

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), ".."))
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from fake_server import StubServer, json_response  # noqa: E402

from sunborn.client import SunbornClient  # noqa: E402
from sunborn.integrations.langchain import Generation, SunbornLangChainCache  # noqa: E402
from sunborn.integrations.llamaindex import SunbornLlamaIndexCache  # noqa: E402


def _plan_node_id(request) -> str:
    body = request.json()
    return body["plan"]["nodes"][0]["id"]


class TestLangChainCache(unittest.TestCase):
    def test_lookup_miss_then_hit(self):
        client = MagicMock(spec=SunbornClient)
        cache = SunbornLangChainCache(client=client)

        client.plan_execution_dag.return_value = {
            "cached_nodes": 0,
            "node_statuses": {},
        }
        self.assertIsNone(cache.lookup("What is retrieval?", "gpt-4o"))

        node_id = client.plan_execution_dag.call_args[0][0][0]["id"]
        client.plan_execution_dag.return_value = {
            "cached_nodes": 1,
            "node_statuses": {
                node_id: {
                    "status": "Cached",
                    "details": {"output": {"texts": ["Cached answer"]}},
                }
            },
        }
        result = cache.lookup("What is retrieval?", "gpt-4o")
        self.assertIsNotNone(result)
        self.assertEqual(len(result), 1)
        self.assertEqual(result[0].text, "Cached answer")

    def test_update_records_the_actual_generations(self):
        client = MagicMock(spec=SunbornClient)
        cache = SunbornLangChainCache(client=client, ttl_secs=90)
        cache.update(
            "Test prompt",
            "gpt-4o",
            [Generation(text="A"), Generation(text="B")],
        )

        _, kwargs = client.record_tool_result.call_args
        self.assertEqual(kwargs["tool_name"], "langchain-cache")
        self.assertEqual(kwargs["output"], {"texts": ["A", "B"]})
        self.assertEqual(kwargs["ttl_secs"], 90)
        self.assertEqual(kwargs["arguments"]["prompt"], "Test prompt")

    def test_clear_evicts_the_adapter_tool(self):
        client = MagicMock(spec=SunbornClient)
        cache = SunbornLangChainCache(client=client)
        cache.clear()
        client.evict_execution.assert_called_once_with(tool_name="langchain-cache")

    def test_fail_closed_vs_permissive(self):
        client = MagicMock(spec=SunbornClient)
        client.plan_execution_dag.side_effect = RuntimeError("offline")

        permissive = SunbornLangChainCache(client=client)
        self.assertIsNone(permissive.lookup("p", "m"))

        strict = SunbornLangChainCache(client=client, fail_closed=True)
        with self.assertRaises(RuntimeError):
            strict.lookup("p", "m")

    def test_real_gateway_paths_and_payloads(self):
        def responder(request):
            if request.path.endswith("/execution/plan"):
                node_id = _plan_node_id(request)
                return json_response(
                    {
                        "cached_nodes": 1,
                        "node_statuses": {
                            node_id: {
                                "status": "Cached",
                                "details": {"output": {"texts": ["from gateway"]}},
                            }
                        },
                    }
                )
            if request.path.endswith("/execution/results"):
                return json_response({"signature": "sig-1", "status": "stored"})
            return json_response({"evicted_count": 2})

        with StubServer(responder) as server:
            with SunbornClient(base_url=server.base_url, api_key="test-key") as client:
                cache = SunbornLangChainCache(client=client)
                hit = cache.lookup("prompt", "llm")
                cache.update("prompt", "llm", [Generation(text="answer")])
                cache.clear()

        self.assertEqual(hit[0].text, "from gateway")
        paths = [request.path for request in server.requests]
        self.assertIn("/v1/execution/plan", paths)
        self.assertIn("/v1/execution/results", paths)
        self.assertIn("/v1/execution/evict", paths)
        stored = next(r for r in server.requests if r.path.endswith("/execution/results"))
        self.assertEqual(stored.json()["output"], {"texts": ["answer"]})


class TestLlamaIndexCache(unittest.TestCase):
    def test_put_get_delete_use_one_key_per_tool(self):
        client = MagicMock(spec=SunbornClient)
        cache = SunbornLlamaIndexCache(client=client)

        cache.put("key-1", {"answer": 42})
        _, put_kwargs = client.record_tool_result.call_args
        tool_name = put_kwargs["tool_name"]
        self.assertTrue(tool_name.startswith("llamaindex-cache:"))
        self.assertEqual(put_kwargs["output"], {"value": {"answer": 42}})

        client.plan_execution_dag.return_value = {
            "cached_nodes": 1,
            "node_statuses": {
                f"node-{tool_name}": {
                    "status": "Cached",
                    "details": {"output": {"value": {"answer": 42}}},
                }
            },
        }
        self.assertEqual(cache.get("key-1"), {"answer": 42})

        self.assertTrue(cache.delete("key-1"))
        client.evict_execution.assert_called_once_with(tool_name=tool_name)

    def test_miss_returns_none(self):
        client = MagicMock(spec=SunbornClient)
        client.plan_execution_dag.return_value = {"cached_nodes": 0, "node_statuses": {}}
        cache = SunbornLlamaIndexCache(client=client)
        self.assertIsNone(cache.get("missing"))

    def test_delete_returns_false_when_gateway_refuses(self):
        client = MagicMock(spec=SunbornClient)
        client.evict_execution.side_effect = RuntimeError("403")
        cache = SunbornLlamaIndexCache(client=client)
        self.assertFalse(cache.delete("key-1"))


if __name__ == "__main__":
    unittest.main()
