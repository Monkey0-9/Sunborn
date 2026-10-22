"""Emit isolated, low-privilege API credentials for the live benchmark runner."""

import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from benchmarks.semantic_cache_bench import BENCHMARK_SUITE


def build_benchmark_credentials() -> list[dict[str, object]]:
    return [
        {
            "key": f"sunborn-benchmark-{item['id']}",
            "tenant": "enterprise-fintech",
            "app": f"bench-{item['id']}",
            "is_admin": False,
            "allowed_models": ["gpt-4o"],
        }
        for item in BENCHMARK_SUITE
    ]


if __name__ == "__main__":
    print(json.dumps(build_benchmark_credentials(), separators=(",", ":")))
