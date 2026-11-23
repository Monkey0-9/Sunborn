"""
Unit tests for Async OpenAI, Async Anthropic, SSE Streaming, and Async Agent Integrations.
"""

import asyncio
import io
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import MagicMock, patch

_sdk_root = str(Path(__file__).resolve().parent.parent)
if _sdk_root not in sys.path:
    sys.path.insert(0, _sdk_root)

from sunborn.anthropic import (
    Anthropic,
    AsyncAnthropic,
    AsyncSunbornAnthropic,
    MessageResponse,
    MessageStreamEvent,
    SunbornAnthropic,
)
from sunborn.integrations.langchain import SunbornLangChainCache
from sunborn.integrations.llamaindex import SunbornLlamaIndexCache
from sunborn.openai import (
    AsyncOpenAI,
    AsyncSunbornOpenAI,
    ChatCompletion,
    ChatCompletionChunk,
    OpenAI,
    SunbornOpenAI,
)


class TestAsyncAndStreaming(unittest.TestCase):
    def test_async_openai_init_and_aliases(self):
        client = AsyncOpenAI(api_key="async-key", base_url="http://test:8080/v1", tenant_id="tenant-async")
        self.assertEqual(client.api_key, "async-key")
        self.assertEqual(client.base_url, "http://test:8080/v1")
        self.assertEqual(client.tenant_id, "tenant-async")
        self.assertIs(AsyncOpenAI, AsyncSunbornOpenAI)

    def test_async_anthropic_init_and_aliases(self):
        client = AsyncAnthropic(api_key="async-anthro-key", base_url="http://test:8080/v1", tenant_id="tenant-anthro")
        self.assertEqual(client.api_key, "async-anthro-key")
        self.assertEqual(client.base_url, "http://test:8080/v1")
        self.assertEqual(client.tenant_id, "tenant-anthro")
        self.assertIs(AsyncAnthropic, AsyncSunbornAnthropic)

    @patch("urllib.request.urlopen")
    def test_sync_openai_streaming_chunks(self, mock_urlopen):
        mock_resp = MagicMock()
        mock_resp.headers = {"Content-Type": "text/event-stream"}
        mock_resp.__iter__.return_value = [
            b"data: "
            + json.dumps(
                {
                    "id": "chunk-1",
                    "object": "chat.completion.chunk",
                    "created": 123456,
                    "model": "gpt-4o",
                    "choices": [
                        {"index": 0, "delta": {"role": "assistant", "content": "Hello"}, "finish_reason": None}
                    ],
                }
            ).encode("utf-8")
            + b"\n\n",
            b"data: "
            + json.dumps(
                {
                    "id": "chunk-2",
                    "object": "chat.completion.chunk",
                    "created": 123456,
                    "model": "gpt-4o",
                    "choices": [{"index": 0, "delta": {"content": " world!"}, "finish_reason": "stop"}],
                }
            ).encode("utf-8")
            + b"\n\n",
            b"data: [DONE]\n\n",
        ]
        mock_urlopen.return_value = mock_resp

        client = OpenAI(api_key="sk-test", base_url="http://localhost:8080/v1")
        stream = client.chat.completions.create(
            model="gpt-4o",
            messages=[{"role": "user", "content": "hi"}],
            stream=True,
        )

        chunks = list(stream)
        self.assertEqual(len(chunks), 2)
        self.assertIsInstance(chunks[0], ChatCompletionChunk)
        self.assertEqual(chunks[0].choices[0].delta.content, "Hello")
        self.assertEqual(chunks[1].choices[0].delta.content, " world!")
        self.assertEqual(chunks[1].choices[0].finish_reason, "stop")

    @patch("urllib.request.urlopen")
    def test_sync_anthropic_streaming_events(self, mock_urlopen):
        mock_resp = MagicMock()
        mock_resp.headers = {"Content-Type": "text/event-stream"}
        mock_resp.__iter__.return_value = [
            b'data: {"type": "content_block_delta", "delta": {"text": "Sunborn"}}\n\n',
            b'data: {"type": "content_block_delta", "delta": {"text": " Gateway"}}\n\n',
            b"data: [DONE]\n\n",
        ]
        mock_urlopen.return_value = mock_resp

        client = Anthropic(api_key="sk-ant", base_url="http://localhost:8080/v1")
        stream = client.messages.create(
            model="claude-3-5-sonnet-20241022",
            messages=[{"role": "user", "content": "hi"}],
            stream=True,
        )

        events = list(stream)
        self.assertEqual(len(events), 2)
        self.assertIsInstance(events[0], MessageStreamEvent)
        self.assertEqual(events[0].text, "Sunborn")
        self.assertEqual(events[1].text, " Gateway")

    def test_async_openai_create(self):
        async def _run():
            with patch("urllib.request.urlopen") as mock_urlopen:
                mock_resp = MagicMock()
                mock_resp.read.return_value = json.dumps(
                    {
                        "id": "chatcmpl-async",
                        "object": "chat.completion",
                        "created": 1677652288,
                        "model": "gpt-4o",
                        "sunborn_cached": True,
                        "sunborn_tier": "c1_exact",
                        "choices": [
                            {
                                "index": 0,
                                "message": {"role": "assistant", "content": "Async success"},
                                "finish_reason": "stop",
                            }
                        ],
                        "usage": {"total_tokens": 15},
                    }
                ).encode("utf-8")
                mock_urlopen.return_value.__enter__.return_value = mock_resp

                client = AsyncOpenAI(api_key="test-key")
                res = await client.chat.completions.create(
                    model="gpt-4o", messages=[{"role": "user", "content": "test async"}]
                )
                self.assertIsInstance(res, ChatCompletion)
                self.assertEqual(res.choices[0].message.content, "Async success")
                self.assertTrue(res.sunborn_cached)

        asyncio.run(_run())

    def test_async_agent_integrations(self):
        async def _run():
            # LangChain async
            lc_cache = SunbornLangChainCache(api_key="sk-test")
            lc_node_id, _, _ = lc_cache._plan("test prompt", "llm-str")
            with (
                patch.object(lc_cache.client, "plan_execution_dag") as mock_plan,
                patch.object(lc_cache.client, "record_tool_result") as mock_record,
            ):
                mock_plan.return_value = {
                    "cached_nodes": 1,
                    "node_statuses": {
                        lc_node_id: {
                            "status": "Cached",
                            "details": {"output": {"texts": ["Cached agent answer"]}},
                        }
                    },
                }
                res = await lc_cache.alookup("test prompt", "llm-str")
                self.assertIsNotNone(res)
                self.assertEqual(res[0].text, "Cached agent answer")

                await lc_cache.aupdate("test prompt", "llm-str", res)
                mock_record.assert_called()

            # LlamaIndex async
            li_cache = SunbornLlamaIndexCache(api_key="sk-test")
            tool_name = li_cache._tool_name("query key")
            node_id = f"node-{tool_name}"
            with (
                patch.object(li_cache.client, "plan_execution_dag") as mock_plan,
                patch.object(li_cache.client, "record_tool_result") as mock_record,
                patch.object(li_cache.client, "evict_execution") as mock_evict,
            ):
                mock_plan.return_value = {
                    "cached_nodes": 1,
                    "node_statuses": {
                        node_id: {
                            "status": "Cached",
                            "details": {"output": {"value": "Llama cached"}},
                        }
                    },
                }
                val = await li_cache.aget("query key")
                self.assertEqual(val, "Llama cached")

                await li_cache.aput("query key", "Llama cached")
                mock_record.assert_called()

                deleted = await li_cache.adelete("query key")
                self.assertTrue(deleted)
                mock_evict.assert_called()

        asyncio.run(_run())

    @patch("urllib.request.urlopen")
    def test_sync_stream_context_manager_and_json_fallback(self, mock_urlopen):
        # 1. Non-SSE JSON fallback (when cached hit returned as application/json)
        mock_resp = MagicMock()
        mock_resp.headers = {"Content-Type": "application/json"}
        mock_resp.read.return_value = json.dumps(
            {
                "id": "chatcmpl-cached-full",
                "object": "chat.completion",
                "model": "gpt-4o",
                "sunborn_cached": True,
                "sunborn_tier": "c1_exact",
                "choices": [
                    {
                        "index": 0,
                        "message": {"role": "assistant", "content": "Full cached stream fallback"},
                        "finish_reason": "stop",
                    }
                ],
            }
        ).encode("utf-8")
        mock_urlopen.return_value = mock_resp

        client = OpenAI(api_key="sk-test")
        with client.chat.completions.create(
            model="gpt-4o", messages=[{"role": "user", "content": "hi"}], stream=True
        ) as stream:
            chunks = list(stream)
            self.assertEqual(len(chunks), 1)
            self.assertEqual(chunks[0].choices[0].delta.content, "Full cached stream fallback")
            self.assertEqual(chunks[0].choices[0].finish_reason, "stop")

    def test_async_stream_and_context_manager(self):
        async def _run():
            with patch("urllib.request.urlopen") as mock_urlopen:
                mock_resp = MagicMock()
                mock_resp.headers = {"Content-Type": "text/event-stream"}
                mock_resp.__iter__.return_value = [
                    b'data: {"id": "c1", "choices": [{"index": 0, "delta": {"content": "Async"}}]}\n\n',
                    b'data: {"id": "c2", "choices": [{"index": 0, "delta": {"content": " Stream"}}]}\n\n',
                    b"data: [DONE]\n\n",
                ]
                mock_urlopen.return_value = mock_resp

                async with AsyncOpenAI(api_key="async-key") as client:
                    self.assertFalse(client.is_closed)
                    # Test both await and async with
                    stream = await client.chat.completions.create(
                        model="gpt-4o", messages=[{"role": "user", "content": "hi"}], stream=True
                    )
                    texts = []
                    async with stream as active_stream:
                        async for chunk in active_stream:
                            texts.append(chunk.choices[0].delta.content)
                    self.assertEqual("".join(texts), "Async Stream")

                    # Models and embeddings on AsyncOpenAI
                    models = await client.models.list()
                    self.assertGreaterEqual(len(models), 1)

                    with patch.object(client._sync_client, "_post") as mock_post:
                        mock_post.return_value = {
                            "data": [{"index": 0, "embedding": [0.0, 1.0]}],
                            "model": "text-embedding-3-small",
                        }
                        emb = await client.embeddings.create(model="text-embedding-3-small", input=["test"])
                        self.assertEqual(emb.data[0].embedding, [0.0, 1.0])

        asyncio.run(_run())


if __name__ == "__main__":
    unittest.main()
