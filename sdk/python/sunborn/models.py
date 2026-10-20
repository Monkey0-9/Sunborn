"""
Data models, policies, and Merkle context digest computation for Sunborn SDK.
"""

from dataclasses import dataclass, field
import hashlib
from typing import Any


class CachePolicy:
    DEFAULT = "default"
    ALLOW_STOCHASTIC = "allow-stochastic"
    BYPASS_CACHE = "bypass-cache"
    FORCE_REFRESH = "force-refresh"


class FallbackPolicy:
    FAIL_CLOSED = "FAIL_CLOSED"
    PERMISSIVE = "PERMISSIVE"


class ExecutionTier:
    EXACT = "exact"
    SEMANTIC = "semantic"
    EXECUTION = "execution"
    ORIGIN_FRESH = "origin_fresh"
    FALLBACK = "fallback"


@dataclass
class ToolResultRecord:
    tool_name: str
    tool_version: str
    arguments: dict[str, Any]
    output: Any
    execution_latency_ms: int = 0
    context_hash: str | None = None
    ttl_secs: int | None = None
    is_error: bool = False


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
