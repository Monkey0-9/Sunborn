"""
Synchronous Sunborn Client and Tool Execution Reuse Decorator.
"""

import functools
import hashlib
import inspect
import json
import os
import time
import urllib.error
import urllib.request
from collections.abc import Callable
from typing import Any

from .exceptions import SunbornConnectionError, raise_for_status
from .models import FallbackPolicy, compute_context_digest


class SunbornClient:
    """Enterprise Python client to interact with Sunborn Semantic AI Gateway."""

    def __init__(
        self,
        base_url: str = "http://localhost:8080/v1",
        api_key: str | None = None,
        tenant_id: str = "default-tenant",
        application_id: str = "default-app",
        timeout: float = 30.0,
        max_retries: int = 2,
        backoff_factor: float = 0.5,
    ):
        clean_url = base_url.rstrip("/")
        if not clean_url.endswith("/v1"):
            clean_url = f"{clean_url}/v1"
        self.base_url = clean_url

        resolved_key = api_key or os.environ.get("SUNBORN_API_KEY")
        if not resolved_key:
            raise ValueError(
                "SunbornClient requires an API key. Pass `api_key` explicitly or set the `SUNBORN_API_KEY` environment variable."
            )
        self.api_key = resolved_key
        self.tenant_id = tenant_id
        self.application_id = application_id
        self.timeout = timeout
        self.max_retries = max(0, max_retries)
        self.backoff_factor = backoff_factor

    def __enter__(self) -> "SunbornClient":
        return self

    def __exit__(self, exc_type, exc_val, exc_tb) -> None:
        self.close()

    def close(self) -> None:
        """Closes any underlying client connections and cleans up resources."""
        pass

    def _post(
        self, path: str, payload: dict, custom_headers: dict | None = None
    ) -> dict:
        url = f"{self.base_url}/{path.lstrip('/')}"
        data = json.dumps(payload, default=str).encode("utf-8")
        headers = {
            "Content-Type": "application/json",
            "x-api-key": self.api_key,
            "x-tenant-id": self.tenant_id,
            "x-application-id": self.application_id,
        }
        if custom_headers:
            headers.update(custom_headers)

        last_error = None
        for attempt in range(self.max_retries + 1):
            req = urllib.request.Request(
                url,
                data=data,
                headers=headers,
                method="POST",
            )
            try:
                with urllib.request.urlopen(req, timeout=self.timeout) as response:
                    content = response.read().decode("utf-8")
                    return json.loads(content) if content else {}
            except urllib.error.HTTPError as e:
                error_body = e.read().decode("utf-8") if getattr(e, "fp", None) else str(e)
                details = None
                try:
                    details = json.loads(error_body)
                except Exception:
                    pass

                # If rate limited or 5xx server error, retry with exponential backoff
                if e.code in (429, 502, 503, 504) and attempt < self.max_retries:
                    sleep_time = self.backoff_factor * (2**attempt)
                    time.sleep(sleep_time)
                    continue

                raise_for_status(e.code, error_body, details)
            except urllib.error.URLError as e:
                last_error = e
                if attempt < self.max_retries:
                    time.sleep(self.backoff_factor * (2**attempt))
                    continue
                raise SunbornConnectionError(
                    f"Failed to connect to Sunborn Gateway at {url}: {e.reason}"
                ) from e

        raise SunbornConnectionError(
            f"Failed to connect to Sunborn Gateway after {self.max_retries} retries: {last_error}"
        )

    def _get(self, path: str, custom_headers: dict | None = None) -> dict:
        url = f"{self.base_url}/{path.lstrip('/')}"
        headers = {
            "Content-Type": "application/json",
            "x-api-key": self.api_key,
            "x-tenant-id": self.tenant_id,
            "x-application-id": self.application_id,
        }
        if custom_headers:
            headers.update(custom_headers)

        last_error = None
        for attempt in range(self.max_retries + 1):
            req = urllib.request.Request(
                url,
                headers=headers,
                method="GET",
            )
            try:
                with urllib.request.urlopen(req, timeout=self.timeout) as response:
                    content = response.read().decode("utf-8")
                    return json.loads(content) if content else {}
            except urllib.error.HTTPError as e:
                error_body = e.read().decode("utf-8") if getattr(e, "fp", None) else str(e)
                details = None
                try:
                    details = json.loads(error_body)
                except Exception:
                    pass

                if e.code in (429, 502, 503, 504) and attempt < self.max_retries:
                    sleep_time = self.backoff_factor * (2**attempt)
                    time.sleep(sleep_time)
                    continue

                raise_for_status(e.code, error_body, details)
            except urllib.error.URLError as e:
                last_error = e
                if attempt < self.max_retries:
                    time.sleep(self.backoff_factor * (2**attempt))
                    continue
                raise SunbornConnectionError(
                    f"Failed to connect to Sunborn Gateway at {url}: {e.reason}"
                ) from e

        raise SunbornConnectionError(
            f"Failed to connect to Sunborn Gateway after {self.max_retries} retries: {last_error}"
        )

    def login(
        self, client_id: str, client_secret: str, tenant_id: str | None = None
    ) -> dict:
        """Authenticates with the Sunborn Gateway login endpoint (/v1/auth/login)."""
        payload: dict[str, Any] = {
            "client_id": client_id,
            "client_secret": client_secret,
        }
        if tenant_id:
            payload["tenant_id"] = tenant_id

        res = self._post("auth/login", payload)
        if "access_token" in res:
            self.api_key = res["access_token"]
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
        response_format: dict | None = None,
        tools: list[dict] | None = None,
        allow_stochastic: bool = False,
        context_digest: str | None = None,
    ) -> dict:
        """Invokes the OpenAI-compatible /chat/completions endpoint."""
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

        custom_headers = {}
        if allow_stochastic:
            custom_headers["x-sunborn-cache-policy"] = "allow-stochastic"
        if context_digest:
            custom_headers["x-sunborn-context-digest"] = context_digest

        return self._post("chat/completions", payload, custom_headers=custom_headers)

    def create_message(
        self,
        model: str = "claude-3-5-sonnet-20241022",
        messages: list[dict[str, Any]] | None = None,
        system: str | list[dict] | None = None,
        max_tokens: int | None = 1024,
        temperature: float | None = None,
        top_p: float | None = None,
        stop_sequences: list[str] | None = None,
    ) -> dict:
        """Invokes the Anthropic-compatible /messages endpoint."""
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

    def get_finops_report(self) -> dict:
        """Retrieves the live FinOps cost and token savings report."""
        return self._get("finops/report")

    def plan_execution_dag(self, plan_nodes: list[dict]) -> dict:
        """Submits an Agent Execution DAG to evaluate memoized tool subgraph reuse."""
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
        arguments: dict,
        output: Any,
        execution_latency_ms: int = 0,
        context_hash: str | None = None,
        ttl_secs: int | None = None,
        is_error: bool = False,
    ) -> dict:
        """Stores a deterministic tool execution result in the Sunborn Execution Cache."""
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


