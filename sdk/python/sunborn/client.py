"""
Synchronous Sunborn client and tool execution-reuse decorator.
"""

from __future__ import annotations

import functools
import hashlib
import inspect
import json
import os
from collections.abc import Callable
from typing import Any

from ._http import SyncTransport
from ._json import dumps as _fast_dumps
from ._json import loads as _fast_loads
from .models import ExecutionResponse, FallbackPolicy

__all__ = [
    "SunbornClient",
    "SunbornAgentClient",
    "get_default_client",
    "sunborn_tool",
    "_fast_dumps",
    "_fast_loads",
]

_USER_AGENT = "Sunborn-Python-SDK/1.0.0"


class SunbornClient:
    """Enterprise Python client for the Sunborn semantic gateway."""

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
                "SunbornClient requires an API key. Pass `api_key` explicitly or set "
                "the `SUNBORN_API_KEY` environment variable."
            )
        self.api_key = resolved_key
        self.tenant_id = tenant_id
        self.application_id = application_id
        self.timeout = timeout
        self.max_retries = max(0, max_retries)
        self.backoff_factor = backoff_factor
        self._transport = SyncTransport(
            self.base_url,
            self._build_headers(),
            timeout=timeout,
            max_retries=self.max_retries,
            backoff_factor=backoff_factor,
        )

    def __enter__(self) -> "SunbornClient":
        return self

    def __exit__(self, exc_type: object, exc_val: object, exc_tb: object) -> None:
        self.close()

    def close(self) -> None:
        """Close the underlying connection pool."""
        self._transport.close()

    @property
    def is_closed(self) -> bool:
        """Return ``True`` once the underlying connection pool is closed."""
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

    def _post(
        self,
        path: str,
        payload: dict[str, Any],
        custom_headers: dict[str, str] | None = None,
    ) -> dict[str, Any]:
        result: dict[str, Any] = self._transport.request(
            "POST", path, payload, headers=custom_headers
        )
        return result

    def _get(self, path: str, custom_headers: dict[str, str] | None = None) -> dict[str, Any]:
        result: dict[str, Any] = self._transport.request(
            "GET", path, None, headers=custom_headers
        )
        return result

    def login(
        self, client_id: str, client_secret: str, tenant_id: str | None = None
    ) -> dict[str, Any]:
        """Authenticate against ``/v1/auth/login`` and adopt the returned token."""
        payload: dict[str, Any] = {
            "client_id": client_id,
            "client_secret": client_secret,
        }
        if tenant_id:
            payload["tenant_id"] = tenant_id

        res = self._post("auth/login", payload)
        if "access_token" in res:
            self.api_key = res["access_token"]
            self._transport.close()
            self._transport = SyncTransport(
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

    def chat_completion(
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

        payload: dict[str, Any] = {
            "model": model,
            "messages": msgs,
        }
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

        return self._post("chat/completions", payload, custom_headers=custom_headers)

    def execute_request(
        self,
        prompt: str,
        model: str = "gpt-4o",
        system_prompt: str | None = None,
        temperature: float | None = 0.0,
        **kwargs: Any,
    ) -> ExecutionResponse:
        """Execute a prompt and return a typed, cache-aware response.

        This is the ergonomic helper documented in the SDK README. It wraps
        :meth:`chat_completion` and exposes the fields the gateway actually
        returns: content, cache tier, cache flag, model, and token usage.
        """
        payload = self.chat_completion(
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

    def create_message(
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
        payload: dict[str, Any] = {
            "model": model,
            "messages": messages or [],
        }
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

        return self._post("messages", payload)

    # Anthropic SDK compatibility alias
    messages = create_message

    def get_finops_report(self) -> dict[str, Any]:
        """Retrieve the live FinOps cost and token savings report."""
        return self._get("finops/report")

    def plan_execution_dag(self, plan_nodes: list[dict[str, Any]]) -> dict[str, Any]:
        """Submit an agent execution DAG to evaluate memoized tool subgraph reuse."""
        payload = {
            "tenant_id": self.tenant_id,
            "application_id": self.application_id,
            "plan": {"nodes": plan_nodes},
        }
        return self._post("execution/plan", payload)

    def record_tool_result(
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
        """Store a deterministic tool execution result in the execution cache."""
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
        return self._post("execution/results", payload)

    def evict_execution(
        self,
        tool_name: str | None = None,
        signature: str | None = None,
    ) -> dict[str, Any]:
        """Evict execution-cache entries.

        The gateway endpoint is **administrative**: non-admin credentials are
        rejected with HTTP 403. Supply exactly one of ``tool_name`` (evict all
        entries recorded under that tool) or ``signature`` (evict one entry).
        """
        if not tool_name and not signature:
            raise ValueError("evict_execution requires either tool_name or signature")
        payload: dict[str, Any] = {"tenant_id": self.tenant_id}
        if signature:
            payload["signature"] = signature
        if tool_name:
            payload["tool_name"] = tool_name
        return self._post("execution/evict", payload)

    def get_graph_stats(self) -> dict[str, Any]:
        """Retrieve graph-memory cache and lineage statistics."""
        return self._get("cache/graph/stats")

    def get_graph_lineage(self, key: str) -> dict[str, Any]:
        """Inspect upstream document lineage and cache residency for a key."""
        return self._get(f"cache/graph/lineage/{key.lstrip('/')}")

    def invalidate_graph_rtc(
        self,
        source_ids: list[str] | None = None,
        doc_hashes: list[int] | None = None,
    ) -> dict[str, Any]:
        """Trigger real-time causal invalidation over the SIMD Roaring lineage."""
        payload: dict[str, Any] = {
            "source_ids": source_ids or [],
            "doc_hashes": doc_hashes or [],
        }
        return self._post("cache/graph/invalidate", payload)

    def get_ceremony_public_keys(self) -> dict[str, Any]:
        """Retrieve active Ed25519 ceremony verifying keys."""
        return self._get("ceremony/public-keys")


SunbornAgentClient = SunbornClient


def get_default_client() -> SunbornClient:
    """Build a client from ``SUNBORN_*`` environment variables."""
    api_key = os.environ.get("SUNBORN_API_KEY")
    if not api_key:
        raise ValueError(
            "SunbornClient requires an API key. Set the `SUNBORN_API_KEY` environment "
            "variable or supply an explicit client."
        )
    return SunbornClient(
        base_url=os.environ.get("SUNBORN_BASE_URL", "http://localhost:8080/v1"),
        api_key=api_key,
        tenant_id=os.environ.get("SUNBORN_TENANT_ID", "default-tenant"),
        application_id=os.environ.get("SUNBORN_APP_ID", "default-app"),
    )


def sunborn_tool(
    name: str | None = None,
    version: str = "1.0",
    deterministic: bool = True,
    ttl_secs: int | None = None,
    context_hash: str | None = None,
    policy: str = FallbackPolicy.FAIL_CLOSED,
    client: SunbornClient | None = None,
) -> Callable[[Callable[..., Any]], Callable[..., Any]]:
    """
    Decorate an agent tool so deterministic calls are memoized by the gateway.

    Both synchronous and asynchronous tool functions are supported. With
    ``policy=FAIL_CLOSED`` (the default), a gateway failure raises instead of
    silently re-executing the tool.
    """
    import asyncio

    def decorator(fn: Callable[..., Any]) -> Callable[..., Any]:
        tool_name = name or fn.__name__
        is_async = inspect.iscoroutinefunction(fn)

        def _build_plan(arguments: dict[str, Any]) -> tuple[str, list[dict[str, Any]]]:
            arg_bytes = json.dumps(arguments, sort_keys=True, default=str).encode("utf-8")
            node_id = f"node-{tool_name}-{hashlib.sha256(arg_bytes).hexdigest()[:8]}"
            plan_nodes = [
                {
                    "id": node_id,
                    "name": tool_name,
                    "dependencies": [],
                    "is_deterministic": True,
                    "effect": "PureDeterministic"
                    if ttl_secs is None
                    else {"TimeSensitiveNonDeterministic": {"max_ttl_secs": ttl_secs}},
                    "tool_call": {
                        "tool_name": tool_name,
                        "tool_version": version,
                        "arguments": arguments,
                        "context_hash": context_hash,
                    },
                }
            ]
            return node_id, plan_nodes

        def _cached_output(eval_result: dict[str, Any], node_id: str) -> Any:
            if eval_result.get("cached_nodes", 0) <= 0:
                return None
            status = (eval_result.get("node_statuses") or {}).get(node_id) or {}
            if status.get("status") == "Cached":
                details = status.get("details") or {}
                return details.get("output")
            return None

        if is_async:

            @functools.wraps(fn)
            async def async_wrapper(*args: Any, **kwargs: Any) -> Any:
                if not deterministic:
                    return await fn(*args, **kwargs)

                sunborn = client or get_default_client()
                arguments = {"args": list(args), "kwargs": kwargs}
                node_id, plan_nodes = _build_plan(arguments)

                try:
                    eval_result = await asyncio.to_thread(
                        sunborn.plan_execution_dag, plan_nodes
                    )
                    cached = _cached_output(eval_result, node_id)
                    if cached is not None:
                        return cached
                except Exception as exc:
                    if policy == FallbackPolicy.FAIL_CLOSED:
                        raise RuntimeError(
                            f"Sunborn Policy FAIL_CLOSED: execution plan query failed "
                            f"for '{tool_name}': {exc}"
                        ) from exc

                result = await fn(*args, **kwargs)

                try:
                    await asyncio.to_thread(
                        sunborn.record_tool_result,
                        tool_name=tool_name,
                        tool_version=version,
                        arguments=arguments,
                        output=result,
                        execution_latency_ms=0,
                        context_hash=context_hash,
                        ttl_secs=ttl_secs,
                    )
                except Exception as exc:
                    if policy == FallbackPolicy.FAIL_CLOSED:
                        raise RuntimeError(
                            f"Sunborn Policy FAIL_CLOSED: recording tool result failed "
                            f"for '{tool_name}': {exc}"
                        ) from exc

                return result

            return async_wrapper

        @functools.wraps(fn)
        def sync_wrapper(*args: Any, **kwargs: Any) -> Any:
            if not deterministic:
                return fn(*args, **kwargs)

            sunborn = client or get_default_client()
            arguments = {"args": list(args), "kwargs": kwargs}
            node_id, plan_nodes = _build_plan(arguments)

            try:
                eval_result = sunborn.plan_execution_dag(plan_nodes)
                cached = _cached_output(eval_result, node_id)
                if cached is not None:
                    return cached
            except Exception as exc:
                if policy == FallbackPolicy.FAIL_CLOSED:
                    raise RuntimeError(
                        f"Sunborn Policy FAIL_CLOSED: execution plan query failed for "
                        f"'{tool_name}': {exc}"
                    ) from exc

            result = fn(*args, **kwargs)

            try:
                sunborn.record_tool_result(
                    tool_name=tool_name,
                    tool_version=version,
                    arguments=arguments,
                    output=result,
                    execution_latency_ms=0,
                    context_hash=context_hash,
                    ttl_secs=ttl_secs,
                )
            except Exception as exc:
                if policy == FallbackPolicy.FAIL_CLOSED:
                    raise RuntimeError(
                        f"Sunborn Policy FAIL_CLOSED: recording tool result failed for "
                        f"'{tool_name}': {exc}"
                    ) from exc

            return result

        return sync_wrapper

    return decorator
