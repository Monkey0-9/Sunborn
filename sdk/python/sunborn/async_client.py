"""Asynchronous Sunborn client built on ``httpx.AsyncClient``."""

from __future__ import annotations

import asyncio
import os
from typing import Any

from ._http import AsyncTransport
from .models import ExecutionResponse

__all__ = ["AsyncSunbornClient"]

_USER_AGENT = "Sunborn-Python-SDK/1.0.0"


class AsyncSunbornClient:
    """Truly asynchronous client for asyncio and async-context-manager use."""

    def __init__(
        self,
        base_url: str = "http://localhost:8080/v1",
        api_key: str | None = None,
        tenant_id: str = "default-tenant",
        application_id: str = "default-app",
        timeout: float = 30.0,
        max_retries: int = 2,
        backoff_factor: float = 0.5,
    ) -> None:
        clean_url = base_url.rstrip("/")
        if not clean_url.endswith("/v1"):
            clean_url = f"{clean_url}/v1"
        self.base_url = clean_url

        resolved_key = api_key or os.environ.get("SUNBORN_API_KEY")
        if not resolved_key:
            raise ValueError(
                "AsyncSunbornClient requires an API key. Pass `api_key` explicitly or "
                "set the `SUNBORN_API_KEY` environment variable."
            )
        self.api_key = resolved_key
        self.tenant_id = tenant_id
        self.application_id = application_id
        self.timeout = timeout
        self.max_retries = max(0, max_retries)
        self.backoff_factor = backoff_factor
        self._transport = AsyncTransport(
            self.base_url,
            self._build_headers(),
            timeout=timeout,
            max_retries=self.max_retries,
            backoff_factor=backoff_factor,
        )

    async def __aenter__(self) -> AsyncSunbornClient:
        return self

    async def __aexit__(self, exc_type: object, exc_val: object, exc_tb: object) -> None:
        await self.aclose()

    async def aclose(self) -> None:
        """Close the underlying async connection pool."""
        await self._transport.aclose()

    def close(self) -> None:
        """Best-effort synchronous close for compatibility.

        Prefer ``await client.aclose()``. Inside a running event loop this
        schedules the close instead of blocking the loop.
        """
        try:
            loop = asyncio.get_running_loop()
        except RuntimeError:
            asyncio.run(self.aclose())
        else:
            loop.create_task(self.aclose())

    @property
    def is_closed(self) -> bool:
        return self._transport.is_closed

    def _build_headers(self, override_tenant: str | None = None) -> dict[str, str]:
        return {
            "Content-Type": "application/json",
            "Accept": "application/json",
            "x-api-key": self.api_key,
            "x-tenant-id": override_tenant or self.tenant_id,
            "x-application-id": self.application_id,
            "User-Agent": _USER_AGENT,
        }

    async def _post(
        self,
        path: str,
        payload: dict[str, Any],
        custom_headers: dict[str, str] | None = None,
    ) -> dict[str, Any]:
        result: dict[str, Any] = await self._transport.request("POST", path, payload, headers=custom_headers)
        return result

    async def _get(self, path: str, custom_headers: dict[str, str] | None = None) -> dict[str, Any]:
        result: dict[str, Any] = await self._transport.request("GET", path, None, headers=custom_headers)
        return result

    async def login(self, client_id: str, client_secret: str, tenant_id: str | None = None) -> dict[str, Any]:
        """Authenticate against ``/v1/auth/login`` and adopt the returned token."""
        payload: dict[str, Any] = {
            "client_id": client_id,
            "client_secret": client_secret,
        }
        if tenant_id:
            payload["tenant_id"] = tenant_id

        res = await self._post("auth/login", payload)
        if "access_token" in res:
            self.api_key = res["access_token"]
            await self._transport.aclose()
            self._transport = AsyncTransport(
                self.base_url,
                self._build_headers(),
                timeout=self.timeout,
                max_retries=self.max_retries,
                backoff_factor=self.backoff_factor,
            )
        if "tenant_id" in res:
            self.tenant_id = res["tenant_id"]
        if "application_id" in res:
            self.application_id = res["application_id"]
        return res

    async def chat_completion(
        self,
        model: str = "gpt-4o",
        messages: list[dict[str, str]] | None = None,
        prompt: str | None = None,
        system_prompt: str | None = None,
        temperature: float | None = None,
        top_p: float | None = None,
        max_tokens: int | None = None,
        stop: str | list[str] | None = None,
        seed: int | None = None,
        response_format: dict[str, Any] | None = None,
        tools: list[dict[str, Any]] | None = None,
        allow_stochastic: bool = False,
        context_digest: str | None = None,
    ) -> dict[str, Any]:
        """Invoke the OpenAI-compatible ``/v1/chat/completions`` endpoint."""
        msgs = list(messages) if messages else []
        if prompt:
            if system_prompt:
                msgs.append({"role": "system", "content": system_prompt})
            msgs.append({"role": "user", "content": prompt})

        payload: dict[str, Any] = {"model": model, "messages": msgs}
        if temperature is not None:
            payload["temperature"] = temperature
        if top_p is not None:
            payload["top_p"] = top_p
        if max_tokens is not None:
            payload["max_tokens"] = max_tokens
        if stop is not None:
            payload["stop"] = [stop] if isinstance(stop, str) else stop
        if seed is not None:
            payload["seed"] = seed
        if response_format is not None:
            payload["response_format"] = response_format
        if tools is not None:
            payload["tools"] = tools

        custom_headers: dict[str, str] = {}
        if allow_stochastic:
            custom_headers["x-sunborn-cache-policy"] = "allow-stochastic"
        if context_digest:
            custom_headers["x-sunborn-context-digest"] = context_digest

        return await self._post("chat/completions", payload, custom_headers=custom_headers)

    async def execute_request(
        self,
        prompt: str,
        model: str = "gpt-4o",
        system_prompt: str | None = None,
        temperature: float | None = 0.0,
        **kwargs: Any,
    ) -> ExecutionResponse:
        """Execute a prompt and return a typed, cache-aware response."""
        payload = await self.chat_completion(
            model=model,
            prompt=prompt,
            system_prompt=system_prompt,
            temperature=temperature,
            **kwargs,
        )
        choices = payload.get("choices") or []
        content = ""
        if choices:
            message = choices[0].get("message") or {}
            content = message.get("content") or ""
        usage = payload.get("usage") or {}
        return ExecutionResponse(
            content=str(content),
            tier=str(payload.get("sunborn_tier", "origin")),
            cached=bool(payload.get("sunborn_cached", False)),
            model=str(payload.get("model", model)),
            usage=dict(usage) if isinstance(usage, dict) else {},
            raw=payload,
        )

    async def create_message(
        self,
        model: str = "claude-3-5-sonnet-20241022",
        messages: list[dict[str, Any]] | None = None,
        system: str | list[dict[str, Any]] | None = None,
        max_tokens: int | None = 1024,
        temperature: float | None = None,
        top_p: float | None = None,
        stop_sequences: list[str] | None = None,
    ) -> dict[str, Any]:
        """Invoke the Anthropic-compatible ``/v1/messages`` endpoint."""
        payload: dict[str, Any] = {"model": model, "messages": messages or []}
        if system is not None:
            payload["system"] = system
        if max_tokens is not None:
            payload["max_tokens"] = max_tokens
        if temperature is not None:
            payload["temperature"] = temperature
        if top_p is not None:
            payload["top_p"] = top_p
        if stop_sequences is not None:
            payload["stop_sequences"] = stop_sequences
        return await self._post("messages", payload)

    messages = create_message

    async def get_finops_report(self) -> dict[str, Any]:
        return await self._get("finops/report")

    async def plan_execution_dag(self, plan_nodes: list[dict[str, Any]]) -> dict[str, Any]:
        payload = {
            "tenant_id": self.tenant_id,
            "application_id": self.application_id,
            "plan": {"nodes": plan_nodes},
        }
        return await self._post("execution/plan", payload)

    async def record_tool_result(
        self,
        tool_name: str,
        tool_version: str,
        arguments: dict[str, Any],
        output: Any,
        execution_latency_ms: int = 0,
        context_hash: str | None = None,
        ttl_secs: int | None = None,
        is_error: bool = False,
    ) -> dict[str, Any]:
        payload = {
            "tenant_id": self.tenant_id,
            "application_id": self.application_id,
            "tool_name": tool_name,
            "tool_version": tool_version,
            "arguments": arguments,
            "output": output,
            "is_error": is_error,
            "execution_latency_ms": execution_latency_ms,
            "context_hash": context_hash,
            "ttl_secs": ttl_secs,
        }
        return await self._post("execution/results", payload)

    async def evict_execution(
        self,
        tool_name: str | None = None,
        signature: str | None = None,
    ) -> dict[str, Any]:
        """Evict execution-cache entries (administrative endpoint)."""
        if not tool_name and not signature:
            raise ValueError("evict_execution requires either tool_name or signature")
        payload: dict[str, Any] = {"tenant_id": self.tenant_id}
        if signature:
            payload["signature"] = signature
        if tool_name:
            payload["tool_name"] = tool_name
        return await self._post("execution/evict", payload)

    async def get_graph_stats(self) -> dict[str, Any]:
        return await self._get("cache/graph/stats")

    async def get_graph_lineage(self, key: str) -> dict[str, Any]:
        return await self._get(f"cache/graph/lineage/{key.lstrip('/')}")

    async def invalidate_graph_rtc(
        self,
        source_ids: list[str] | None = None,
        doc_hashes: list[int] | None = None,
    ) -> dict[str, Any]:
        payload: dict[str, Any] = {
            "source_ids": source_ids or [],
            "doc_hashes": doc_hashes or [],
        }
        return await self._post("cache/graph/invalidate", payload)

    async def get_ceremony_public_keys(self) -> dict[str, Any]:
        return await self._get("ceremony/public-keys")
