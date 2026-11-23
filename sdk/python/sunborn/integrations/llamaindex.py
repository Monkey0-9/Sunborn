"""
LlamaIndex cache integration backed by Sunborn's execution-reuse endpoints.

Each key is stored under its own tool signature, so ``delete(key)`` evicts
exactly one entry (the eviction endpoint is administrative).

    from sunborn.integrations.llamaindex import SunbornLlamaIndexCache

    cache = SunbornLlamaIndexCache()
"""

from __future__ import annotations

import asyncio
import hashlib
from typing import Any

from ..client import SunbornClient

_TOOL_PREFIX = "llamaindex-cache"
_TOOL_VERSION = "1"


class SunbornLlamaIndexCache:
    """LlamaIndex-style cache using Sunborn execution reuse."""

    def __init__(
        self,
        client: SunbornClient | None = None,
        base_url: str = "http://localhost:8080/v1",
        api_key: str | None = None,
        tenant_id: str = "default-tenant",
        application_id: str = "llamaindex-agent",
        ttl_secs: int | None = None,
        fail_closed: bool = False,
    ) -> None:
        self.client = client or SunbornClient(
            base_url=base_url,
            api_key=api_key,
            tenant_id=tenant_id,
            application_id=application_id,
        )
        self.ttl_secs = ttl_secs
        self.fail_closed = fail_closed

    @staticmethod
    def _tool_name(key: str) -> str:
        digest = hashlib.sha256(key.encode("utf-8")).hexdigest()[:16]
        return f"{_TOOL_PREFIX}:{digest}"

    def _plan(self, key: str) -> tuple[str, list[dict[str, Any]]]:
        tool_name = self._tool_name(key)
        arguments = {"cache_key": key}
        node_id = f"node-{tool_name}"
        node = {
            "id": node_id,
            "name": tool_name,
            "dependencies": [],
            "is_deterministic": True,
            "effect": "PureDeterministic"
            if self.ttl_secs is None
            else {"TimeSensitiveNonDeterministic": {"max_ttl_secs": self.ttl_secs}},
            "tool_call": {
                "tool_name": tool_name,
                "tool_version": _TOOL_VERSION,
                "arguments": arguments,
                "context_hash": None,
            },
        }
        return node_id, [node]

    def get(self, key: str) -> Any | None:
        """Return the cached value for ``key``, or ``None`` on a miss."""
        node_id, nodes = self._plan(key)
        try:
            evaluation = self.client.plan_execution_dag(nodes)
        except Exception:
            if self.fail_closed:
                raise
            return None

        status = (evaluation.get("node_statuses") or {}).get(node_id) or {}
        if status.get("status") != "Cached":
            return None
        details = status.get("details") or {}
        output = details.get("output")
        if isinstance(output, dict) and "value" in output:
            return output["value"]
        return output

    def put(self, key: str, val: Any) -> None:
        """Store ``val`` under ``key``."""
        tool_name = self._tool_name(key)
        try:
            self.client.record_tool_result(
                tool_name=tool_name,
                tool_version=_TOOL_VERSION,
                arguments={"cache_key": key},
                output={"value": val},
                execution_latency_ms=0,
                ttl_secs=self.ttl_secs,
            )
        except Exception:
            if self.fail_closed:
                raise

    def delete(self, key: str) -> bool:
        """Evict one entry. Returns ``False`` when the gateway refuses."""
        try:
            self.client.evict_execution(tool_name=self._tool_name(key))
            return True
        except Exception:
            if self.fail_closed:
                raise
            return False

    async def aget(self, key: str) -> Any | None:
        return await asyncio.to_thread(self.get, key)

    async def aput(self, key: str, val: Any) -> None:
        await asyncio.to_thread(self.put, key, val)

    async def adelete(self, key: str) -> bool:
        return await asyncio.to_thread(self.delete, key)
