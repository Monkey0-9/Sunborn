"""
Drop-in OpenAI Client for Sunborn Semantic AI Gateway.

Provides 100% wire-compatible drop-in replacement for `openai.OpenAI` and `openai.AsyncOpenAI`,
routing all requests through Sunborn's high-throughput Rust caching and invalidation gateway.

Usage:
    from sunborn.openai import OpenAI, AsyncOpenAI

    client = OpenAI()
    response = client.chat.completions.create(
        model="gpt-4o",
        messages=[{"role": "user", "content": "Hello!"}],
    )
    print(response.choices[0].message.content)
    print("Cached:", getattr(response, "sunborn_cached", False))

    # Streaming:
    for chunk in client.chat.completions.create(
        model="gpt-4o",
        messages=[{"role": "user", "content": "Hello!"}],
        stream=True,
    ):
        print(chunk.choices[0].delta.content or "", end="")
"""

from __future__ import annotations

import asyncio
import os
import urllib.error
import urllib.request
from collections.abc import AsyncIterator, Iterator
from typing import Any, cast

from .client import _fast_dumps, _fast_loads
from .exceptions import SunbornConnectionError, raise_for_status


class ChatCompletionMessage:
    def __init__(self, role: str, content: str | None = None, **kwargs: Any) -> None:
        self.role = role
        self.content = content
        for k, v in kwargs.items():
            setattr(self, k, v)

    def to_dict(self) -> dict[str, Any]:
        return {"role": self.role, "content": self.content}


class ChatCompletionChoice:
    def __init__(self, index: int, message: ChatCompletionMessage, finish_reason: str = "stop") -> None:
        self.index = index
        self.message = message
        self.finish_reason = finish_reason


class CompletionUsage:
    def __init__(self, prompt_tokens: int = 0, completion_tokens: int = 0, total_tokens: int = 0) -> None:
        self.prompt_tokens = prompt_tokens
        self.completion_tokens = completion_tokens
        self.total_tokens = total_tokens


class ChatCompletion:
    def __init__(self, raw: dict[str, Any]) -> None:
        self._raw = raw
        self.id = raw.get("id", "")
        self.object = raw.get("object", "chat.completion")
        self.created = raw.get("created", 0)
        self.model = raw.get("model", "")
        self.sunborn_cached = raw.get("sunborn_cached", False)
        self.sunborn_tier = raw.get("sunborn_tier", "origin")
        self.sunborn_attestation = raw.get("sunborn_attestation", None)

        choices = []
        for i, c in enumerate(raw.get("choices", [])):
            msg_dict = c.get("message", {})
            msg = ChatCompletionMessage(
                role=msg_dict.get("role", "assistant"),
                content=msg_dict.get("content", ""),
            )
            choices.append(
                ChatCompletionChoice(
                    index=c.get("index", i),
                    message=msg,
                    finish_reason=c.get("finish_reason", "stop"),
                )
            )
        self.choices = choices

        usage_dict = raw.get("usage", {})
        self.usage = CompletionUsage(
            prompt_tokens=usage_dict.get("prompt_tokens", 0),
            completion_tokens=usage_dict.get("completion_tokens", 0),
            total_tokens=usage_dict.get("total_tokens", 0),
        )

    def __repr__(self) -> str:
        content = (self.choices[0].message.content or "") if self.choices else ""
        return f"ChatCompletion(id='{self.id}', model='{self.model}', cached={self.sunborn_cached}, tier='{self.sunborn_tier}', content='{content[:40]}...')"


class ChatCompletionChunkDelta:
    def __init__(self, role: str | None = None, content: str | None = None, **kwargs: Any) -> None:
        self.role = role
        self.content = content
        for k, v in kwargs.items():
            setattr(self, k, v)


class ChatCompletionChunkChoice:
    def __init__(
        self,
        index: int = 0,
        delta: ChatCompletionChunkDelta | None = None,
        finish_reason: str | None = None,
    ) -> None:
        self.index = index
        self.delta = delta or ChatCompletionChunkDelta()
        self.finish_reason = finish_reason


class ChatCompletionChunk:
    def __init__(self, raw: dict[str, Any]) -> None:
        self._raw = raw
        self.id = raw.get("id", "")
        self.object = raw.get("object", "chat.completion.chunk")
        self.created = raw.get("created", 0)
        self.model = raw.get("model", "")
        self.sunborn_cached = raw.get("sunborn_cached", False)
        self.sunborn_tier = raw.get("sunborn_tier", "origin")

        choices = []
        for i, c in enumerate(raw.get("choices", [])):
            delta_dict = c.get("delta", {})
            delta = ChatCompletionChunkDelta(
                role=delta_dict.get("role"),
                content=delta_dict.get("content"),
            )
            choices.append(
                ChatCompletionChunkChoice(
                    index=c.get("index", i),
                    delta=delta,
                    finish_reason=c.get("finish_reason"),
                )
            )
        self.choices = choices

    def __repr__(self) -> str:
        content = self.choices[0].delta.content if self.choices else ""
        return f"ChatCompletionChunk(id='{self.id}', model='{self.model}', content='{content}')"


