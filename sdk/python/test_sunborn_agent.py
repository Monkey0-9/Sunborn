import os
import sys
import uuid
import datetime
import asyncio
import unittest
from unittest.mock import MagicMock, patch
import urllib.error

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sunborn_agent import (
    SunbornClient,
    AsyncSunbornClient,
    FallbackPolicy,
    sunborn_tool,
    compute_context_digest,
)


class TestSunbornClient(unittest.TestCase):
    def test_client_init_requires_key(self):
        with patch.dict("os.environ", {}, clear=True):
            with self.assertRaises(ValueError):
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

    def test_client_init_normalizes_base_url(self):
        client1 = SunbornClient(base_url="http://localhost:8080", api_key="test-key")
        self.assertEqual(client1.base_url, "http://localhost:8080/v1")

        client2 = SunbornClient(base_url="http://localhost:8080/v1/", api_key="test-key")
        self.assertEqual(client2.base_url, "http://localhost:8080/v1")

    @patch("sunborn_agent.urllib.request.urlopen")
    def test_chat_completion_request_payload_full_options(self, mock_urlopen):
        mock_response = MagicMock()
        mock_response.read.return_value = b'{"choices": [{"message": {"content": "ok"}}]}'
        mock_response.__enter__.return_value = mock_response
        mock_urlopen.return_value = mock_response

        client = SunbornClient(api_key="test-key")
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
        self.assertIn("choices", res)
        self.assertEqual(res["choices"][0]["message"]["content"], "ok")

        # Verify request call arguments
        req = mock_urlopen.call_args[0][0]
        self.assertEqual(req.headers.get("X-sunborn-cache-policy"), "allow-stochastic")
        self.assertEqual(req.headers.get("X-sunborn-context-digest"), "abcd1234efgh5678")

    @patch("sunborn_agent.urllib.request.urlopen")
    def test_anthropic_messages_payload(self, mock_urlopen):
        mock_response = MagicMock()
        mock_response.read.return_value = b'{"id": "msg_1", "content": [{"type": "text", "text": "hello"}]}'
        mock_response.__enter__.return_value = mock_response
        mock_urlopen.return_value = mock_response

        client = SunbornClient(api_key="test-key")
        res = client.create_message(
            model="claude-3-5-sonnet-20241022",
            messages=[{"role": "user", "content": "Hi"}],
            system="System instructions",
            max_tokens=500,
            temperature=0.2,
            stop_sequences=["STOP"],
        )
        self.assertEqual(res["id"], "msg_1")
        self.assertEqual(res["content"][0]["text"], "hello")

    def test_compute_context_digest_parity(self):
        cd1 = compute_context_digest("gpt-4o", "sys", ["chunk_b", "chunk_a"])
        cd2 = compute_context_digest("gpt-4o", "sys", ["chunk_a", "chunk_b"])
        # Invariant: Merkle root over sorted chunk IDs must produce identical digest regardless of input order
        self.assertEqual(cd1, cd2)
        self.assertEqual(len(cd1), 64)

        # Invariant: Different model or chunk must change digest
        cd3 = compute_context_digest("gpt-4o-mini", "sys", ["chunk_a", "chunk_b"])
        self.assertNotEqual(cd1, cd3)

        cd4 = compute_context_digest("gpt-4o", "sys", ["chunk_a", "chunk_c"])
        self.assertNotEqual(cd1, cd4)

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

        @sunborn_tool(name="strict_tool", policy=FallbackPolicy.FAIL_CLOSED, client=client)
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

        result = lax_calc()
        self.assertEqual(result, 200)

    def test_tool_decorator_complex_types(self):
        client = MagicMock(spec=SunbornClient)
        client.plan_execution_dag.return_value = {"cached_nodes": 0, "node_statuses": {}}

        @sunborn_tool(name="complex_tool", client=client)
        def process_record(u_id, dt):
            return f"{u_id}-{dt.year}"

        u = uuid.uuid4()
        now = datetime.datetime.now()
        out = process_record(u, now)
        self.assertEqual(out, f"{u}-{now.year}")

    def test_tool_decorator_non_deterministic(self):
        client = MagicMock(spec=SunbornClient)

        @sunborn_tool(name="random_tool", deterministic=False, client=client)
        def random_action():
            return 999

        self.assertEqual(random_action(), 999)
        client.plan_execution_dag.assert_not_called()
        client.record_tool_result.assert_not_called()

    @patch("sunborn_agent.urllib.request.urlopen")
    def test_error_handling_http_error(self, mock_urlopen):
        err = urllib.error.HTTPError("http://localhost:8080/v1/fail", 401, "Unauthorized", {}, MagicMock(read=lambda: b'{"error":"invalid_api_key"}'))
        mock_urlopen.side_effect = err

        client = SunbornClient(api_key="bad-key")
        with self.assertRaises(RuntimeError) as ctx:
            client.get_finops_report()
        self.assertIn("401", str(ctx.exception))
        self.assertIn("invalid_api_key", str(ctx.exception))

    @patch("sunborn_agent.urllib.request.urlopen")
    def test_error_handling_connection_error(self, mock_urlopen):
        err = urllib.error.URLError("Connection refused")
        mock_urlopen.side_effect = err

        client = SunbornClient(api_key="key")
        with self.assertRaises(ConnectionError) as ctx:
            client.get_finops_report()
        self.assertIn("Failed to connect", str(ctx.exception))

    @patch("sunborn_agent.urllib.request.urlopen")
    def test_async_sunborn_client(self, mock_urlopen):
        mock_response = MagicMock()
        mock_response.read.return_value = b'{"choices": [{"message": {"content": "async_ok"}}]}'
        mock_response.__enter__.return_value = mock_response
        mock_urlopen.return_value = mock_response

        client = AsyncSunbornClient(api_key="test-key")
        res = asyncio.run(client.chat_completion(model="gpt-4o", prompt="Async test"))
        self.assertEqual(res["choices"][0]["message"]["content"], "async_ok")


if __name__ == "__main__":
    unittest.main()
