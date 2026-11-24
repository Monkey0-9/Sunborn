#!/usr/bin/env python3
"""Demo 2 — Instant causal invalidation.

Two answers are cached. The authoritative source behind one of them is bumped
via /v1/invalidation. The dependent answer stops being reusable; the unrelated
answer stays hot. This is the difference between causal invalidation and a TTL.
"""

from __future__ import annotations

import sys
import uuid

from _client import banner, finish, new_client, post, result, step

TENANT = "tenant_demo"
APP = "app_demo"
MODEL = "gpt-4o-mini"


def body(prompt: str, snapshots=None) -> dict:
    payload = {"tenant_id": TENANT, "application_id": APP, "model": MODEL, "prompt": prompt}
    if snapshots is not None:
        payload["dependency_snapshots"] = snapshots
    return payload


def main() -> int:
    banner("Demo 2 — Causal invalidation: retire the dependent answer, keep the unrelated one")
    checks: list[tuple[str, bool]] = []
    source_id = str(uuid.uuid4())

    with new_client() as client:
        dependent = "What does the 2026 travel policy say about cabin class?"
        unrelated = "How do I rotate an API key?"

        snapshots = [{"source_id": source_id, "snapshot_epoch": 1, "content_hash": "policy-v1"}]

        step("cache the dependent answer against travel-policy v1")
        post(client, "/v1/requests", body(dependent, snapshots))
        step("cache an unrelated answer with no snapshot")
        post(client, "/v1/requests", body(unrelated))

        before = post(client, "/v1/requests", body(dependent, snapshots))
        step(f"dependent before invalidation: cached={before.get('cached')}")
        checks.append(("dependent answer was hot before invalidation", bool(before.get("cached"))))

        step(f"publish travel-policy v2 (source_id={source_id})")
        post(
            client,
            "/v1/invalidation",
            {"source_id": source_id, "tenant_id": TENANT, "version_tag": "v2"},
        )

        after = post(client, "/v1/requests", body(dependent, snapshots))
        step(f"dependent after invalidation: cached={after.get('cached')}")
        checks.append(("dependent answer stopped being reusable", not bool(after.get("cached"))))

        other = post(client, "/v1/requests", body(unrelated))
        step(f"unrelated answer: cached={other.get('cached')}")
        checks.append(("unrelated answer stayed hot", bool(other.get("cached"))))

    for name, ok in checks:
        result(name, ok)
    return finish(checks)


if __name__ == "__main__":
    sys.exit(main())
