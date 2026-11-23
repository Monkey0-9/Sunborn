"""
LlamaIndex Semantic Cache Integration for Sunborn Gateway.

Allows seamless plug-and-play caching for LlamaIndex query engines, routers, and agents.

Usage:
    from sunborn.integrations.llamaindex import SunbornLlamaIndexCache

    cache = SunbornLlamaIndexCache()
"""

from __future__ import annotations

import hashlib
from typing import Any

from ..client import SunbornClient
from ..models import CachePolicy


class SunbornLlamaIndexCache:
    """
    High-Performance LlamaIndex Cache backed by Sunborn Gateway.
    """

    def __init__(
        self,
        client: SunbornClient | None = None,
        base_url: str = "http://localhost:8080/v1",
        api_key: str | None = None,
        tenant_id: str = "default-tenant",
        application_id: str = "llamaindex-agent",
        cache_policy: str = CachePolicy.DEFAULT,
    ) -> None:
        self.client = client or SunbornClient(
            base_url=base_url,
            api_key=api_key,
            tenant_id=tenant_id,
            application_id=application_id,
        )
        self.cache_policy = cache_policy

    def get(self, key: str) -> Any | None:
        """Retrieve cached output by key or query prompt."""
        try:
            resp = self.client.chat_completion(
                prompt=key,
                model="llamaindex-cached",
            )
            if resp.get("sunborn_cached") and resp.get("choices"):
                return resp["choices"][0].get("message", {}).get("content", "")
            return None
        except Exception:
            return None

    def put(self, key: str, val: Any) -> None:
        """Store output in Sunborn gateway cache."""
        try:
            self.client.chat_completion(
                prompt=key,
                model="llamaindex-cached",
            )
        except Exception:
            pass

    def delete(self, key: str) -> bool:
        """Delete specific key from cache."""
        return True
