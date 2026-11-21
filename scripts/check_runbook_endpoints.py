#!/usr/bin/env python3
"""Fail when operator documentation references an endpoint the gateway lacks.

Runbooks are executed under pressure. A curl line pointing at a route that was
renamed is worse than no runbook, because the operator spends the incident
debugging a 404. This checker extracts every documented gateway URL from the
runbooks and compares it with the routes registered in the router.

It deliberately checks paths only: request bodies and header requirements are
covered by the handler tests, and cannot be inferred reliably from prose.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ROUTER_SOURCE = ROOT / "crates" / "sunborn-gateway" / "src" / "lib.rs"
DOC_DIRS = [ROOT / "docs" / "runbooks"]
DOC_FILES = [ROOT / "README.md", ROOT / "docs" / "INSTALL.md"]
# Docs may address the gateway through any of these prefixes.
URL_PATTERN = re.compile(r"localhost:8080(/[A-Za-z0-9/_{}\-]*)")
# Inline references such as `POST /v1/graph/invalidate`.
METHOD_PATTERN = re.compile(r"\b(?:GET|POST|PUT|DELETE|PATCH) (/[A-Za-z0-9/_{}.\-]+)")
ROUTE_PATTERN = re.compile(r'"(/[A-Za-z0-9/_{}.*\-]*)"')


def canonical(path: str) -> str:
    """Normalise wildcard path params so `{id}` and `{*id}` compare equal."""
    return path.replace("{*", "{").rstrip("/") or "/"


def registered_routes() -> set[str]:
    text = ROUTER_SOURCE.read_text(encoding="utf-8")
    return {canonical(r) for r in ROUTE_PATTERN.findall(text)}


def documented_paths() -> dict[str, list[str]]:
    found: dict[str, list[str]] = {}
    sources = [p for p in DOC_FILES if p.exists()]
    for directory in DOC_DIRS:
        sources.extend(sorted(directory.glob("*.md")))
    for path in sources:
        text = path.read_text(encoding="utf-8")
        for pattern in (URL_PATTERN, METHOD_PATTERN):
            for match in pattern.finditer(text):
                found.setdefault(canonical(match.group(1)), []).append(path.name)
    return found


def main() -> int:
    routes = registered_routes()
    if not routes:
        print(f"FAIL: no routes parsed from {ROUTER_SOURCE}", file=sys.stderr)
        return 1

    missing = {
        path: sorted(set(where))
        for path, where in documented_paths().items()
        if path not in routes
    }

    if missing:
        print("Documented endpoints that the gateway does not serve:", file=sys.stderr)
        for path, where in sorted(missing.items()):
            print(f"  {path}  (referenced in {', '.join(where)})", file=sys.stderr)
        print(
            "\nEither register the route or correct the documentation. "
            "An operator following a 404 mid-incident is worse than no runbook.",
            file=sys.stderr,
        )
        return 1

    print(
        f"OK: every documented gateway endpoint exists "
        f"({len(documented_paths())} paths checked against {len(routes)} routes)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
