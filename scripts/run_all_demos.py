#!/usr/bin/env python3
"""Runs all five launch demos in order and reports a single exit code.

Each demo is a separate process against the live gateway, so one failure does
not hide the others' output.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

DEMOS = [
    "demo1_semantic_correctness.py",
    "demo2_causal_invalidation.py",
    "demo3_agentic_subgraph_reuse.py",
    "demo4_verifiable_evidence.py",
    "demo5_real_economic_savings.py",
]


def main() -> int:
    here = Path(__file__).resolve().parent
    demo_dir = here / "demos"
    results: list[tuple[str, int]] = []

    for name in DEMOS:
        print(f"\n### running {name}\n")
        completed = subprocess.run([sys.executable, str(demo_dir / name)], cwd=str(demo_dir))
        results.append((name, completed.returncode))

    print("\n" + "=" * 72)
    print("Summary")
    print("=" * 72)
    for name, code in results:
        print(f"  {'PASS' if code == 0 else 'FAIL'}  {name}")

    failed = [name for name, code in results if code != 0]
    if failed:
        print(f"\n{len(failed)} demo(s) failed: {', '.join(failed)}")
        return 1
    print(f"\nAll {len(results)} demos passed against the live gateway.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
