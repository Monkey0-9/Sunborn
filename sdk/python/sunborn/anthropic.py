"""
Drop-in Anthropic Client for Sunborn Semantic AI Gateway.

Provides 100% wire-compatible drop-in replacement for `anthropic.Anthropic`,
routing all requests through Sunborn's high-throughput Rust gateway.

Usage:
    from sunborn.anthropic import Anthropic

    client = Anthropic()
    message = client.messages.create(
        model="claude-3-5-sonnet-20241022",
        max_tokens=1024,
        messages=[{"role": "user", "content": "Hello Claude!"}],
    )
    print(message.content[0].text)
"""

from __future__ import annotations

import json
import os
import urllib.error
import urllib.request
from typing import Any

from .client import _fast_dumps, _fast_loads
from .exceptions import SunbornConnectionError, raise_for_status


class ContentBlock:
    def __init__(self, type: str, text: str = "") -> None:
        self.type = type
        self.text = text

    def __repr__(self) -> str:
        return f"ContentBlock(type='{self.type}', text='{self.text[:40]}...')"


class MessageUsage:
    def __init__(self, input_tokens: int = 0, output_tokens: int = 0) -> None:
        self.input_tokens = input_tokens
        self.output_tokens = output_tokens


class MessageResponse:
    def __init__(self, raw: dict[str, Any]) -> None:
        self._raw = raw
        self.id = raw.get("id", "")
        self.type = raw.get("type", "message")
        self.role = raw.get("role", "assistant")
        self.model = raw.get("model", "")
        self.stop_reason = raw.get("stop_reason", "end_turn")
        self.sunborn_cached = raw.get("sunborn_cached", False)
        self.sunborn_tier = raw.get("sunborn_tier", "origin")

        contents = []
        raw_content = raw.get("content", [])
        if isinstance(raw_content, str):
            contents.append(ContentBlock(type="text", text=raw_content))
        elif isinstance(raw_content, list):
            for item in raw_content:
                if isinstance(item, dict):
                    contents.append(ContentBlock(
                        type=item.get("type", "text"),
                        text=item.get("text", ""),
                    ))
                elif isinstance(item, str):
                    contents.append(ContentBlock(type="text", text=item))
        self.content = contents

        usage_dict = raw.get("usage", {})
        self.usage = MessageUsage(
            input_tokens=usage_dict.get("input_tokens", 0),
            output_tokens=usage_dict.get("output_tokens", 0),
        )

    def __repr__(self) -> str:
        text = self.content[0].text if self.content else ""
        return f"MessageResponse(id='{self.id}', model='{self.model}', cached={self.sunborn_cached}, text='{text[:40]}...')"


class MessagesResource:
    def __init__(self, client: Anthropic) -> None:
        self._client = client

    def create(
        self,
        *,
        model: str,
        messages: list[dict[str, Any]],
        max_tokens: int = 1024,
        temperature: float | None = None,
        system: str | None = None,
        stream: bool = False,
        **kwargs: Any,
    ) -> MessageResponse:
        payload: dict[str, Any] = {
            "model": model,
            "messages": messages,
            "max_tokens": max_tokens,
        }
        if temperature is not None:
            payload["temperature"] = temperature
        if system is not None:
            payload["system"] = system
        if stream:
            payload["stream"] = True

        headers = self._client._build_headers()
        url = f"{self._client.base_url.rstrip('/')}/messages"
        data = self._client._post(url, payload, headers)
        return MessageResponse(data)


class Anthropic:
    """
    Drop-in Anthropic client compatible with `import anthropic; anthropic.Anthropic()`.
    """

    def __init__(
        self,
        api_key: str | None = None,
        base_url: str | None = None,
        tenant_id: str | None = None,
        timeout: float = 30.0,
    ) -> None:
        self.api_key = (
            api_key
            or os.environ.get("SUNBORN_API_KEY")
            or os.environ.get("ANTHROPIC_API_KEY")
            or "sunborn-local-dev-key"
        )
        self.base_url = (
            base_url
            or os.environ.get("SUNBORN_BASE_URL")
            or "http://localhost:8080/v1"
        )
        self.tenant_id = (
            tenant_id
            or os.environ.get("SUNBORN_TENANT_ID")
            or "default-tenant"
        )
        self.timeout = timeout
        self.messages = MessagesResource(self)

    def _build_headers(self) -> dict[str, str]:
        return {
            "Content-Type": "application/json",
            "x-api-key": self.api_key,
            "x-tenant-id": self.tenant_id,
            "User-Agent": "Sunborn-Anthropic-Client/1.0.0",
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


SunbornAnthropic = Anthropic
