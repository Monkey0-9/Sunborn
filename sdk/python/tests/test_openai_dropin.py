"""
Unit tests for Sunborn drop-in OpenAI and Anthropic clients.
"""

import io
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch
from urllib.error import HTTPError

_sdk_root = str(Path(__file__).resolve().parent.parent)
if _sdk_root not in sys.path:
    sys.path.insert(0, _sdk_root)

from sunborn.anthropic import Anthropic, MessageResponse, SunbornAnthropic
from sunborn.exceptions import SunbornAuthError, SunbornRateLimitError, SunbornServerError
from sunborn.openai import (
    APIError,
    AuthenticationError,
    ChatCompletion,
    ChatCompletionMessageToolCall,
    OpenAI,
    OpenAIError,
    RateLimitError,
    SunbornOpenAI,
)


class TestOpenAIDropinClient(unittest.TestCase):
    def test_client_init_and_aliases(self):
        # Base URL normalization to /v1
        client1 = OpenAI(api_key="test-key", base_url="http://test-gateway:8080", tenant_id="acme-corp")
        self.assertEqual(client1.api_key, "test-key")
        self.assertEqual(client1.base_url, "http://test-gateway:8080/v1")
        self.assertEqual(client1.tenant_id, "acme-corp")
        self.assertIs(OpenAI, SunbornOpenAI)

        # Context manager and close
        with OpenAI(api_key="key-ctx") as ctx_client:
            self.assertFalse(ctx_client.is_closed)
        self.assertTrue(ctx_client.is_closed)

    @patch("urllib.request.urlopen")
    def test_chat_completions_create_success(self, mock_urlopen):
        mock_resp = MagicMock()
        mock_resp.read.return_value = json.dumps(
            {
                "id": "chatcmpl-12345",
                "object": "chat.completion",
                "created": 1677652288,
                "model": "gpt-4o",
                "sunborn_cached": True,
                "sunborn_tier": "c1_exact",
                "choices": [
                    {
                        "index": 0,
                        "message": {"role": "assistant", "content": "Semantic caching reduces latency by 90%."},
                        "finish_reason": "stop",
                    }
                ],
                "usage": {"prompt_tokens": 12, "completion_tokens": 10, "total_tokens": 22},
            }
        ).encode("utf-8")
        mock_urlopen.return_value.__enter__.return_value = mock_resp

        client = OpenAI(api_key="sk-test", base_url="http://localhost:8080/v1")
        res = client.chat.completions.create(
            model="gpt-4o",
            messages=[{"role": "user", "content": "What is semantic caching?"}],
            temperature=0.0,
            top_p=0.9,
            seed=42,
            sunborn_cache_policy="exact_only",
            sunborn_context_digest="0123456789abcdef",
        )

        self.assertIsInstance(res, ChatCompletion)
        self.assertEqual(res.id, "chatcmpl-12345")
        self.assertEqual(res.model, "gpt-4o")
        self.assertTrue(res.sunborn_cached)
        self.assertEqual(res.sunborn_tier, "c1_exact")
        self.assertEqual(len(res.choices), 1)
        self.assertEqual(res.choices[0].message.role, "assistant")
        self.assertEqual(res.choices[0].message.content, "Semantic caching reduces latency by 90%.")
        self.assertEqual(res.usage.total_tokens, 22)

        # Dict subscripting compatibility
        self.assertEqual(res["id"], "chatcmpl-12345")
        self.assertEqual(res.get("model"), "gpt-4o")
        self.assertEqual(res["choices"][0]["message"]["content"], "Semantic caching reduces latency by 90%.")

        # Pydantic v1/v2 compatibility
        d = res.model_dump()
        self.assertIsInstance(d, dict)
        self.assertEqual(d["id"], "chatcmpl-12345")
        self.assertEqual(res.dict()["usage"]["total_tokens"], 22)
        self.assertIn("chatcmpl-12345", res.model_dump_json())

    @patch("urllib.request.urlopen")
    def test_chat_completions_tool_calls(self, mock_urlopen):
        mock_resp = MagicMock()
        mock_resp.read.return_value = json.dumps(
            {
                "id": "chatcmpl-tools",
                "object": "chat.completion",
                "created": 1677652288,
                "model": "gpt-4o",
                "choices": [
                    {
                        "index": 0,
                        "message": {
                            "role": "assistant",
                            "content": None,
                            "tool_calls": [
                                {
                                    "id": "call_abc123",
                                    "type": "function",
                                    "function": {
                                        "name": "lookup_weather",
                                        "arguments": '{"location": "Tokyo"}',
                                    },
                                }
                            ],
                        },
                        "finish_reason": "tool_calls",
                    }
                ],
            }
        ).encode("utf-8")
        mock_urlopen.return_value.__enter__.return_value = mock_resp

        client = OpenAI(api_key="sk-test")
        res = client.chat.completions.create(
            model="gpt-4o",
            messages=[{"role": "user", "content": "Weather in Tokyo?"}],
            tools=[{"type": "function", "function": {"name": "lookup_weather"}}],
        )

        msg = res.choices[0].message
        self.assertIsNone(msg.content)
        self.assertIsNotNone(msg.tool_calls)
        self.assertEqual(len(msg.tool_calls), 1)
        self.assertIsInstance(msg.tool_calls[0], ChatCompletionMessageToolCall)
        self.assertEqual(msg.tool_calls[0].id, "call_abc123")
        self.assertEqual(msg.tool_calls[0].function.name, "lookup_weather")
        self.assertEqual(msg.tool_calls[0].function.arguments, '{"location": "Tokyo"}')
        self.assertEqual(res.choices[0].finish_reason, "tool_calls")

    @patch("urllib.request.urlopen")
    def test_models_and_embeddings_resource(self, mock_urlopen):
        # Models list fallback
        client = OpenAI(api_key="sk-test")
        mock_resp = MagicMock()
        mock_resp.read.return_value = json.dumps(
            {
                "data": [
                    {"id": "gpt-4o", "owned_by": "openai"},
                    {"id": "claude-3-5-sonnet", "owned_by": "anthropic"},
                ]
            }
        ).encode("utf-8")
        mock_urlopen.return_value.__enter__.return_value = mock_resp

        models = client.models.list()
        self.assertGreaterEqual(len(models), 2)
        self.assertEqual(models[0].id, "gpt-4o")

        # Embeddings create
        emb_resp = MagicMock()
        emb_resp.read.return_value = json.dumps(
            {
                "data": [{"index": 0, "embedding": [0.1, 0.2, 0.3]}],
                "model": "text-embedding-3-small",
                "usage": {"total_tokens": 5},
            }
        ).encode("utf-8")
        mock_urlopen.return_value.__enter__.return_value = emb_resp

        emb = client.embeddings.create(model="text-embedding-3-small", input=["test"])
        self.assertEqual(len(emb.data), 1)
        self.assertEqual(emb.data[0].embedding, [0.1, 0.2, 0.3])
        self.assertEqual(emb.model, "text-embedding-3-small")

    @patch("urllib.request.urlopen")
    def test_chat_completions_error_mapping(self, mock_urlopen):
        # 401 Unauthorized
        err_401 = HTTPError(
            url="http://localhost:8080/v1/chat/completions",
            code=401,
            msg="Unauthorized",
            hdrs={},
            fp=io.BytesIO(b'{"error": "Invalid API key"}'),
        )
        mock_urlopen.side_effect = err_401
        client = OpenAI()
        with self.assertRaises(AuthenticationError):
            client.chat.completions.create(model="gpt-4o", messages=[{"role": "user", "content": "test"}])

        # 429 Rate Limit
        err_429 = HTTPError(
            url="http://localhost:8080/v1/chat/completions",
            code=429,
            msg="Too Many Requests",
            hdrs={},
            fp=io.BytesIO(b'{"error": "Rate limit exceeded"}'),
        )
        mock_urlopen.side_effect = err_429
        with self.assertRaises(RateLimitError):
            client.chat.completions.create(model="gpt-4o", messages=[{"role": "user", "content": "test"}])

    def test_openai_exception_aliases(self):
        self.assertTrue(issubclass(AuthenticationError, OpenAIError))
        self.assertTrue(issubclass(RateLimitError, OpenAIError))
        self.assertTrue(issubclass(APIError, OpenAIError))


