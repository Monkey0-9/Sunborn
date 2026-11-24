"""Shared helpers for the Sunborn launch demos.

Every demo talks to a real gateway and prints values it actually received.
Nothing here simulates a response. If the gateway is unreachable the demo
exits non-zero and says so, because a demo that invents its own evidence is
worse than no demo.

Environment:
    SUNBORN_URL       gateway base URL        (default http://localhost:8080)
    SUNBORN_API_KEY   admin API key           (required)
    SUNBORN_TENANT    tenant for demo traffic (default tenant_demo)
"""

from __future__ import annotations

import os
import sys

try:
    import httpx
except ImportError:  # pragma: no cover - environment dependent
    sys.exit("demos require httpx: python -m pip install httpx")

BASE_URL = os.environ.get("SUNBORN_URL", "http://localhost:8080")
API_KEY = os.environ.get("SUNBORN_API_KEY", "")
TENANT = os.environ.get("SUNBORN_TENANT", "tenant_demo")
APPLICATION = "app_demo"
MODEL = "gpt-4o-mini"


def require_config() -> None:
    if not API_KEY:
        sys.exit(
            "SUNBORN_API_KEY is not set.\n"
            "Start the gateway, create an admin key, then re-run:\n"
            "  export SUNBORN_API_KEY=<your key>"
        )


def new_client() -> httpx.Client:
    require_config()
    return httpx.Client(
        base_url=BASE_URL.rstrip("/"),
        timeout=30.0,
        headers={"x-api-key": API_KEY, "content-type": "application/json"},
    )


def post(client: httpx.Client, path: str, body: dict) -> dict:
    """POSTs and returns JSON, failing loudly with the server's own message."""
    response = client.post(path, json=body)
    if response.status_code >= 400:
        sys.exit(f"POST {path} -> {response.status_code}: {response.text[:800]}")
    return response.json() if response.content else {}


def ask(client: httpx.Client, prompt: str, **overrides) -> dict:
    """Issues a normal /v1/requests call for this tenant."""
    body = {
        "tenant_id": TENANT,
        "application_id": APPLICATION,
        "model": MODEL,
        "prompt": prompt,
    }
    body.update(overrides)
    return post(client, "/v1/requests", body)


def banner(title: str) -> None:
    print()
    print("=" * 72)
    print(title)
    print("=" * 72)


def step(text: str) -> None:
    print(f"  -> {text}")


def result(label: str, ok: bool) -> None:
    print(f"  [{'PASS' if ok else 'FAIL'}] {label}")


def finish(checks: list[tuple[str, bool]]) -> int:
    failed = [name for name, ok in checks if not ok]
    print()
    if failed:
        print(f"DEMO FAILED: {len(failed)} check(s) did not hold: {', '.join(failed)}")
        return 1
    print(f"DEMO PASSED: {len(checks)}/{len(checks)} checks held against the live gateway.")
    return 0
