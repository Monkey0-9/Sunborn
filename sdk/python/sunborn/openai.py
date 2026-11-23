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
import json
import os
import urllib.error
import urllib.request
from collections.abc import AsyncIterator, Callable, Iterator
from typing import Any, NoReturn, cast

from .client import _fast_dumps, _fast_loads
from .exceptions import (
    SunbornAuthError,
    SunbornConnectionError,
    SunbornError,
    SunbornForbiddenError,
    SunbornNotFoundError,
    SunbornRateLimitError,
    SunbornServerError,
    SunbornValidationError,
    raise_for_status,
)

# Standard OpenAI Exception Aliases
OpenAIError = SunbornError
APIError = SunbornError
APIConnectionError = SunbornConnectionError
AuthenticationError = SunbornAuthError
PermissionDeniedError = SunbornForbiddenError
NotFoundError = SunbornNotFoundError
RateLimitError = SunbornRateLimitError
BadRequestError = SunbornValidationError
InternalServerError = SunbornServerError


class FunctionCall:
    """Represents a legacy function call in OpenAI format."""

    def __init__(self, name: str = "", arguments: str = "", **kwargs: Any) -> None:
        self.name = name
        self.arguments = arguments
        for k, v in kwargs.items():
            setattr(self, k, v)

    def to_dict(self) -> dict[str, Any]:
        return {"name": self.name, "arguments": self.arguments}

    def model_dump(self) -> dict[str, Any]:
        return self.to_dict()

    def __repr__(self) -> str:
        return f"FunctionCall(name='{self.name}', arguments='{self.arguments[:30]}...')"


class ChatCompletionMessageToolCall:
    """Represents a tool call produced by a model in OpenAI format."""

    def __init__(
        self,
        id: str = "",
        type: str = "function",
        function: FunctionCall | dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> None:
        self.id = id
        self.type = type
        if isinstance(function, dict):
            self.function = FunctionCall(
                name=function.get("name", ""),
                arguments=function.get("arguments", ""),
            )
        elif isinstance(function, FunctionCall):
            self.function = function
        else:
            self.function = FunctionCall(name="", arguments="")
        for k, v in kwargs.items():
            setattr(self, k, v)

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "type": self.type,
            "function": self.function.to_dict() if hasattr(self.function, "to_dict") else self.function,
        }

    def model_dump(self) -> dict[str, Any]:
        return self.to_dict()

    def __repr__(self) -> str:
        return f"ChatCompletionMessageToolCall(id='{self.id}', type='{self.type}', function={self.function})"


class ChatCompletionMessage:
    """Represents a chat completion message in OpenAI format."""

    def __init__(
        self,
        role: str = "assistant",
        content: str | None = None,
        tool_calls: list[dict[str, Any] | ChatCompletionMessageToolCall] | None = None,
        function_call: dict[str, Any] | FunctionCall | None = None,
        refusal: str | None = None,
        **kwargs: Any,
    ) -> None:
        self.role = role
        self.content = content
        self.refusal = refusal

        self.tool_calls: list[ChatCompletionMessageToolCall] | None = None
        if tool_calls is not None:
            self.tool_calls = [
                tc if isinstance(tc, ChatCompletionMessageToolCall) else ChatCompletionMessageToolCall(**tc)
                for tc in tool_calls
            ]

        self.function_call: FunctionCall | None = None
        if isinstance(function_call, dict):
            self.function_call = FunctionCall(**function_call)
        elif isinstance(function_call, FunctionCall):
            self.function_call = function_call

        for k, v in kwargs.items():
            setattr(self, k, v)

    def to_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {"role": self.role, "content": self.content}
        if self.tool_calls is not None:
            d["tool_calls"] = [tc.to_dict() for tc in self.tool_calls]
        if self.function_call is not None:
            d["function_call"] = self.function_call.to_dict()
        if self.refusal is not None:
            d["refusal"] = self.refusal
        return d

    def model_dump(self) -> dict[str, Any]:
        return self.to_dict()

    def dict(self) -> dict[str, Any]:
        return self.to_dict()

    def __getitem__(self, key: str) -> Any:
        return getattr(self, key)

    def get(self, key: str, default: Any = None) -> Any:
        return getattr(self, key, default)

    def __contains__(self, key: str) -> bool:
        return hasattr(self, key)

    def __repr__(self) -> str:
        text = (self.content or "")[:40]
        return f"ChatCompletionMessage(role='{self.role}', content='{text}...')"