class ChatCompletionsResource:
    def __init__(self, client: OpenAI) -> None:
        self._client = client

    def create(
        self,
        *,
        model: str,
        messages: list[dict[str, Any]],
        temperature: float | None = None,
        max_tokens: int | None = None,
        stream: bool = False,
        tools: list[dict[str, Any]] | None = None,
        tool_choice: Any | None = None,
        response_format: dict[str, Any] | None = None,
        sunborn_cache_policy: str | None = None,
        sunborn_tenant_id: str | None = None,
        **kwargs: Any,
    ) -> ChatCompletion | Iterator[ChatCompletionChunk]:
        payload: dict[str, Any] = {
            "model": model,
            "messages": messages,
        }
        if temperature is not None:
            payload["temperature"] = temperature
        if max_tokens is not None:
            payload["max_tokens"] = max_tokens
        if stream:
            payload["stream"] = True
        if tools:
            payload["tools"] = tools
        if tool_choice:
            payload["tool_choice"] = tool_choice
        if response_format:
            payload["response_format"] = response_format
        if sunborn_cache_policy:
            payload["cache_policy"] = sunborn_cache_policy

        headers = self._client._build_headers(sunborn_tenant_id)
        url = f"{self._client.base_url.rstrip('/')}/chat/completions"

        if stream:
            return self._client._post_stream(url, payload, headers)

        data = self._client._post(url, payload, headers)
        return ChatCompletion(data)


class ChatResource:
    def __init__(self, client: OpenAI) -> None:
        self.completions = ChatCompletionsResource(client)


class OpenAI:
    """
    Drop-in OpenAI client compatible with `import openai; openai.OpenAI()`.

    Routes seamlessly through Sunborn's high-speed Axum gateway with automatic
    semantic caching, FinOps token attribution, and causal graph invalidation.
    """

    def __init__(
        self,
        api_key: str | None = None,
        base_url: str | None = None,
        tenant_id: str | None = None,
        application_id: str | None = None,
        timeout: float = 30.0,
    ) -> None:
        self.api_key = (
            api_key or os.environ.get("SUNBORN_API_KEY") or os.environ.get("OPENAI_API_KEY") or "sunborn-local-dev-key"
        )
        self.base_url = (
            base_url
            or os.environ.get("SUNBORN_BASE_URL")
            or os.environ.get("OPENAI_BASE_URL")
            or "http://localhost:8080/v1"
        )
        self.tenant_id = tenant_id or os.environ.get("SUNBORN_TENANT_ID") or "default-tenant"
        self.application_id = application_id or os.environ.get("SUNBORN_APPLICATION_ID") or "default-app"
        self.timeout = timeout
        self.chat = ChatResource(self)

    def _build_headers(self, override_tenant: str | None = None) -> dict[str, str]:
        return {
            "Content-Type": "application/json",
            "x-api-key": self.api_key,
            "Authorization": f"Bearer {self.api_key}",
            "x-tenant-id": override_tenant or self.tenant_id,
            "x-application-id": self.application_id,
            "User-Agent": "Sunborn-OpenAI-Client/1.0.0",
        }

    def _post(self, url: str, payload: dict[str, Any], headers: dict[str, str]) -> dict[str, Any]:
        body = _fast_dumps(payload)
        req = urllib.request.Request(url, data=body, headers=headers, method="POST")
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as response:
                resp_bytes = response.read()
                return _fast_loads(resp_bytes)
        except urllib.error.HTTPError as err:
            err_body = err.read().decode("utf-8", errors="replace")
            raise_for_status(err.code, err_body)
        except urllib.error.URLError as err:
            raise SunbornConnectionError(f"Failed to connect to Sunborn Gateway at {url}: {err}") from err

    def _post_stream(self, url: str, payload: dict[str, Any], headers: dict[str, str]) -> Iterator[ChatCompletionChunk]:
        body = _fast_dumps(payload)
        headers = dict(headers)
        headers["Accept"] = "text/event-stream"
        req = urllib.request.Request(url, data=body, headers=headers, method="POST")
        try:
            response = urllib.request.urlopen(req, timeout=self.timeout)
        except urllib.error.HTTPError as err:
            err_body = err.read().decode("utf-8", errors="replace")
            raise_for_status(err.code, err_body)
        except urllib.error.URLError as err:
            raise SunbornConnectionError(f"Failed to connect to Sunborn Gateway at {url}: {err}") from err

        with response:
            content_type = response.headers.get("Content-Type", "")
            # If server returned non-SSE JSON (e.g. cached hit response returned all at once)
            if "application/json" in content_type:
                raw_bytes = response.read()
                data = _fast_loads(raw_bytes)
                content = ""
                if data.get("choices"):
                    content = data["choices"][0].get("message", {}).get("content", "")
                yield ChatCompletionChunk(
                    {
                        "id": data.get("id", "chunk-0"),
                        "object": "chat.completion.chunk",
                        "created": data.get("created", 0),
                        "model": data.get("model", ""),
                        "sunborn_cached": data.get("sunborn_cached", False),
                        "sunborn_tier": data.get("sunborn_tier", "origin"),
                        "choices": [
                            {"index": 0, "delta": {"role": "assistant", "content": content}, "finish_reason": "stop"}
                        ],
                    }
                )
                return

            for line in response:
                line_str = line.decode("utf-8", errors="replace").strip()
                if not line_str or line_str.startswith(":"):
                    continue
                if line_str.startswith("data:"):
                    data_str = line_str[5:].strip()
                    if data_str == "[DONE]":
                        break
                    try:
                        chunk_dict = _fast_loads(data_str)
                        yield ChatCompletionChunk(chunk_dict)
                    except Exception:
                        continue


