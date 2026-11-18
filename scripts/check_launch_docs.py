#!/usr/bin/env python3
"""Fail on broken launch-document links or unqualified launch claims.

This gate is intentionally narrow. It checks the launch package, its root
entry points, and a small blocklist of claims that the launch docs promise not
to make. A line is allowed to mention a blocked term when it also contains an
explicit prohibition ("no", "not", "never", "without", "out of scope"), so the
policy statements themselves do not trip the gate.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LAUNCH_DIR = ROOT / "docs" / "launch"

REQUIRED = [
    ROOT / "README.md",
    ROOT / "GOVERNANCE.md",
    ROOT / "ROADMAP.md",
    LAUNCH_DIR / "README.md",
    LAUNCH_DIR / "01_LAUNCH_PLAN.md",
    LAUNCH_DIR / "02_FREE_TIER_ARCHITECTURE.md",
    LAUNCH_DIR / "03_CAPACITY_AND_SLO.md",
    LAUNCH_DIR / "04_TRUST_CENTER.md",
    LAUNCH_DIR / "05_PRELAUNCH_CHECKLIST.md",
]

LINK_DOCS = [ROOT / "README.md", ROOT / "GOVERNANCE.md", ROOT / "ROADMAP.md"] + sorted(
    LAUNCH_DIR.glob("*.md")
)

BANNED = [
    (
        re.compile(
            r"\b(GPTCache|Helicone|Portkey|LiteLLM|LangChain|Pinecone|Weaviate|Milvus)\b",
            re.IGNORECASE,
        ),
        "named competitor",
    ),
    (
        re.compile(
            r"\b(headcount|FTE|full[- ]time employees?|staffing plan|team of \d+|our engineers)\b",
            re.IGNORECASE,
        ),
        "headcount claim",
    ),
]

# A blocked term is acceptable on a line that explicitly disclaims it.
NEGATION = re.compile(
    r"\b(no|not|never|without|avoid|prohibit|forbid|out of scope|do not|does not)\b",
    re.IGNORECASE,
)

INLINE_LINK = re.compile(r"(?<!!)\[[^\]]*\]\(([^)]+)\)")
FENCE = re.compile(r"```.*?```", re.DOTALL)


def strip_fenced_code(text: str) -> str:
    """Keep line numbers stable while hiding code samples from the scanners."""
    return FENCE.sub(lambda match: "\n" * match.group(0).count("\n"), text)


def check_required() -> list[str]:
    return [f"missing required document: {path.relative_to(ROOT)}" for path in REQUIRED if not path.is_file()]


def check_links() -> tuple[int, list[str]]:
    failures: list[str] = []
    checked = 0
    for doc in LINK_DOCS:
        text = strip_fenced_code(doc.read_text(encoding="utf-8"))
        for match in INLINE_LINK.finditer(text):
            target = match.group(1).strip()
            if target.startswith(("http://", "https://", "mailto:", "#")):
                continue
            target = target.split("#", 1)[0]
            if not target:
                continue
            checked += 1
            resolved = (doc.parent / target).resolve()
            if not resolved.exists():
                line = text.count("\n", 0, match.start()) + 1
                failures.append(
                    f"{doc.relative_to(ROOT)}:{line}: broken link -> {match.group(1)}"
                )
    return checked, failures


def check_claims() -> tuple[int, list[str]]:
    failures: list[str] = []
    scanned = 0
    for doc in LINK_DOCS:
        for line_no, line in enumerate(doc.read_text(encoding="utf-8").splitlines(), 1):
            scanned += 1
            if NEGATION.search(line):
                continue
            for pattern, label in BANNED:
                if pattern.search(line):
                    failures.append(
                        f"{doc.relative_to(ROOT)}:{line_no}: {label} -> {line.strip()}"
                    )
    return scanned, failures


def main() -> int:
    failures = check_required()
    link_count, link_failures = check_links()
    line_count, claim_failures = check_claims()
    failures.extend(link_failures)
    failures.extend(claim_failures)

    if failures:
        print("Launch documentation gate FAILED:", file=sys.stderr)
        for failure in failures:
            print(f"  - {failure}", file=sys.stderr)
        return 1

    print(
        f"Launch documentation gate OK: {len(REQUIRED)} required documents, "
        f"{link_count} relative links across {len(LINK_DOCS)} files, "
        f"{line_count} lines scanned for unqualified claims."
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
