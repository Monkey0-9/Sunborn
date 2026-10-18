#!/usr/bin/env python3
"""
Sunborn AI Agent Integration Client & Tool Execution Reuse SDK

Provides:
1. Universal drop-in OpenAI- and Anthropic-compatible client configuration:
   client = SunbornClient(base_url="http://localhost:8080/v1", api_key="key-enterprise-fintech")
2. Anthropic Messages API support (/v1/messages) and OpenAI Chat Completions (/v1/chat/completions).
3. Deterministic Tool Memoization decorator for multi-step agent workflows:
   @sunborn_tool(name="fetch_user_balance", deterministic=True)
   def fetch_user_balance(user_id: str) -> dict: ...
4. Agent DAG execution plan submission and cache query via /v1/execution/plan and /v1/execution/results.
5. Client-side ContextDigest computation matching the Sunborn Rust Merkle root specification.
6. Asynchronous client `AsyncSunbornClient` for asyncio workflows.
"""

import asyncio
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


def compute_context_digest(
    model_id: str,
    system_prompt: str | None = None,
    rag_chunk_ids: list[str] | None = None,
) -> str:
    """
    Computes the 32-byte hexadecimal ContextDigest matching the Sunborn Rust Merkle root contract:
    - Binds model_id, system_prompt, and sorted RAG document chunk IDs.
    - Two prompts are compared for semantic reuse iff their ContextDigest matches identically.
    """
    # 1. System Prompt Hash
    if system_prompt is not None:
        h_sys = hashlib.sha256()
        h_sys.update(b"sunborn:sys_prompt:v1:")
        h_sys.update(system_prompt.encode("utf-8"))
        system_hash = h_sys.digest()
    else:
        h_sys = hashlib.sha256()
        h_sys.update(b"sunborn:sys_prompt:v1:none")
        system_hash = h_sys.digest()

    # 2. RAG Chunk IDs Merkle Root
    chunks = sorted(rag_chunk_ids) if rag_chunk_ids else []
    if not chunks:
        h_rag = hashlib.sha256()
        h_rag.update(b"sunborn:rag_root:empty")
        rag_root = h_rag.digest()
    else:
        current_hashes: list[bytes] = []
        for c in chunks:
            h_leaf = hashlib.sha256()
            h_leaf.update(b"sunborn:rag_leaf:")
            h_leaf.update(c.encode("utf-8"))
            current_hashes.append(h_leaf.digest())

        while len(current_hashes) > 1:
            next_level: list[bytes] = []
            for i in range(0, len(current_hashes), 2):
                chunk0 = current_hashes[i]
                chunk1 = (
                    current_hashes[i + 1] if i + 1 < len(current_hashes) else chunk0
                )
                h_node = hashlib.sha256()
                h_node.update(b"sunborn:rag_node:")
                h_node.update(chunk0)
                h_node.update(chunk1)
                next_level.append(h_node.digest())
            current_hashes = next_level
        rag_root = current_hashes[0]

    # 3. Final Combined Context Digest
    final_hasher = hashlib.sha256()
    final_hasher.update(b"sunborn:context_digest:v1:")
    model_bytes = model_id.encode("utf-8")
    final_hasher.update(len(model_bytes).to_bytes(4, byteorder="big"))
    final_hasher.update(model_bytes)
    final_hasher.update(system_hash)
    final_hasher.update(rag_root)

    return final_hasher.hexdigest()


class SunbornClient:
    """Lightweight Python client to interact with Sunborn Semantic AI Gateway."""

    def __init__(
        self,
        base_url: str = "http://localhost:8080/v1",
        api_key: str | None = None,
        tenant_id: str = "default-tenant",
        application_id: str = "default-app",
        timeout: float = 30.0,
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
            raise RuntimeError(f"Sunborn Gateway error [{e.code}]: {error_body}") from e
        except urllib.error.URLError as e:
            raise ConnectionError(
                f"Failed to connect to Sunborn Gateway at {url}: {e.reason}"
            ) from e

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
            raise RuntimeError(f"Sunborn Gateway error [{e.code}]: {error_body}") from e
        except urllib.error.URLError as e:
            raise ConnectionError(
                f"Failed to connect to Sunborn Gateway at {url}: {e.reason}"
            ) from e

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


class AsyncSunbornClient:
    """Asynchronous client for Sunborn Gateway supporting asyncio execution."""

    def __init__(
        self,
        base_url: str = "http://localhost:8080/v1",
        api_key: str | None = None,
        tenant_id: str = "default-tenant",
        application_id: str = "default-app",
        timeout: float = 30.0,
    ):
        self._sync_client = SunbornClient(
            base_url=base_url,
            api_key=api_key,
            tenant_id=tenant_id,
            application_id=application_id,
            timeout=timeout,
        )

    @property
    def base_url(self) -> str:
        return self._sync_client.base_url

    @property
    def tenant_id(self) -> str:
        return self._sync_client.tenant_id

    @property
    def application_id(self) -> str:
        return self._sync_client.application_id

    @property
    def api_key(self) -> str:
        return self._sync_client.api_key

    async def login(
        self, client_id: str, client_secret: str, tenant_id: str | None = None
    ) -> dict:
        """Asynchronously authenticates with the Sunborn Gateway login endpoint."""
        return await asyncio.to_thread(
            self._sync_client.login, client_id, client_secret, tenant_id
        )

    async def chat_completion(self, *args, **kwargs) -> dict:
        return await asyncio.to_thread(
            self._sync_client.chat_completion, *args, **kwargs
        )

    async def create_message(self, *args, **kwargs) -> dict:
        return await asyncio.to_thread(
            self._sync_client.create_message, *args, **kwargs
        )

    messages = create_message

    async def get_finops_report(self) -> dict:
        return await asyncio.to_thread(self._sync_client.get_finops_report)

    async def plan_execution_dag(self, plan_nodes: list[dict]) -> dict:
        return await asyncio.to_thread(self._sync_client.plan_execution_dag, plan_nodes)

    async def record_tool_result(self, *args, **kwargs) -> dict:
        return await asyncio.to_thread(
            self._sync_client.record_tool_result, *args, **kwargs
        )


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


class FallbackPolicy:
    FAIL_CLOSED = "FAIL_CLOSED"
    PERMISSIVE = "PERMISSIVE"


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
                    # In PERMISSIVE mode, proceed to execute natively

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


# Convenience alias for seamless integration
SunbornAgentClient = SunbornClient


if __name__ == "__main__":
    print("Testing Sunborn Agent SDK imports & client construction...")
    c = SunbornClient(api_key="test-key-enterprise")
    print(f"Initialized SunbornClient with base_url={c.base_url}, tenant={c.tenant_id}")
    c2 = SunbornAgentClient(api_key="test-key-enterprise")
    print("Initialized SunbornAgentClient alias successfully.")
    print("Sunborn Agent SDK ready.")
