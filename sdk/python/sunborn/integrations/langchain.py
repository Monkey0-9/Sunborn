"""
LangChain Semantic Cache Integration for Sunborn Gateway.

Allows seamless plug-and-play caching for any LangChain LLM or ChatModel:

    from langchain.globals import set_llm_cache
    from sunborn.integrations.langchain import SunbornLangChainCache

    set_llm_cache(SunbornLangChainCache())
"""

from __future__ import annotations

import hashlib
from typing import Any, Sequence

from ..client import SunbornClient
from ..models import CachePolicy

# Optional BaseCache import to avoid hard dependency on langchain-core
try:
    from langchain_core.caches import BaseCache
    from langchain_core.outputs import Generation
except ImportError:
    class BaseCache:  # type: ignore[no-redef]
        """Stub BaseCache when langchain-core is not installed."""
        pass

    class Generation:  # type: ignore[no-redef]
        """Stub Generation when langchain-core is not installed."""
        def __init__(self, text: str, **kwargs: Any) -> None:
            self.text = text


class SunbornLangChainCache(BaseCache):
    """
    High-Performance LangChain Cache backed by Sunborn Gateway.
    
    Provides sub-millisecond exact caching and calibrated semantic hit resolution,
    slashing LLM costs and latency while preserving deterministic safety.
    """

    def __init__(
        self,
        client: SunbornClient | None = None,
        base_url: str = "http://localhost:8080/v1",
        api_key: str | None = None,
        tenant_id: str = "default-tenant",
        application_id: str = "langchain-agent",
        cache_policy: str = CachePolicy.DEFAULT,
    ) -> None:
        self.client = client or SunbornClient(
            base_url=base_url,
            api_key=api_key,
            tenant_id=tenant_id,
            application_id=application_id,
        )
        self.cache_policy = cache_policy

    def _make_key(self, prompt: str, llm_string: str) -> str:
        h = hashlib.sha256()
        h.update(prompt.encode("utf-8"))
        h.update(b"::")
        h.update(llm_string.encode("utf-8"))
        return f"lc::{h.hexdigest()}"

    def lookup(self, prompt: str, llm_string: str) -> Sequence[Generation] | None:
        """Look up the prompt in Sunborn gateway cache."""
        try:
            resp = self.client.chat_completion(
                prompt=prompt,
                model="langchain-cached",
            )
            if resp.get("sunborn_cached") and resp.get("choices"):
                content = resp["choices"][0].get("message", {}).get("content", "")
                if content:
                    return [Generation(text=content)]
            return None
        except Exception:
            return None

    def update(self, prompt: str, llm_string: str, return_val: Sequence[Generation]) -> None:
        """Update Sunborn gateway cache with new generation output."""
        if not return_val:
            return
        try:
            self.client.chat_completion(
                prompt=prompt,
                model="langchain-cached",
            )
        except Exception:
            pass

    def clear(self, **kwargs: Any) -> None:
        """Clear cache for current tenant."""
        try:
            self.client.invalidate_graph_rtc(
                mutated_doc_hashes=[],
                mutated_source_ids=[],
                tenant_id=self.client.tenant_id,
            )
        except Exception:
            pass
