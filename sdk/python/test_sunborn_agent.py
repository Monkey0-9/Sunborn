"""SDK tests exercising the real HTTP transport against a loopback stub."""

import asyncio
import datetime
import os
import sys
import unittest
import uuid
from unittest.mock import MagicMock, patch

_HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, _HERE)
sys.path.insert(0, os.path.join(_HERE, "tests"))

from fake_server import StubServer, json_response, sequence  # noqa: E402
from sunborn_agent import (  # noqa: E402
    AsyncSunbornClient,
    FallbackPolicy,
    SunbornAuthError,
    SunbornClient,
    SunbornConnectionError,
    compute_context_digest,
    sunborn_tool,
)

OPENAI_BODY = {
    "id": "chatcmpl-1",
    "object": "chat.completion",
    "created": 1,
    "model": "gpt-4o",
    "sunborn_cached": True,
    "sunborn_tier": "exact",
    "choices": [{"index": 0, "message": {"role": "assistant", "content": "ok"}}],
    "usage": {"prompt_tokens": 2, "completion_tokens": 1, "total_tokens": 3},
}


class TestSunbornClient(unittest.TestCase):
    def test_client_init_requires_key(self):
        with patch.dict("os.environ", {}, clear=True), self.assertRaises(ValueError):
            SunbornClient()

    def test_client_init_with_explicit_key(self):
        client = SunbornClient(
            base_url="http://localhost:8080/v1",
            api_key="key-test-123",
            tenant_id="tenant-alpha",
            application_id="app-1",
        )
        self.assertEqual(client.base_url, "http://localhost:8080/v1")
        self.assertEqual(client.api_key, "key-test-123")
        self.assertEqual(client.tenant_id, "tenant-alpha")
        self.assertEqual(client.application_id, "app-1")
        client.close()

    def test_client_init_normalizes_base_url(self):
        client1 = SunbornClient(base_url="http://localhost:8080", api_key="test-key")
        self.assertEqual(client1.base_url, "http://localhost:8080/v1")
        client1.close()

        client2 = SunbornClient(
            base_url="http://localhost:8080/v1/", api_key="test-key"
        )
        self.assertEqual(client2.base_url, "http://localhost:8080/v1")
        client2.close()

    def test_chat_completion_request_payload_full_options(self):
        with StubServer(lambda _req: json_response(OPENAI_BODY)) as server:
            with SunbornClient(base_url=server.base_url, api_key="test-key") as client:
                res = client.chat_completion(
                    model="gpt-4o",
                    prompt="Hello Sunborn",
                    system_prompt="You are helpful.",
                    temperature=0.7,
                    top_p=0.9,
                    max_tokens=256,
                    stop=["\n", "User:"],
                    seed=42,
                    response_format={"type": "json_object"},
                    allow_stochastic=True,
                    context_digest="abcd1234efgh5678",
                )

            self.assertEqual(res["choices"][0]["message"]["content"], "ok")
            request = server.last_request
            self.assertEqual(request.path, "/v1/chat/completions")
            self.assertEqual(
                request.headers.get("x-sunborn-cache-policy"), "allow-stochastic"
            )
            self.assertEqual(
                request.headers.get("x-sunborn-context-digest"), "abcd1234efgh5678"
            )
            self.assertEqual(request.headers.get("x-api-key"), "test-key")
            body = request.json()
            self.assertEqual(body["model"], "gpt-4o")
            self.assertEqual(body["temperature"], 0.7)
            self.assertEqual(body["top_p"], 0.9)
            self.assertEqual(body["max_tokens"], 256)
            self.assertEqual(body["stop"], ["\n", "User:"])
            self.assertEqual(body["seed"], 42)
            self.assertEqual(body["messages"][0]["role"], "system")

    def test_execute_request_returns_typed_response(self):
        with StubServer(lambda _req: json_response(OPENAI_BODY)) as server:
            with SunbornClient(base_url=server.base_url, api_key="test-key") as client:
                response = client.execute_request("Explain caching.")

            self.assertEqual(response.content, "ok")
            self.assertTrue(response.cached)
            self.assertEqual(response.tier, "exact")
            self.assertEqual(response.model, "gpt-4o")
            self.assertEqual(response.usage["total_tokens"], 3)
            self.assertEqual(response.raw["id"], "chatcmpl-1")

    def test_anthropic_messages_payload(self):
        body = {"id": "msg_1", "content": [{"type": "text", "text": "hello"}]}
        with StubServer(lambda _req: json_response(body)) as server:
            with SunbornClient(base_url=server.base_url, api_key="test-key") as client:
                res = client.create_message(
                    model="claude-3-5-sonnet-20241022",
                    messages=[{"role": "user", "content": "Hi"}],
                    system="System instructions",
                    max_tokens=500,
                    temperature=0.2,
                    stop_sequences=["STOP"],
                )

            self.assertEqual(res["id"], "msg_1")
            request = server.last_request
            self.assertEqual(request.path, "/v1/messages")
            payload = request.json()
            self.assertEqual(payload["system"], "System instructions")
            self.assertEqual(payload["max_tokens"], 500)
            self.assertEqual(payload["stop_sequences"], ["STOP"])

    def test_evict_execution_requires_selector_and_sends_tool_name(self):
        with StubServer(lambda _req: json_response({"evicted_count": 3})) as server:
            with SunbornClient(base_url=server.base_url, api_key="admin") as client:
                with self.assertRaises(ValueError):
                    client.evict_execution()
                result = client.evict_execution(tool_name="langchain-cache")

            self.assertEqual(result["evicted_count"], 3)
            request = server.last_request
            self.assertEqual(request.path, "/v1/execution/evict")
            self.assertEqual(request.json()["tool_name"], "langchain-cache")

    def test_error_handling_http_error(self):
        with StubServer(
            lambda _req: (401, {"detail": "invalid_api_key"})
        ) as server:
            with SunbornClient(base_url=server.base_url, api_key="bad-key") as client:
                with self.assertRaises(SunbornAuthError) as ctx:
                    client.get_finops_report()
            self.assertIn("401", str(ctx.exception))
            self.assertIn("invalid_api_key", str(ctx.exception))

    def test_error_handling_connection_error(self):
        client = SunbornClient(
            base_url="http://127.0.0.1:1",
            api_key="key",
            max_retries=0,
            timeout=0.5,
        )
        with self.assertRaises(SunbornConnectionError):
            client.get_finops_report()
        client.close()

    def test_retry_on_server_error_then_success(self):
        responder = sequence(
            (503, {"detail": "busy"}),
            (200, {"status": "ok"}),
        )
        with StubServer(responder) as server:
            with SunbornClient(
                base_url=server.base_url,
                api_key="test-key",
                max_retries=2,
                backoff_factor=0.0,
            ) as client:
                result = client.get_finops_report()

            self.assertEqual(result["status"], "ok")
            self.assertEqual(len(server.requests), 2)

    def test_empty_response_decoding(self):
        with StubServer(lambda _req: (200, None)) as server:
            with SunbornClient(base_url=server.base_url, api_key="test-key") as client:
                self.assertEqual(client._get("empty/endpoint"), {})

    def test_closed_client_get_raises(self):
        client = SunbornClient(base_url="http://127.0.0.1:1", api_key="test-key")
        client.close()
        self.assertTrue(client.is_closed)
        with self.assertRaises(RuntimeError):
            client._get("finops/report")

    def test_graph_caching_sdk_methods(self):
        def responder(request):
            if "lineage" in request.path:
                return json_response(
                    {
                        "key": "test-key-1",
                        "is_cached": True,
                        "upstream_source_ids": ["00000000-0000-0000-0000-000000000001"],
                    }
                )
            if "invalidate" in request.path:
                return json_response({"status": "invalidated", "evicted_records": 1})
            return json_response(
                {
                    "node_count": 10,
                    "edge_count": 25,
                    "cached_entries": 100,
                    "max_capacity": 65536,
                    "invalidated_records": 2,
                    "hits": 80,
                    "misses": 20,
                }
            )

        with StubServer(responder) as server:
            with SunbornClient(base_url=server.base_url, api_key="test-key") as client:
                stats = client.get_graph_stats()
                lineage = client.get_graph_lineage("test-key-1")
                inv = client.invalidate_graph_rtc(
                    source_ids=["00000000-0000-0000-0000-000000000001"],
                    doc_hashes=[12345],
                )

            self.assertEqual(stats["node_count"], 10)
            self.assertTrue(lineage["is_cached"])
            self.assertEqual(inv["status"], "invalidated")
            self.assertEqual(server.requests[-1].json()["doc_hashes"], [12345])

    def test_async_sunborn_client(self):
        with StubServer(lambda _req: json_response(OPENAI_BODY)) as server:
            client = AsyncSunbornClient(base_url=server.base_url, api_key="test-key")
            self.assertEqual(client.base_url, f"{server.base_url}/v1")

            async def run():
                async with client:
                    res = await client.chat_completion(
                        model="gpt-4o", prompt="Async test"
                    )
                    self.assertEqual(res["choices"][0]["message"]["content"], "ok")
                self.assertTrue(client.is_closed)

            asyncio.run(run())

    def test_async_sunborn_client_login(self):
        def responder(request):
            if request.path.endswith("/auth/login"):
                return json_response(
                    {
                        "access_token": "token-123",
                        "tenant_id": "t-1",
                        "application_id": "app-1",
                    }
                )
            return json_response(OPENAI_BODY)

        with StubServer(responder) as server:
            client = AsyncSunbornClient(base_url=server.base_url, api_key="init-key")

            async def run():
                async with client:
                    res = await client.login(
                        client_id="cid", client_secret="csecret"
                    )
                    self.assertEqual(res["access_token"], "token-123")
                    self.assertEqual(client.api_key, "token-123")
                    self.assertEqual(client.tenant_id, "t-1")
                    self.assertEqual(client.application_id, "app-1")
                    chat = await client.chat_completion(
                        model="gpt-4o", prompt="after login"
                    )
                    self.assertEqual(chat["choices"][0]["message"]["content"], "ok")

            asyncio.run(run())

    def test_compute_context_digest_parity(self):
        cd1 = compute_context_digest("gpt-4o", "sys", ["chunk_b", "chunk_a"])
        cd2 = compute_context_digest("gpt-4o", "sys", ["chunk_a", "chunk_b"])
        self.assertEqual(cd1, cd2)
        self.assertEqual(len(cd1), 64)

        cd3 = compute_context_digest("gpt-4o-mini", "sys", ["chunk_a", "chunk_b"])
        self.assertNotEqual(cd1, cd3)

        cd4 = compute_context_digest("gpt-4o", "sys", ["chunk_a", "chunk_c"])
        self.assertNotEqual(cd1, cd4)


