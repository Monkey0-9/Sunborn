"""
LangChain cache integration backed by Sunborn's execution-reuse endpoints.

``update`` stores the returned generations; ``lookup`` asks the gateway's
execution planner whether that exact generation is already memoized. Nothing is
fabricated when the gateway is unavailable: the cache abstains unless
``fail_closed`` is set.

    from langchain.globals import set_llm_cache
    from sunborn.integrations.langchain import SunbornLangChainCache

    set_llm_cache(SunbornLangChainCache())
"""

from __future__ import annotations

import asyncio
import hashlib
from collections.abc import Sequence
from typing import Any

from ..client import SunbornClient

try:  # Optional dependency: the adapter works without langchain installed.
    from langchain_core.caches import BaseCache
    from langchain_core.outputs import Generation
except ImportError:

    class BaseCache:  # type: ignore[no-redef]
        """Placeholder for ``langchain_core.caches.BaseCache``."""

    class Generation:  # type: ignore[no-redef]
        """Placeholder for ``langchain_core.outputs.Generation``."""

        def __init__(self, text: str, **kwargs: Any) -> None:
            self.text = text


_TOOL_NAME = "langchain-cache"
_TOOL_VERSION = "1"


def _texts_from_output(output: Any) -> list[str] | None:
    if isinstance(output, dict):
        texts = output.get("texts")
        if isinstance(texts, list):
            return [str(item) for item in texts]
        return None
    if isinstance(output, list):
        return [str(item) for item in output]
    if isinstance(output, str):
        return [output]
    return None


class SunbornLangChainCache(BaseCache):
    """LangChain ``BaseCache`` implementation using Sunborn execution reuse."""

    def __init__(
        self,
        client: SunbornClient | None = None,
        base_url: str = "http://localhost:8080/v1",
        api_key: str | None = None,
        tenant_id: str = "default-tenant",
        application_id: str = "langchain-agent",
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

    def _plan(self, prompt: str, llm_string: str) -> tuple[str, dict[str, Any], list[dict[str, Any]]]:
        cache_key = hashlib.sha256(f"{prompt}\x00{llm_string}".encode()).hexdigest()
        arguments = {
            "cache_key": cache_key,
            "prompt": prompt,
            "llm_string": llm_string,
        }
        node_id = f"node-{_TOOL_NAME}-{cache_key[:16]}"
        node = {
            "id": node_id,
            "name": _TOOL_NAME,
            "dependencies": [],
            "is_deterministic": True,
            "effect": "PureDeterministic"
            if self.ttl_secs is None
            else {"TimeSensitiveNonDeterministic": {"max_ttl_secs": self.ttl_secs}},
            "tool_call": {
                "tool_name": _TOOL_NAME,
                "tool_version": _TOOL_VERSION,
                "arguments": arguments,
                "context_hash": None,
            },
        }
        return node_id, arguments, [node]

    def lookup(self, prompt: str, llm_string: str) -> Sequence[Generation] | None:
        """Return cached generations, or ``None`` on a miss."""
        node_id, _arguments, nodes = self._plan(prompt, llm_string)
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
        texts = _texts_from_output(details.get("output"))
        if texts is None:
            return None
        return [Generation(text=text) for text in texts]

    def update(self, prompt: str, llm_string: str, return_val: Sequence[Generation]) -> None:
        """Store the generations that were just produced."""
        if not return_val:
            return
        _node_id, arguments, _nodes = self._plan(prompt, llm_string)
        texts = [generation.text for generation in return_val]
        try:
            self.client.record_tool_result(
                tool_name=_TOOL_NAME,
                tool_version=_TOOL_VERSION,
                arguments=arguments,
                output={"texts": texts},
                execution_latency_ms=0,
                ttl_secs=self.ttl_secs,
            )
        except Exception:
            if self.fail_closed:
                raise

    def clear(self, **kwargs: Any) -> None:
        """Evict every entry recorded by this adapter (administrative endpoint)."""
        try:
            self.client.evict_execution(tool_name=_TOOL_NAME)
        except Exception:
            if self.fail_closed:
                raise

    async def alookup(self, prompt: str, llm_string: str) -> Sequence[Generation] | None:
        return await asyncio.to_thread(self.lookup, prompt, llm_string)

    async def aupdate(self, prompt: str, llm_string: str, return_val: Sequence[Generation]) -> None:
        await asyncio.to_thread(self.update, prompt, llm_string, return_val)

    async def aclear(self, **kwargs: Any) -> None:
        await asyncio.to_thread(self.clear, **kwargs)
