#!/usr/bin/env python3
"""
Sunborn AI Agent Integration Client & Tool Execution Reuse SDK

Provides:
1. Universal drop-in OpenAI client configuration:
   client = SunbornClient(base_url="http://localhost:8080/v1", api_key="key-enterprise-fintech")
2. Deterministic Tool Memoization decorator for multi-step agent workflows:
   @sunborn_tool(name="fetch_user_balance", deterministic=True)
   def fetch_user_balance(user_id: str) -> dict: ...
3. Agent DAG execution plan submission and cache query via /v1/execution/plan
"""

import os
import json
import time
import functools
import hashlib
import urllib.request
import urllib.error
from typing import Any, Callable, Dict, List, Optional

class SunbornClient:
    """Lightweight Python client to interact with Sunborn Semantic AI Gateway."""

    def __init__(
        self,
        base_url: str = "http://localhost:8080/v1",
        api_key: Optional[str] = None,
        tenant_id: str = "default-tenant",
        application_id: str = "default-app",
        timeout: float = 30.0,
    ):
        self.base_url = base_url.rstrip("/")
        resolved_key = api_key or os.environ.get("SUNBORN_API_KEY")
        if not resolved_key:
            raise ValueError(
                "SunbornClient requires an API key. Pass `api_key` explicitly or set the `SUNBORN_API_KEY` environment variable."
            )
        self.api_key = resolved_key
        self.tenant_id = tenant_id
        self.application_id = application_id
        self.timeout = timeout

    def _post(self, path: str, payload: dict, custom_headers: Optional[dict] = None) -> dict:
        url = f"{self.base_url}/{path.lstrip('/')}"
        data = json.dumps(payload).encode("utf-8")
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
                return json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            error_body = e.read().decode("utf-8")
            raise RuntimeError(f"Sunborn Gateway error [{e.code}]: {error_body}")

    def _get(self, path: str, custom_headers: Optional[dict] = None) -> dict:
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
                return json.loads(response.read().decode("utf-8"))
        except urllib.error.HTTPError as e:
            error_body = e.read().decode("utf-8")
            raise RuntimeError(f"Sunborn Gateway error [{e.code}]: {error_body}")

    def login(self, client_id: str, client_secret: str, tenant_id: Optional[str] = None) -> dict:
        """
        Authenticates with the Sunborn Gateway login endpoint (/v1/auth/login)
        and configures the client credentials from the issued token.
        """
        payload: Dict[str, Any] = {
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
        messages: Optional[List[Dict[str, str]]] = None,
        prompt: Optional[str] = None,
        system_prompt: Optional[str] = None,
        temperature: Optional[float] = None,
        tools: Optional[List[dict]] = None,
    ) -> dict:
        """Invokes the OpenAI-compatible /chat/completions endpoint."""
        msgs = list(messages) if messages else []
        if prompt:
            if system_prompt:
                msgs.append({"role": "system", "content": system_prompt})
            msgs.append({"role": "user", "content": prompt})

        payload: Dict[str, Any] = {
            "model": model,
            "messages": msgs,
        }
        if temperature is not None:
            payload["temperature"] = temperature
        if tools is not None:
            payload["tools"] = tools

        return self._post("chat/completions", payload)

    def get_finops_report(self) -> dict:
        """Retrieves the live FinOps cost and token savings report."""
        return self._get("finops/report")

    def plan_execution_dag(self, plan_nodes: List[dict]) -> dict:
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
    ) -> dict:
        """Stores a deterministic tool execution result in the Sunborn Execution Cache."""
        payload = {
            "tenant_id": self.tenant_id,
            "application_id": self.application_id,
            "tool_name": tool_name,
            "tool_version": tool_version,
            "arguments": arguments,
            "output": output,
            "is_error": False,
            "execution_latency_ms": execution_latency_ms,
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


class FallbackPolicy:
    FAIL_CLOSED = "FAIL_CLOSED"
    PERMISSIVE = "PERMISSIVE"


def sunborn_tool(
    name: Optional[str] = None,
    version: str = "1.0",
    deterministic: bool = True,
    policy: str = FallbackPolicy.FAIL_CLOSED,
    client: Optional[SunbornClient] = None,
):
    """
    Decorator for AI agent tools to automatically memoize deterministic calls
    via the Sunborn Execution Engine with strict fallback governance.
    """
    def decorator(fn: Callable):
        tool_name = name or fn.__name__

        @functools.wraps(fn)
        def wrapper(*args, **kwargs):
            if not deterministic:
                return fn(*args, **kwargs)

            sunborn = client or get_default_client()
            arguments = {"args": list(args), "kwargs": kwargs}

            # 1. Query Gateway plan to check if memoized tool call exists
            node_id = f"node-{tool_name}-{hashlib.sha256(json.dumps(arguments, sort_keys=True).encode()).hexdigest()[:8]}"
            plan_nodes = [
                {
                    "id": node_id,
                    "name": tool_name,
                    "dependencies": [],
                    "is_deterministic": True,
                    "tool_call": {
                        "tool_name": tool_name,
                        "tool_version": version,
                        "arguments": arguments,
                    },
                }
            ]

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
                    )
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
                )
            except Exception as e:
                if policy == FallbackPolicy.FAIL_CLOSED:
                    raise RuntimeError(
                        f"Sunborn Policy FAIL_CLOSED: Recording tool result failed for '{tool_name}': {e}"
                    )

            return result

        return wrapper

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
