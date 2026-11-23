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

from sunborn.openai import OpenAI, SunbornOpenAI, ChatCompletion
from sunborn.anthropic import Anthropic, SunbornAnthropic, MessageResponse
from sunborn.exceptions import SunbornAuthError, SunbornRateLimitError, SunbornServerError


class TestOpenAIDropinClient(unittest.TestCase):
    def test_client_init_and_aliases(self):
        client = OpenAI(api_key="test-key", base_url="http://test-gateway:8080/v1", tenant_id="acme-corp")
        self.assertEqual(client.api_key, "test-key")
        self.assertEqual(client.base_url, "http://test-gateway:8080/v1")
        self.assertEqual(client.tenant_id, "acme-corp")
        self.assertIs(OpenAI, SunbornOpenAI)

    @patch("urllib.request.urlopen")
    def test_chat_completions_create_success(self, mock_urlopen):
        mock_resp = MagicMock()
        mock_resp.read.return_value = json.dumps({
            "id": "chatcmpl-12345",
            "object": "chat.completion",
            "created": 1677652288,
            "model": "gpt-4o",
            "sunborn_cached": True,
            "sunborn_tier": "c1_exact",
            "choices": [{
                "index": 0,
                "message": {
                    "role": "assistant",
                    "content": "Semantic caching reduces latency by 90%."
                },
                "finish_reason": "stop"
            }],
            "usage": {
                "prompt_tokens": 12,
                "completion_tokens": 10,
                "total_tokens": 22
            }
        }).encode("utf-8")
        mock_urlopen.return_value.__enter__.return_value = mock_resp

        client = OpenAI(api_key="sk-test", base_url="http://localhost:8080/v1")
        res = client.chat.completions.create(
            model="gpt-4o",
            messages=[{"role": "user", "content": "What is semantic caching?"}],
            temperature=0.0,
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

    @patch("urllib.request.urlopen")
    def test_chat_completions_error_mapping(self, mock_urlopen):
        # 401 Unauthorized
        err_401 = HTTPError(
            url="http://localhost:8080/v1/chat/completions",
            code=401,
            msg="Unauthorized",
            hdrs={},
            fp=io.BytesIO(b'{"error": "Invalid API key"}')
        )
        mock_urlopen.side_effect = err_401
        client = OpenAI()
        with self.assertRaises(SunbornAuthError):
            client.chat.completions.create(
                model="gpt-4o",
                messages=[{"role": "user", "content": "test"}]
            )

        # 429 Rate Limit
        err_429 = HTTPError(
            url="http://localhost:8080/v1/chat/completions",
            code=429,
            msg="Too Many Requests",
            hdrs={},
            fp=io.BytesIO(b'{"error": "Rate limit exceeded"}')
        )
        mock_urlopen.side_effect = err_429
        with self.assertRaises(SunbornRateLimitError):
            client.chat.completions.create(
                model="gpt-4o",
                messages=[{"role": "user", "content": "test"}]
            )


class TestAnthropicDropinClient(unittest.TestCase):
    def test_client_init_and_aliases(self):
        client = Anthropic(api_key="test-anthropic-key", base_url="http://test-gateway:8080/v1")
        self.assertEqual(client.api_key, "test-anthropic-key")
        self.assertIs(Anthropic, SunbornAnthropic)

    @patch("urllib.request.urlopen")
    def test_messages_create_success(self, mock_urlopen):
        mock_resp = MagicMock()
        mock_resp.read.return_value = json.dumps({
            "id": "msg_01XFDUDYJgAACzvnptvVoYEL",
            "type": "message",
            "role": "assistant",
            "model": "claude-3-5-sonnet-20241022",
            "content": [{
                "type": "text",
                "text": "Hello! I am Claude through Sunborn Gateway."
            }],
            "stop_reason": "end_turn",
            "sunborn_cached": True,
            "sunborn_tier": "c1_exact",
            "usage": {
                "input_tokens": 15,
                "output_tokens": 12
            }
        }).encode("utf-8")
        mock_urlopen.return_value.__enter__.return_value = mock_resp

        client = Anthropic(api_key="sk-ant-test")
        msg = client.messages.create(
            model="claude-3-5-sonnet-20241022",
            messages=[{"role": "user", "content": "Hello!"}],
            max_tokens=100
        )

        self.assertIsInstance(msg, MessageResponse)
        self.assertEqual(msg.id, "msg_01XFDUDYJgAACzvnptvVoYEL")
        self.assertTrue(msg.sunborn_cached)
        self.assertEqual(len(msg.content), 1)
        self.assertEqual(msg.content[0].text, "Hello! I am Claude through Sunborn Gateway.")
        self.assertEqual(msg.usage.input_tokens, 15)


if __name__ == "__main__":
    unittest.main()