class ChatCompletionChoice:
    """Represents a single completion choice."""

    def __init__(
        self,
        index: int = 0,
        message: ChatCompletionMessage | dict[str, Any] | None = None,
        finish_reason: str | None = "stop",
        logprobs: Any | None = None,
        **kwargs: Any,
    ) -> None:
        self.index = index
        if isinstance(message, dict):
            self.message = ChatCompletionMessage(**message)
        elif isinstance(message, ChatCompletionMessage):
            self.message = message
        else:
            self.message = ChatCompletionMessage()
        self.finish_reason = finish_reason
        self.logprobs = logprobs
        for k, v in kwargs.items():
            setattr(self, k, v)

    def to_dict(self) -> dict[str, Any]:
        return {
            "index": self.index,
            "message": self.message.to_dict(),
            "finish_reason": self.finish_reason,
            "logprobs": self.logprobs,
        }

    def model_dump(self) -> dict[str, Any]:
        return self.to_dict()

    def dict(self) -> dict[str, Any]:
        return self.to_dict()

    def __getitem__(self, key: str) -> Any:
        return getattr(self, key)

    def get(self, key: str, default: Any = None) -> Any:
        return getattr(self, key, default)


class CompletionUsage:
    """Represents token usage information."""

    def __init__(
        self,
        prompt_tokens: int = 0,
        completion_tokens: int = 0,
        total_tokens: int = 0,
        prompt_tokens_details: dict[str, Any] | None = None,
        completion_tokens_details: dict[str, Any] | None = None,
        **kwargs: Any,
    ) -> None:
        self.prompt_tokens = prompt_tokens
        self.completion_tokens = completion_tokens
        self.total_tokens = total_tokens
        self.prompt_tokens_details = prompt_tokens_details or {}
        self.completion_tokens_details = completion_tokens_details or {}
        for k, v in kwargs.items():
            setattr(self, k, v)

    def to_dict(self) -> dict[str, Any]:
        return {
            "prompt_tokens": self.prompt_tokens,
            "completion_tokens": self.completion_tokens,
            "total_tokens": self.total_tokens,
            "prompt_tokens_details": self.prompt_tokens_details,
            "completion_tokens_details": self.completion_tokens_details,
        }

    def model_dump(self) -> dict[str, Any]:
        return self.to_dict()

    def dict(self) -> dict[str, Any]:
        return self.to_dict()

    def __getitem__(self, key: str) -> Any:
        return getattr(self, key)

    def get(self, key: str, default: Any = None) -> Any:
        return getattr(self, key, default)

    def __repr__(self) -> str:
        return f"CompletionUsage(prompt_tokens={self.prompt_tokens}, completion_tokens={self.completion_tokens}, total_tokens={self.total_tokens})"