class TestSunbornToolDecorator(unittest.TestCase):
    def test_tool_decorator_cached_hit(self):
        client = MagicMock(spec=SunbornClient)
        client.plan_execution_dag.return_value = {
            "cached_nodes": 1,
            "node_statuses": {
                "node-test_func-f2a89c4b": {
                    "status": "Cached",
                    "details": {"output": 42},
                }
            },
        }

        with patch("sunborn_agent.hashlib.sha256") as mock_sha:
            mock_hasher = MagicMock()
            mock_hasher.hexdigest.return_value = "f2a89c4b123456"
            mock_sha.return_value = mock_hasher

            @sunborn_tool(name="test_func", client=client)
            def compute(x):
                return x * 2

            val = compute(21)
            self.assertEqual(val, 42)
            client.record_tool_result.assert_not_called()

    def test_tool_decorator_miss_executes_and_records(self):
        client = MagicMock(spec=SunbornClient)
        client.plan_execution_dag.return_value = {
            "cached_nodes": 0,
            "node_statuses": {},
        }

        @sunborn_tool(name="add_nums", client=client, ttl_secs=120)
        def add(a, b):
            return a + b

        result = add(10, 20)
        self.assertEqual(result, 30)
        client.record_tool_result.assert_called_once()
        _, kwargs = client.record_tool_result.call_args
        self.assertEqual(kwargs["tool_name"], "add_nums")
        self.assertEqual(kwargs["output"], 30)
        self.assertEqual(kwargs["ttl_secs"], 120)

    def test_tool_decorator_fail_closed_on_error(self):
        client = MagicMock(spec=SunbornClient)
        client.plan_execution_dag.side_effect = RuntimeError("Network partition")

        @sunborn_tool(
            name="strict_tool", policy=FallbackPolicy.FAIL_CLOSED, client=client
        )
        def strict_calc():
            return 100

        with self.assertRaises(RuntimeError) as ctx:
            strict_calc()
        self.assertIn("FAIL_CLOSED", str(ctx.exception))

    def test_tool_decorator_permissive_on_error(self):
        client = MagicMock(spec=SunbornClient)
        client.plan_execution_dag.side_effect = RuntimeError("Network partition")

        @sunborn_tool(name="lax_tool", policy=FallbackPolicy.PERMISSIVE, client=client)
        def lax_calc():
            return 200

        self.assertEqual(lax_calc(), 200)

    def test_tool_decorator_complex_types(self):
        client = MagicMock(spec=SunbornClient)
        client.plan_execution_dag.return_value = {
            "cached_nodes": 0,
            "node_statuses": {},
        }

        @sunborn_tool(name="complex_tool", client=client)
        def process_record(u_id, dt):
            return f"{u_id}-{dt.year}"

        now = datetime.datetime.now()
        unique = uuid.uuid4()
        self.assertEqual(process_record(unique, now), f"{unique}-{now.year}")

    def test_tool_decorator_non_deterministic(self):
        client = MagicMock(spec=SunbornClient)

        @sunborn_tool(name="random_tool", deterministic=False, client=client)
        def random_action():
            return 999

        self.assertEqual(random_action(), 999)
        client.plan_execution_dag.assert_not_called()
        client.record_tool_result.assert_not_called()

    def test_async_tool_decorator_cached_hit(self):
        client = MagicMock(spec=SunbornClient)
        client.plan_execution_dag.return_value = {
            "cached_nodes": 1,
            "node_statuses": {
                "node-async_calc-f2a89c4b": {
                    "status": "Cached",
                    "details": {"output": 84},
                }
            },
        }

        with patch("sunborn_agent.hashlib.sha256") as mock_sha:
            mock_hasher = MagicMock()
            mock_hasher.hexdigest.return_value = "f2a89c4b123456"
            mock_sha.return_value = mock_hasher

            @sunborn_tool(name="async_calc", client=client)
            async def async_compute(x):
                return x * 4

            self.assertEqual(asyncio.run(async_compute(21)), 84)
            client.record_tool_result.assert_not_called()

    def test_async_tool_decorator_miss_executes_and_records(self):
        client = MagicMock(spec=SunbornClient)
        client.plan_execution_dag.return_value = {
            "cached_nodes": 0,
            "node_statuses": {},
        }

        @sunborn_tool(name="async_multiply", client=client, ttl_secs=60)
        async def async_mult(a, b):
            return a * b

        self.assertEqual(asyncio.run(async_mult(6, 7)), 42)
        client.record_tool_result.assert_called_once()
        _, kwargs = client.record_tool_result.call_args
        self.assertEqual(kwargs["tool_name"], "async_multiply")
        self.assertEqual(kwargs["output"], 42)
        self.assertEqual(kwargs["ttl_secs"], 60)


if __name__ == "__main__":
    unittest.main()