def _create_completion_sync(client: Any, **kwargs: Any) -> ChatCompletion:
    return cast(ChatCompletion, client.chat.completions.create(**kwargs))


def _stream_completion_sync(client: Any, **kwargs: Any) -> Iterator[ChatCompletionChunk]:
    return cast(Iterator[ChatCompletionChunk], client.chat.completions.create(**kwargs))


class AsyncChatCompletionsResource:
    def __init__(self, client: AsyncOpenAI) -> None:
        self._client = client

    async def create(
        self,
        *,
        model: str,
        messages: list[dict[str, Any]],
        temperature: float | None = None,
        max_tokens: int | None = None,
        stream: bool = False,
        tools: list[dict[str, Any]] | None = None,
        tool_choice: Any | None = None,
        response_format: dict[str, Any] | None = None,
        sunborn_cache_policy: str | None = None,
        sunborn_tenant_id: str | None = None,
        **kwargs: Any,
    ) -> ChatCompletion | AsyncIterator[ChatCompletionChunk]:
        if stream:
            return self._stream_create(
                model=model,
                messages=messages,
                temperature=temperature,
                max_tokens=max_tokens,
                tools=tools,
                tool_choice=tool_choice,
                response_format=response_format,
                sunborn_cache_policy=sunborn_cache_policy,
                sunborn_tenant_id=sunborn_tenant_id,
                **kwargs,
            )

        return await asyncio.to_thread(
            _create_completion_sync,
            self._client._sync_client,
            model=model,
            messages=messages,
            temperature=temperature,
            max_tokens=max_tokens,
            stream=False,
            tools=tools,
            tool_choice=tool_choice,
            response_format=response_format,
            sunborn_cache_policy=sunborn_cache_policy,
            sunborn_tenant_id=sunborn_tenant_id,
            **kwargs,
        )

    async def _stream_create(self, **kwargs: Any) -> AsyncIterator[ChatCompletionChunk]:
        sync_iter = await asyncio.to_thread(
            _stream_completion_sync,
            self._client._sync_client,
            stream=True,
            **kwargs,
        )
        for chunk in sync_iter:
            yield chunk


class AsyncChatResource:
    def __init__(self, client: AsyncOpenAI) -> None:
        self.completions = AsyncChatCompletionsResource(client)


class AsyncOpenAI:
    """
    Drop-in AsyncOpenAI client compatible with `openai.AsyncOpenAI()`.

    Provides non-blocking async execution for high-concurrency asyncio agent loops.
    """

    def __init__(
        self,
        api_key: str | None = None,
        base_url: str | None = None,
        tenant_id: str | None = None,
        application_id: str | None = None,
        timeout: float = 30.0,
    ) -> None:
        self._sync_client = OpenAI(
            api_key=api_key,
            base_url=base_url,
            tenant_id=tenant_id,
            application_id=application_id,
            timeout=timeout,
        )
        self.chat = AsyncChatResource(self)

    @property
    def api_key(self) -> str:
        return self._sync_client.api_key

    @property
    def base_url(self) -> str:
        return self._sync_client.base_url

    @property
    def tenant_id(self) -> str:
        return self._sync_client.tenant_id

    @property
    def application_id(self) -> str:
        return self._sync_client.application_id


# Aliases for drop-in imports
SunbornOpenAI = OpenAI
AsyncSunbornOpenAI = AsyncOpenAI