class ChatCompletion:
    """Represents a complete chat completion returned by Sunborn/OpenAI."""

    def __init__(self, raw: dict[str, Any]) -> None:
        self._raw = raw
        self.id = raw.get("id", "")
        self.object = raw.get("object", "chat.completion")
        self.created = raw.get("created", 0)
        self.model = raw.get("model", "")
        self.system_fingerprint = raw.get("system_fingerprint", None)
        self.service_tier = raw.get("service_tier", None)

        # Sunborn Semantic AI Gateway Attestation & Verification Attributes
        self.sunborn_cached = raw.get("sunborn_cached", False)
        self.sunborn_tier = raw.get("sunborn_tier", "origin")
        self.sunborn_attestation = raw.get("sunborn_attestation", None)

        choices = []
        for i, c in enumerate(raw.get("choices", [])):
            msg_dict = c.get("message", {})
            msg_kwargs = {k: v for k, v in msg_dict.items() if k not in ("role", "content")}
            msg = ChatCompletionMessage(
                role=msg_dict.get("role", "assistant"),
                content=msg_dict.get("content", ""),
                **msg_kwargs,
            )
            choice_kwargs = {k: v for k, v in c.items() if k not in ("index", "message", "finish_reason")}
            choices.append(
                ChatCompletionChoice(
                    index=c.get("index", i),
                    message=msg,
                    finish_reason=c.get("finish_reason", "stop"),
                    **choice_kwargs,
                )
            )
        self.choices = choices

        usage_dict = raw.get("usage", {})
        self.usage = CompletionUsage(
            prompt_tokens=usage_dict.get("prompt_tokens", 0),
            completion_tokens=usage_dict.get("completion_tokens", 0),
            total_tokens=usage_dict.get("total_tokens", 0),
            prompt_tokens_details=usage_dict.get("prompt_tokens_details"),
            completion_tokens_details=usage_dict.get("completion_tokens_details"),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "id": self.id,
            "object": self.object,
            "created": self.created,
            "model": self.model,
            "choices": [c.to_dict() for c in self.choices],
            "usage": self.usage.to_dict(),
            "system_fingerprint": self.system_fingerprint,
            "service_tier": self.service_tier,
            "sunborn_cached": self.sunborn_cached,
            "sunborn_tier": self.sunborn_tier,
            "sunborn_attestation": self.sunborn_attestation,
        }

    def model_dump(self) -> dict[str, Any]:
        return self.to_dict()

    def dict(self) -> dict[str, Any]:
        return self.to_dict()

    def model_dump_json(self) -> str:
        return json.dumps(self.to_dict())

    def __getitem__(self, key: str) -> Any:
        return self._raw[key] if key in self._raw else getattr(self, key)

    def get(self, key: str, default: Any = None) -> Any:
        if key in self._raw:
            return self._raw[key]
        return getattr(self, key, default)

    def __contains__(self, key: str) -> bool:
        return key in self._raw or hasattr(self, key)

    def __repr__(self) -> str:
        content = (self.choices[0].message.content or "") if self.choices else ""
        return f"ChatCompletion(id='{self.id}', model='{self.model}', cached={self.sunborn_cached}, tier='{self.sunborn_tier}', content='{content[:40]}...')"


class ChatCompletionChunkDelta:
    """Represents the incremental delta in a streamed completion chunk."""

    def __init__(
        self,
        role: str | None = None,
        content: str | None = None,
        tool_calls: list[dict[str, Any]] | None = None,
        refusal: str | None = None,
        **kwargs: Any,
    ) -> None:
        self.role = role
        self.content = content
        self.tool_calls = tool_calls
        self.refusal = refusal
        for k, v in kwargs.items():
            setattr(self, k, v)

    def to_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {}
        if self.role is not None:
            d["role"] = self.role
        if self.content is not None:
            d["content"] = self.content
        if self.tool_calls is not None:
            d["tool_calls"] = self.tool_calls
        if self.refusal is not None:
            d["refusal"] = self.refusal
        return d

    def model_dump(self) -> dict[str, Any]:
        return self.to_dict()

    def __getitem__(self, key: str) -> Any:
        return getattr(self, key)

    def get(self, key: str, default: Any = None) -> Any:
        return getattr(self, key, default)


class ChatCompletionChunkChoice:
    """Represents a choice inside a streaming chunk."""

    def __init__(
        self,
        index: int = 0,
        delta: ChatCompletionChunkDelta | dict[str, Any] | None = None,
        finish_reason: str | None = None,
        logprobs: Any | None = None,
        **kwargs: Any,
    ) -> None:
        self.index = index
        if isinstance(delta, dict):
            self.delta = ChatCompletionChunkDelta(**delta)
        elif isinstance(delta, ChatCompletionChunkDelta):
            self.delta = delta
        else:
            self.delta = ChatCompletionChunkDelta()
        self.finish_reason = finish_reason
        self.logprobs = logprobs
        for k, v in kwargs.items():
            setattr(self, k, v)

    def to_dict(self) -> dict[str, Any]:
        return {
            "index": self.index,
            "delta": self.delta.to_dict(),
            "finish_reason": self.finish_reason,
            "logprobs": self.logprobs,
        }

    def model_dump(self) -> dict[str, Any]:
        return self.to_dict()

    def __getitem__(self, key: str) -> Any:
        return getattr(self, key)

    def get(self, key: str, default: Any = None) -> Any:
        return getattr(self, key, default)


