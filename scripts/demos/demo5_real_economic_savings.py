#!/usr/bin/env python3
"""Demo 5 — Economic savings from observed cache behaviour.

Runs a fixed workload, counts how many requests the gateway actually served
from cache, and prices the avoided origin calls at published list prices.

Labelled honestly: the hit counts are measured against the live gateway; the
prices are published list prices, not a reconciled invoice. A real FinOps report
requires provider billing data.
"""

from __future__ import annotations

import sys

from _client import banner, finish, new_client, result, step

TENANT = "tenant_demo"
APP = "app_demo"
MODEL = "gpt-4o-mini"

# Published list price for gpt-4o-mini, USD per million tokens.
INPUT_PER_MTOK = 0.15
OUTPUT_PER_MTOK = 0.60
ASSUMED_INPUT_TOKENS = 800
ASSUMED_OUTPUT_TOKENS = 300

PROMPTS = [
    "Explain semantic caching.",
    "Explain semantic caching.",  # exact repeat
    "Explain semantic caching please.",  # paraphrase
    "What is a Merkle root?",
    "What is a Merkle root?",
    "How does tenant isolation work?",
    "How does tenant isolation work?",
    "Summarise the on-call runbook.",
]


def main() -> int:
    banner("Demo 5 — Measured reuse counts, priced at published list rates")
    checks: list[tuple[str, bool]] = []

    origin_cost = (
        ASSUMED_INPUT_TOKENS / 1_000_000 * INPUT_PER_MTOK
        + ASSUMED_OUTPUT_TOKENS / 1_000_000 * OUTPUT_PER_MTOK
    )

    calls = 0
    reuses = 0
    with new_client() as client:
        for prompt in PROMPTS:
            response = client.post(
                "/v1/requests",
                json={
                    "tenant_id": TENANT,
                    "application_id": APP,
                    "model": MODEL,
                    "prompt": prompt,
                },
            )
            if response.status_code >= 400:
                sys.exit(f"POST /v1/requests -> {response.status_code}: {response.text[:500]}")
            payload = response.json()
            calls += 1
            if payload.get("cached"):
                reuses += 1
            step(f"cached={str(bool(payload.get('cached'))):5s}  {prompt}")

    avoided = reuses * origin_cost
    paid = calls * origin_cost
    savings_pct = (avoided / paid * 100.0) if paid else 0.0

    print()
    print(f"  calls                    : {calls}")
    print(f"  served from cache        : {reuses}")
    print(f"  cost per origin call     : ${origin_cost:.8f}")
    print(f"  modelled cost without    : ${paid:.8f}")
    print(f"  modelled cost avoided    : ${avoided:.8f}")
    print(f"  modelled saving          : {savings_pct:.2f}%")
    print("  note: prices are published list rates, not a reconciled invoice.")

    checks.append(("the workload produced at least one reuse", reuses > 0))
    checks.append(("modelled saving is non-negative", avoided >= 0.0))

    for name, ok in checks:
        result(name, ok)
    return finish(checks)


if __name__ == "__main__":
    sys.exit(main())