def get_default_client() -> SunbornClient:
    api_key = os.environ.get("SUNBORN_API_KEY")
    if not api_key:
        raise ValueError(
            "SunbornClient requires an API key. Set the `SUNBORN_API_KEY` environment variable or supply an explicit client."
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
):
    """
    Decorator for AI agent tools to automatically memoize deterministic calls
    via the Sunborn Execution Engine with strict fallback governance.
    Supports both synchronous and asynchronous tool functions.
    """
    import asyncio

    def decorator(fn: Callable):
        tool_name = name or fn.__name__
        is_async = inspect.iscoroutinefunction(fn)

        def _build_plan(arguments: dict) -> tuple[str, list[dict]]:
            arg_bytes = json.dumps(arguments, sort_keys=True, default=str).encode(
                "utf-8"
            )
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

        if is_async:

            @functools.wraps(fn)
            async def async_wrapper(*args, **kwargs):
                if not deterministic:
                    return await fn(*args, **kwargs)

                sunborn = client or get_default_client()
                arguments = {"args": list(args), "kwargs": kwargs}
                node_id, plan_nodes = _build_plan(arguments)

                try:
                    eval_result = await asyncio.to_thread(
                        sunborn.plan_execution_dag, plan_nodes
                    )
                    if eval_result.get("cached_nodes", 0) > 0:
                        status = eval_result["node_statuses"].get(node_id, {})
                        if status.get("status") == "Cached":
                            return status["details"]["output"]
                except Exception as e:
                    if policy == FallbackPolicy.FAIL_CLOSED:
                        raise RuntimeError(
                            f"Sunborn Policy FAIL_CLOSED: Execution plan query failed for '{tool_name}': {e}"
                        ) from e

                t0 = time.time()
                result = await fn(*args, **kwargs)
                latency_ms = int((time.time() - t0) * 1000)

                try:
                    await asyncio.to_thread(
                        sunborn.record_tool_result,
                        tool_name=tool_name,
                        tool_version=version,
                        arguments=arguments,
                        output=result,
                        execution_latency_ms=latency_ms,
                        context_hash=context_hash,
                        ttl_secs=ttl_secs,
                    )
                except Exception as e:
                    if policy == FallbackPolicy.FAIL_CLOSED:
                        raise RuntimeError(
                            f"Sunborn Policy FAIL_CLOSED: Recording tool result failed for '{tool_name}': {e}"
                        ) from e

                return result

            return async_wrapper
        else:

            @functools.wraps(fn)
            def sync_wrapper(*args, **kwargs):
                if not deterministic:
                    return fn(*args, **kwargs)

                sunborn = client or get_default_client()
                arguments = {"args": list(args), "kwargs": kwargs}
                node_id, plan_nodes = _build_plan(arguments)

                # 1. Query Gateway plan to check if memoized tool call exists
                try:
                    eval_result = sunborn.plan_execution_dag(plan_nodes)
                    if eval_result.get("cached_nodes", 0) > 0:
                        status = eval_result["node_statuses"].get(node_id, {})
                        if status.get("status") == "Cached":
                            return status["details"]["output"]
                except Exception as e:
                    if policy == FallbackPolicy.FAIL_CLOSED:
                        raise RuntimeError(
                            f"Sunborn Policy FAIL_CLOSED: Execution plan query failed for '{tool_name}': {e}"
                        ) from e

                # 2. Execute underlying function
                t0 = time.time()
                result = fn(*args, **kwargs)
                latency_ms = int((time.time() - t0) * 1000)

                # 3. Store result back into Sunborn Execution Cache
                try:
                    sunborn.record_tool_result(
                        tool_name=tool_name,
                        tool_version=version,
                        arguments=arguments,
                        output=result,
                        execution_latency_ms=latency_ms,
                        context_hash=context_hash,
                        ttl_secs=ttl_secs,
                    )
                except Exception as e:
                    if policy == FallbackPolicy.FAIL_CLOSED:
                        raise RuntimeError(
                            f"Sunborn Policy FAIL_CLOSED: Recording tool result failed for '{tool_name}': {e}"
                        ) from e

                return result

            return sync_wrapper

    return decorator


SunbornAgentClient = SunbornClient
