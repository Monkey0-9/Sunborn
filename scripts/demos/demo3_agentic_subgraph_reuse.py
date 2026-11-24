#!/usr/bin/env python3
"""Demo 3 — Agentic partial execution reuse.

A three-node agent workflow is planned twice: read a balance (read-only),
compute an interest figure (pure), then execute a payment (non-idempotent
mutation). The first two nodes may be reused; the payment node must never be,
no matter how many times the workflow runs.
"""

from __future__ import annotations

import sys

from _client import banner, finish, new_client, post, result, step

TENANT = "tenant_demo"
APP = "app_demo"


def plan_payload(balance: dict) -> dict:
    return {
        "tenant_id": TENANT,
        "application_id": APP,
        "plan": {
            "nodes": [
                {
                    "id": "read_balance",
                    "name": "read_balance",
                    "dependencies": [],
                    "tool_call": {
                        "tool_name": "balance_api",
                        "tool_version": "1",
                        "arguments": balance,
                        "context_hash": None,
                    },
                    "is_deterministic": True,
                    "effect": {"ReadOnlyIdempotent": {"required_state_version": 1, "source_ids": []}},
                },
                {
                    "id": "calc_interest",
                    "name": "calc_interest",
                    "dependencies": ["read_balance"],
                    "tool_call": None,
                    "is_deterministic": True,
                    "effect": "PureDeterministic",
                },
                {
                    "id": "execute_payment",
                    "name": "execute_payment",
                    "dependencies": ["calc_interest"],
                    "tool_call": {
                        "tool_name": "payments",
                        "tool_version": "1",
                        "arguments": {"amount": 100},
                        "context_hash": None,
                    },
                    "is_deterministic": False,
                    "effect": {
                        "MutatingSideEffecting": {
                            "operation_type": "payment",
                            "idempotency_key": "demo-payment",
                        }
                    },
                },
            ]
        },
    }


def record(client, balance: dict) -> None:
    post(
        client,
        "/v1/execution/results",
        {
            "tenant_id": TENANT,
            "application_id": APP,
            "tool_name": "balance_api",
            "tool_version": "1",
            "arguments": balance,
            "context_hash": None,
            "output": balance,
            "is_error": False,
            "execution_latency_ms": 1,
            "ttl_secs": None,
        },
    )


def main() -> int:
    banner("Demo 3 — Agentic reuse: pure and read-only nodes reused, mutation never replayed")
    checks: list[tuple[str, bool]] = []
    balance = {"account": "acct-demo", "balance": 1000}

    with new_client() as client:
        step("record the authoritative balance, then plan the workflow (run 1)")
        record(client, balance)
        run1 = post(client, "/v1/execution/plan", plan_payload(balance))
        step(f"run 1 reused={run1.get('reused_node_ids')} pending={run1.get('pending_node_ids')}")
        checks.append(("mutation is never in the reuse set", "execute_payment" not in run1.get("reused_node_ids", [])))

        step("plan the same workflow again (run 2)")
        run2 = post(client, "/v1/execution/plan", plan_payload(balance))
        reused2 = run2.get("reused_node_ids", [])
        step(f"run 2 reused={reused2} pending={run2.get('pending_node_ids')}")
        checks.append(("read-only node reused on the second run", "read_balance" in reused2))
        checks.append(("mutation is still excluded on the second run", "execute_payment" not in reused2))

        step("the authoritative balance changes, then plan again (run 3)")
        record(client, {"account": "acct-demo", "balance": 2500})
        run3 = post(client, "/v1/execution/plan", plan_payload(balance))
        reused3 = run3.get("reused_node_ids", [])
        step(f"run 3 reused={reused3} pending={run3.get('pending_node_ids')}")
        checks.append(("stale read-only node is recomputed", "read_balance" not in reused3))
        checks.append(("mutation remains excluded", "execute_payment" not in reused3))

    for name, ok in checks:
        result(name, ok)
    return finish(checks)


if __name__ == "__main__":
    sys.exit(main())