class ChatCompletionChunk:
    """Represents a streaming chunk yielded by Sunborn/OpenAI."""

    def __init__(self, raw: dict[str, Any]) -> None:
        self._raw = raw
        self.id = raw.get("id", "")
        self.object = raw.get("object", "chat.completion.chunk")
        self.created = raw.get("created", 0)
        self.model = raw.get("model", "")
        self.system_fingerprint = raw.get("system_fingerprint", None)
        self.sunborn_cached = raw.get("sunborn_cached", False)
        self.sunborn_tier = raw.get("sunborn_tier", "origin")

        choices = []
        for i, c in enumerate(raw.get("choices", [])):
            delta_dict = c.get("delta", {})
            delta_kwargs = {k: v for k, v in delta_dict.items() if k not in ("role", "content")}
            delta = ChatCompletionChunkDelta(
                role=delta_dict.get("role"),
                content=delta_dict.get("content"),
                **delta_kwargs,
            )
            choices.append(
                ChatCompletionChunkChoice(
                    index=c.get("index", i),
                    delta=delta,
                    finish_reason=c.get("finish_reason"),
                )
            )
        self.choices = choices

        usage_dict = raw.get("usage")
        self.usage = CompletionUsage(**usage_dict) if usage_dict else None

    def to_dict(self) -> dict[str, Any]:
        d: dict[str, Any] = {
            "id": self.id,
            "object": self.object,
            "created": self.created,
            "model": self.model,
            "choices": [c.to_dict() for c in self.choices],
            "sunborn_cached": self.sunborn_cached,
            "sunborn_tier": self.sunborn_tier,
        }
        if self.usage is not None:
            d["usage"] = self.usage.to_dict()
        return d

    def model_dump(self) -> dict[str, Any]:
        return self.to_dict()

    def dict(self) -> dict[str, Any]:
        return self.to_dict()

    def model_dump_json(self) -> str:
        return json.dumps(self.to_dict())

    def __getitem__(self, key: str) -> Any:
        return self._raw[key] if key in self._raw else getattr(self, key)

    def get(self, key: str, default: Any = None) -> Any:
        if key in self._raw:
            return self._raw[key]
        return getattr(self, key, default)

    def __contains__(self, key: str) -> bool:
        return key in self._raw or hasattr(self, key)

    def __repr__(self) -> str:
        content = self.choices[0].delta.content if self.choices else ""
        return f"ChatCompletionChunk(id='{self.id}', model='{self.model}', content='{content}')"


class Stream(Iterator[ChatCompletionChunk]):
    """Synchronous streaming context manager and iterator for ChatCompletionChunk."""

    def __init__(self, iterator: Iterator[ChatCompletionChunk], response: Any | None = None) -> None:
        self._iterator = iterator
        self._response = response
        self._closed = False

    def __iter__(self) -> Stream:
        return self

    def __next__(self) -> ChatCompletionChunk:
        if self._closed:
            raise StopIteration
        try:
            return next(self._iterator)
        except StopIteration:
            self.close()
            raise

    def close(self) -> None:
        if not self._closed:
            self._closed = True
            if hasattr(self._iterator, "close"):
                try:
                    self._iterator.close()
                except Exception:
                    pass
            if self._response and hasattr(self._response, "close"):
                try:
                    self._response.close()
                except Exception:
                    pass

    def __enter__(self) -> Stream:
        return self

    def __exit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        self.close()


class AsyncStream(AsyncIterator[ChatCompletionChunk]):
    """
    Asynchronous streaming context manager, awaitable, and async-iterator.

    Supports both:
        stream = await client.chat.completions.create(stream=True)
        async for chunk in stream: ...

    And:
        async for chunk in client.chat.completions.create(stream=True): ...
    """

    def __init__(self, sync_stream: Stream) -> None:
        self._stream = sync_stream
        self._closed = False

    def __aiter__(self) -> AsyncStream:
        return self

    async def __anext__(self) -> ChatCompletionChunk:
        if self._closed:
            raise StopAsyncIteration
        sentinel = object()
        chunk = await asyncio.to_thread(self._get_next_chunk, sentinel)
        if chunk is sentinel:
            self.close()
            raise StopAsyncIteration
        return cast(ChatCompletionChunk, chunk)

    def _get_next_chunk(self, sentinel: Any) -> Any:
        try:
            return next(self._stream)
        except StopIteration:
            return sentinel

    def close(self) -> None:
        self._closed = True
        self._stream.close()

    async def aclose(self) -> None:
        await asyncio.to_thread(self.close)

    def __await__(self) -> Any:
        async def _resolve() -> AsyncStream:
            return self

        return _resolve().__await__()

    async def __aenter__(self) -> AsyncStream:
        return self

    async def __aexit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        await self.aclose()


