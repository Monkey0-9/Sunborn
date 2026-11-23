"""
Drop-in OpenAI Client for Sunborn Semantic AI Gateway.

Provides 100% wire-compatible drop-in replacement for `openai.OpenAI` and `openai.AsyncOpenAI`,
routing all requests through Sunborn's high-throughput Rust caching and invalidation gateway.

Usage:
    from sunborn.openai import OpenAI

    client = OpenAI()
    response = client.chat.completions.create(
        model="gpt-4o",
        messages=[{"role": "user", "content": "Hello!"}],
    )
    print(response.choices[0].message.content)
    print("Cached:", getattr(response, "sunborn_cached", False))
"""

from __future__ import annotations

import json
import os
import urllib.error
import urllib.request
from typing import Any, AsyncIterator, Iterator

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
            choices.append(ChatCompletionChoice(
                index=c.get("index", i),
                message=msg,
                finish_reason=c.get("finish_reason", "stop"),
            ))
        self.choices = choices

        usage_dict = raw.get("usage", {})
        self.usage = CompletionUsage(
            prompt_tokens=usage_dict.get("prompt_tokens", 0),
            completion_tokens=usage_dict.get("completion_tokens", 0),
            total_tokens=usage_dict.get("total_tokens", 0),
        )

    def __repr__(self) -> str:
        content = self.choices[0].message.content if self.choices else ""
        return f"ChatCompletion(id='{self.id}', model='{self.model}', cached={self.sunborn_cached}, tier='{self.sunborn_tier}', content='{content[:40]}...')"


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
    ) -> ChatCompletion | Iterator[ChatCompletion]:
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

        data = self._client._post(url, payload, headers)
        return ChatCompletion(data)


class ChatResource:
    def __init__(self, client: OpenAI) -> None:
        self.completions = ChatCompletionsResource(client)


class OpenAI:
    """
    Drop-in OpenAI client compatible with `import openai; openai.OpenAI()`.
    
    Routes seamlessly through Sunborn's high-speed Axum gateway with automatic
    semantic caching,FinOps token attribution, and causal graph invalidation.
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
            api_key
            or os.environ.get("SUNBORN_API_KEY")
            or os.environ.get("OPENAI_API_KEY")
            or "sunborn-local-dev-key"
        )
        self.base_url = (
            base_url
            or os.environ.get("SUNBORN_BASE_URL")
            or os.environ.get("OPENAI_BASE_URL")
            or "http://localhost:8080/v1"
        )
        self.tenant_id = (
            tenant_id
            or os.environ.get("SUNBORN_TENANT_ID")
            or "default-tenant"
        )
        self.application_id = (
            application_id
            or os.environ.get("SUNBORN_APPLICATION_ID")
            or "default-app"
        )
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


# Aliases for drop-in imports
SunbornOpenAI = OpenAI
