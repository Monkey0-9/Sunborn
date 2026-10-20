#!/usr/bin/env python3
"""
Sunborn AI Agent Integration Client & Tool Execution Reuse SDK (Backward-Compatibility Module).
Re-exports the core package components from `sunborn` while maintaining module attributes
for test patchers.
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

from sunborn import (
    AsyncSunbornClient,
    CachePolicy,
    ExecutionTier,
    FallbackPolicy,
    SunbornAgentClient,
    SunbornAuthError,
    SunbornClient,
    SunbornConnectionError,
    SunbornError,
    SunbornForbiddenError,
    SunbornNotFoundError,
    SunbornRateLimitError,
    SunbornServerError,
    SunbornValidationError,
    ToolResultRecord,
    compute_context_digest,
    get_default_client,
    sunborn_tool,
)

__all__ = [
    "SunbornClient",
    "AsyncSunbornClient",
    "SunbornAgentClient",
    "get_default_client",
    "sunborn_tool",
    "CachePolicy",
    "FallbackPolicy",
    "ExecutionTier",
    "ToolResultRecord",
    "compute_context_digest",
    "SunbornError",
    "SunbornConnectionError",
    "SunbornAuthError",
    "SunbornForbiddenError",
    "SunbornNotFoundError",
    "SunbornRateLimitError",
    "SunbornValidationError",
    "SunbornServerError",
]

if __name__ == "__main__":
    print("Testing Sunborn Agent SDK imports & client construction...")
    c = SunbornClient(api_key="test-key-enterprise")
    print(f"Initialized SunbornClient with base_url={c.base_url}, tenant={c.tenant_id}")
    c2 = SunbornAgentClient(api_key="test-key-enterprise")
    print("Initialized SunbornAgentClient alias successfully.")
    print("Sunborn Agent SDK ready.")