class Model:
    """Represents an available model descriptor in OpenAI format."""

    def __init__(self, id: str, created: int = 1700000000, owned_by: str = "sunborn", **kwargs: Any) -> None:
        self.id = id
        self.created = created
        self.owned_by = owned_by
        self.object = "model"
        for k, v in kwargs.items():
            setattr(self, k, v)

    def to_dict(self) -> dict[str, Any]:
        return {"id": self.id, "created": self.created, "owned_by": self.owned_by, "object": self.object}

    def model_dump(self) -> dict[str, Any]:
        return self.to_dict()

    def __repr__(self) -> str:
        return f"Model(id='{self.id}', owned_by='{self.owned_by}')"


class ModelsResource:
    def __init__(self, client: OpenAI) -> None:
        self._client = client

    def list(self) -> list[Model]:
        """Lists available models or returns registered gateway models."""
        url = f"{self._client.base_url.rstrip('/')}/models"
        headers = self._client._build_headers()
        try:
            data = self._client._get(url, headers)
            raw_models = data.get("data", [])
            return [Model(**m) if isinstance(m, dict) else Model(id=str(m)) for m in raw_models]
        except Exception:
            # Resilient fallback if /v1/models is not mounted on the gateway
            return [
                Model(id="gpt-4o", owned_by="openai"),
                Model(id="gpt-4o-mini", owned_by="openai"),
                Model(id="claude-3-5-sonnet-20241022", owned_by="anthropic"),
                Model(id="o1-mini", owned_by="openai"),
            ]

    def retrieve(self, model: str) -> Model:
        return Model(id=model, owned_by="sunborn")


class AsyncModelsResource:
    def __init__(self, client: AsyncOpenAI) -> None:
        self._client = client

    async def list(self) -> list[Model]:
        return await asyncio.to_thread(self._client._sync_client.models.list)

    async def retrieve(self, model: str) -> Model:
        return await asyncio.to_thread(self._client._sync_client.models.retrieve, model)


class Embedding:
    """Represents an embedding vector in OpenAI format."""

    def __init__(self, index: int, embedding: list[float], object: str = "embedding") -> None:
        self.index = index
        self.embedding = embedding
        self.object = object

    def to_dict(self) -> dict[str, Any]:
        return {"index": self.index, "embedding": self.embedding, "object": self.object}

    def model_dump(self) -> dict[str, Any]:
        return self.to_dict()


class CreateEmbeddingResponse:
    """Represents the response from creating embeddings."""

    def __init__(self, data: list[Embedding], model: str, usage: CompletionUsage | None = None) -> None:
        self.data = data
        self.model = model
        self.object = "list"
        self.usage = usage or CompletionUsage()

    def to_dict(self) -> dict[str, Any]:
        return {
            "data": [e.to_dict() for e in self.data],
            "model": self.model,
            "object": self.object,
            "usage": self.usage.to_dict(),
        }

    def model_dump(self) -> dict[str, Any]:
        return self.to_dict()


class EmbeddingsResource:
    def __init__(self, client: OpenAI) -> None:
        self._client = client

    def create(
        self,
        *,
        model: str,
        input: str | list[str],
        encoding_format: str = "float",
        dimensions: int | None = None,
        **kwargs: Any,
    ) -> CreateEmbeddingResponse:
        url = f"{self._client.base_url.rstrip('/')}/embeddings"
        payload: dict[str, Any] = {
            "model": model,
            "input": input,
            "encoding_format": encoding_format,
        }
        if dimensions is not None:
            payload["dimensions"] = dimensions
        payload.update(kwargs)

        headers = self._client._build_headers()
        data = self._client._post(url, payload, headers)
        raw_data = data.get("data", [])
        embs = [
            Embedding(
                index=e.get("index", idx),
                embedding=e.get("embedding", []),
            )
            for idx, e in enumerate(raw_data)
        ]
        usage = CompletionUsage(**data.get("usage", {}))
        return CreateEmbeddingResponse(data=embs, model=data.get("model", model), usage=usage)


