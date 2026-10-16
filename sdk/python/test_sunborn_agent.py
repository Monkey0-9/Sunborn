import os
import sys
import unittest
from unittest.mock import MagicMock, patch

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from sunborn_agent import SunbornClient, FallbackPolicy, sunborn_tool


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

    @patch("sunborn_agent.urllib.request.urlopen")
    def test_chat_completion_request_payload(self, mock_urlopen):
        mock_response = MagicMock()
        mock_response.read.return_value = b'{"choices": [{"message": {"content": "ok"}}]}'
        mock_response.__enter__.return_value = mock_response
        mock_urlopen.return_value = mock_response

        client = SunbornClient(api_key="test-key")
        res = client.chat_completion(
            model="gpt-4o",
            prompt="Hello Sunborn",
            system_prompt="You are helpful.",
        )
        self.assertIn("choices", res)
        self.assertEqual(res["choices"][0]["message"]["content"], "ok")

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

        # Mocking the node_id logic
        with patch("sunborn_agent.hashlib.sha256") as mock_sha:
            mock_hasher = MagicMock()
            mock_hasher.hexdigest.return_value = "f2a89c4b123456"
            mock_sha.return_value = mock_hasher

            @sunborn_tool(name="test_func", client=client)
            def compute(x):
                return x * 2

            val = compute(21)
            self.assertEqual(val, 42)
            # Underlying compute shouldn't have recorded because it hit the cache
            client.record_tool_result.assert_not_called()

    def test_tool_decorator_miss_executes_and_records(self):
        client = MagicMock(spec=SunbornClient)
        client.plan_execution_dag.return_value = {
            "cached_nodes": 0,
            "node_statuses": {},
        }

        @sunborn_tool(name="add_nums", client=client)
        def add(a, b):
            return a + b

        result = add(10, 20)
        self.assertEqual(result, 30)
        client.record_tool_result.assert_called_once()
        _, kwargs = client.record_tool_result.call_args
        self.assertEqual(kwargs["tool_name"], "add_nums")
        self.assertEqual(kwargs["output"], 30)


if __name__ == "__main__":
    unittest.main()