class TestAnthropicDropinClient(unittest.TestCase):
    def test_client_init_and_aliases(self):
        client = Anthropic(api_key="test-anthropic-key", base_url="http://test-gateway:8080/v1")
        self.assertEqual(client.api_key, "test-anthropic-key")
        self.assertIs(Anthropic, SunbornAnthropic)

    @patch("urllib.request.urlopen")
    def test_messages_create_success(self, mock_urlopen):
        mock_resp = MagicMock()
        mock_resp.read.return_value = json.dumps(
            {
                "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
                "type": "message",
                "role": "assistant",
                "model": "claude-3-5-sonnet-20241022",
                "content": [{"type": "text", "text": "Hello! I am Claude through Sunborn Gateway."}],
                "stop_reason": "end_turn",
                "sunborn_cached": True,
                "sunborn_tier": "c1_exact",
                "usage": {"input_tokens": 15, "output_tokens": 12},
            }
        ).encode("utf-8")
        mock_urlopen.return_value.__enter__.return_value = mock_resp

        client = Anthropic(api_key="sk-ant-test")
        msg = client.messages.create(
            model="claude-3-5-sonnet-20241022", messages=[{"role": "user", "content": "Hello!"}], max_tokens=100
        )

        self.assertIsInstance(msg, MessageResponse)
        self.assertEqual(msg.id, "msg_01XFDUDYJgAACzvnptvVoYEL")
        self.assertTrue(msg.sunborn_cached)
        self.assertEqual(len(msg.content), 1)
        self.assertEqual(msg.content[0].text, "Hello! I am Claude through Sunborn Gateway.")
        self.assertEqual(msg.usage.input_tokens, 15)


if __name__ == "__main__":
    unittest.main()