class AsyncEmbeddingsResource:
    def __init__(self, client: AsyncOpenAI) -> None:
        self._client = client

    async def create(
        self,
        *,
        model: str,
        input: str | list[str],
        encoding_format: str = "float",
        dimensions: int | None = None,
        **kwargs: Any,
    ) -> CreateEmbeddingResponse:
        return await asyncio.to_thread(
            self._client._sync_client.embeddings.create,
            model=model,
            input=input,
            encoding_format=encoding_format,
            dimensions=dimensions,
            **kwargs,
        )


class ChatCompletionsResource:
    def __init__(self, client: OpenAI) -> None:
        self._client = client

    def create(
        self,
        *,
        model: str,
        messages: list[dict[str, Any]],
        temperature: float | None = None,
        top_p: float | None = None,
        max_tokens: int | None = None,
        max_completion_tokens: int | None = None,
        stream: bool = False,
        tools: list[dict[str, Any]] | None = None,
        tool_choice: Any | None = None,
        response_format: dict[str, Any] | None = None,
        stop: str | list[str] | None = None,
        seed: int | None = None,
        presence_penalty: float | None = None,
        frequency_penalty: float | None = None,
        logit_bias: dict[str, float] | None = None,
        user: str | None = None,
        stream_options: dict[str, Any] | None = None,
        sunborn_cache_policy: str | None = None,
        sunborn_tenant_id: str | None = None,
        sunborn_context_digest: str | None = None,
        sunborn_allow_stochastic: bool | None = None,
        **kwargs: Any,
    ) -> ChatCompletion | Stream:
        payload: dict[str, Any] = {
            "model": model,
            "messages": messages,
        }
        if temperature is not None:
            payload["temperature"] = temperature
        if top_p is not None:
            payload["top_p"] = top_p
        if max_tokens is not None:
            payload["max_tokens"] = max_tokens
        if max_completion_tokens is not None:
            payload["max_completion_tokens"] = max_completion_tokens
        if stream:
            payload["stream"] = True
        if tools is not None:
            payload["tools"] = tools
        if tool_choice is not None:
            payload["tool_choice"] = tool_choice
        if response_format is not None:
            payload["response_format"] = response_format
        if stop is not None:
            payload["stop"] = stop
        if seed is not None:
            payload["seed"] = seed
        if presence_penalty is not None:
            payload["presence_penalty"] = presence_penalty
        if frequency_penalty is not None:
            payload["frequency_penalty"] = frequency_penalty
        if logit_bias is not None:
            payload["logit_bias"] = logit_bias
        if user is not None:
            payload["user"] = user
        if stream_options is not None:
            payload["stream_options"] = stream_options
        if sunborn_cache_policy:
            payload["cache_policy"] = sunborn_cache_policy

        # Forward non-sunborn kwargs directly into request payload
        for k, v in kwargs.items():
            if not k.startswith("sunborn_"):
                payload[k] = v

        headers = self._client._build_headers(sunborn_tenant_id)
        if sunborn_cache_policy:
            headers["x-sunborn-cache-policy"] = sunborn_cache_policy
        if sunborn_context_digest:
            headers["x-sunborn-context-digest"] = sunborn_context_digest
        if sunborn_allow_stochastic:
            headers["x-sunborn-cache-policy"] = "allow-stochastic"

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
        organization: str | None = None,
        project: str | None = None,
        tenant_id: str | None = None,
        application_id: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 2,
        default_headers: dict[str, str] | None = None,
        **kwargs: Any,
    ) -> None:
        self.api_key = (
            api_key or os.environ.get("SUNBORN_API_KEY") or os.environ.get("OPENAI_API_KEY") or "sunborn-local-dev-key"
        )
        raw_url = (
            base_url
            or os.environ.get("SUNBORN_BASE_URL")
            or os.environ.get("OPENAI_BASE_URL")
            or "http://localhost:8080/v1"
        )
        clean_url = raw_url.rstrip("/")
        if not clean_url.endswith("/v1"):
            clean_url = f"{clean_url}/v1"
        self.base_url = clean_url

        self.organization = organization or os.environ.get("OPENAI_ORG_ID")
        self.project = project or os.environ.get("OPENAI_PROJECT_ID")
        self.tenant_id = tenant_id or os.environ.get("SUNBORN_TENANT_ID") or "default-tenant"
        self.application_id = application_id or os.environ.get("SUNBORN_APPLICATION_ID") or "default-app"
        self.timeout = timeout
        self.max_retries = max(0, max_retries)
        self.default_headers = default_headers or {}
        self._closed = False

        self.chat = ChatResource(self)
        self.models = ModelsResource(self)
        self.embeddings = EmbeddingsResource(self)

    def __enter__(self) -> OpenAI:
        return self

    def __exit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        self.close()

    def close(self) -> None:
        self._closed = True

    @property
    def is_closed(self) -> bool:
        return self._closed

    def _build_headers(self, override_tenant: str | None = None) -> dict[str, str]:
        headers = {
            "Content-Type": "application/json",
            "Accept": "application/json",
            "x-api-key": self.api_key,
            "Authorization": f"Bearer {self.api_key}",
            "x-tenant-id": override_tenant or self.tenant_id,
            "x-application-id": self.application_id,
            "User-Agent": "Sunborn-OpenAI-Client/1.0.0",
        }
        if self.organization:
            headers["OpenAI-Organization"] = self.organization
        if self.project:
            headers["OpenAI-Project"] = self.project
        if self.default_headers:
            headers.update(self.default_headers)
        return headers

    def _parse_error(self, err: urllib.error.HTTPError) -> NoReturn:
        err_body = err.read().decode("utf-8", errors="replace") if hasattr(err, "read") else str(err)
        details: dict[str, Any] | None = None
        try:
            parsed = _fast_loads(err_body)
            if isinstance(parsed, dict):
                details = parsed
        except Exception:
            pass
        raise_for_status(err.code, err_body, details)

    def _post(self, url: str, payload: dict[str, Any], headers: dict[str, str]) -> dict[str, Any]:
        body = _fast_dumps(payload)
        req = urllib.request.Request(url, data=body, headers=headers, method="POST")
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as response:
                resp_bytes = response.read()
                return _fast_loads(resp_bytes)
        except urllib.error.HTTPError as err:
            self._parse_error(err)
        except urllib.error.URLError as err:
            raise SunbornConnectionError(f"Failed to connect to Sunborn Gateway at {url}: {err}") from err

    def _get(self, url: str, headers: dict[str, str]) -> dict[str, Any]:
        req = urllib.request.Request(url, headers=headers, method="GET")
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as response:
                resp_bytes = response.read()
                return _fast_loads(resp_bytes)
        except urllib.error.HTTPError as err:
            self._parse_error(err)
        except urllib.error.URLError as err:
            raise SunbornConnectionError(f"Failed to connect to Sunborn Gateway at {url}: {err}") from err

    def _post_stream(self, url: str, payload: dict[str, Any], headers: dict[str, str]) -> Stream:
        body = _fast_dumps(payload)
        stream_headers = dict(headers)
        stream_headers["Accept"] = "text/event-stream"
        req = urllib.request.Request(url, data=body, headers=stream_headers, method="POST")

        try:
            response = urllib.request.urlopen(req, timeout=self.timeout)
        except urllib.error.HTTPError as err:
            self._parse_error(err)
        except urllib.error.URLError as err:
            raise SunbornConnectionError(f"Failed to connect to Sunborn Gateway at {url}: {err}") from err

        def _generator() -> Iterator[ChatCompletionChunk]:
            with response:
                content_type = response.headers.get("Content-Type", "")
                # If server returned non-SSE JSON (e.g. cached hit response returned all at once)
                if "application/json" in content_type:
                    raw_bytes = response.read()
                    data = _fast_loads(raw_bytes)
                    choices = data.get("choices", [])
                    if choices:
                        chunk_choices = []
                        for idx, ch in enumerate(choices):
                            msg_dict = ch.get("message", {})
                            content = msg_dict.get("content", "")
                            chunk_choices.append(
                                {
                                    "index": ch.get("index", idx),
                                    "delta": {"role": msg_dict.get("role", "assistant"), "content": content},
                                    "finish_reason": ch.get("finish_reason", "stop"),
                                }
                            )
                        yield ChatCompletionChunk(
                            {
                                "id": data.get("id", "chunk-0"),
                                "object": "chat.completion.chunk",
                                "created": data.get("created", 0),
                                "model": data.get("model", ""),
                                "sunborn_cached": data.get("sunborn_cached", False),
                                "sunborn_tier": data.get("sunborn_tier", "origin"),
                                "choices": chunk_choices,
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
                            if isinstance(chunk_dict, dict):
                                yield ChatCompletionChunk(chunk_dict)
                        except Exception:
                            continue

        return Stream(_generator(), response=response)


class AsyncChatCompletionsResource:
    def __init__(self, client: AsyncOpenAI) -> None:
        self._client = client

    def create(
        self,
        *,
        model: str,
        messages: list[dict[str, Any]],
        temperature: float | None = None,
        top_p: float | None = None,
        max_tokens: int | None = None,
        max_completion_tokens: int | None = None,
        stream: bool = False,
        tools: list[dict[str, Any]] | None = None,
        tool_choice: Any | None = None,
        response_format: dict[str, Any] | None = None,
        stop: str | list[str] | None = None,
        seed: int | None = None,
        presence_penalty: float | None = None,
        frequency_penalty: float | None = None,
        logit_bias: dict[str, float] | None = None,
        user: str | None = None,
        stream_options: dict[str, Any] | None = None,
        sunborn_cache_policy: str | None = None,
        sunborn_tenant_id: str | None = None,
        sunborn_context_digest: str | None = None,
        sunborn_allow_stochastic: bool | None = None,
        **kwargs: Any,
    ) -> Any:
        if stream:
            return self._stream_create(
                model=model,
                messages=messages,
                temperature=temperature,
                top_p=top_p,
                max_tokens=max_tokens,
                max_completion_tokens=max_completion_tokens,
                tools=tools,
                tool_choice=tool_choice,
                response_format=response_format,
                stop=stop,
                seed=seed,
                presence_penalty=presence_penalty,
                frequency_penalty=frequency_penalty,
                logit_bias=logit_bias,
                user=user,
                stream_options=stream_options,
                sunborn_cache_policy=sunborn_cache_policy,
                sunborn_tenant_id=sunborn_tenant_id,
                sunborn_context_digest=sunborn_context_digest,
                sunborn_allow_stochastic=sunborn_allow_stochastic,
                **kwargs,
            )

        async def _exec() -> ChatCompletion:
            sync_create = cast(Callable[..., ChatCompletion], self._client._sync_client.chat.completions.create)
            return await asyncio.to_thread(
                sync_create,
                model=model,
                messages=messages,
                temperature=temperature,
                top_p=top_p,
                max_tokens=max_tokens,
                max_completion_tokens=max_completion_tokens,
                stream=False,
                tools=tools,
                tool_choice=tool_choice,
                response_format=response_format,
                stop=stop,
                seed=seed,
                presence_penalty=presence_penalty,
                frequency_penalty=frequency_penalty,
                logit_bias=logit_bias,
                user=user,
                stream_options=stream_options,
                sunborn_cache_policy=sunborn_cache_policy,
                sunborn_tenant_id=sunborn_tenant_id,
                sunborn_context_digest=sunborn_context_digest,
                sunborn_allow_stochastic=sunborn_allow_stochastic,
                **kwargs,
            )

        return _exec()

    def _stream_create(self, **kwargs: Any) -> AsyncStream:
        sync_stream = cast(
            Stream,
            self._client._sync_client.chat.completions.create(
                stream=True,
                **kwargs,
            ),
        )
        return AsyncStream(sync_stream)


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
        organization: str | None = None,
        project: str | None = None,
        tenant_id: str | None = None,
        application_id: str | None = None,
        timeout: float = 30.0,
        max_retries: int = 2,
        default_headers: dict[str, str] | None = None,
        **kwargs: Any,
    ) -> None:
        self._sync_client = OpenAI(
            api_key=api_key,
            base_url=base_url,
            organization=organization,
            project=project,
            tenant_id=tenant_id,
            application_id=application_id,
            timeout=timeout,
            max_retries=max_retries,
            default_headers=default_headers,
            **kwargs,
        )
        self.chat = AsyncChatResource(self)
        self.models = AsyncModelsResource(self)
        self.embeddings = AsyncEmbeddingsResource(self)

    async def __aenter__(self) -> AsyncOpenAI:
        return self

    async def __aexit__(self, exc_type: Any, exc_val: Any, exc_tb: Any) -> None:
        await self.aclose()

    async def aclose(self) -> None:
        await asyncio.to_thread(self._sync_client.close)

    def close(self) -> None:
        self._sync_client.close()

    @property
    def is_closed(self) -> bool:
        return self._sync_client.is_closed

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
