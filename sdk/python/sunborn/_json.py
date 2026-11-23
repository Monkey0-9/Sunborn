"""JSON helpers with optional orjson/msgspec acceleration.

The standard library is always the fallback, so the SDK has no mandatory
third-party runtime dependency beyond ``httpx``.
"""

from __future__ import annotations

import json
from typing import Any

try:  # pragma: no cover - import-time capability probe
    import orjson as _orjson
except ImportError:  # pragma: no cover
    _orjson = None

try:  # pragma: no cover - import-time capability probe
    import msgspec as _msgspec
except ImportError:  # pragma: no cover
    _msgspec = None


def dumps(payload: Any) -> bytes:
    """Serialize *payload* to UTF-8 bytes."""
    if _orjson is not None:
        result: bytes = _orjson.dumps(payload)
        return result
    if _msgspec is not None:
        encoded: bytes = _msgspec.json.encode(payload)
        return encoded
    return json.dumps(payload, default=str, ensure_ascii=False).encode("utf-8")


def loads(content: str | bytes) -> Any:
    """Deserialize JSON from text or bytes."""
    if _orjson is not None:
        return _orjson.loads(content)
    if _msgspec is not None:
        return _msgspec.json.decode(content)
    if isinstance(content, bytes):
        content = content.decode("utf-8")
    return json.loads(content)
