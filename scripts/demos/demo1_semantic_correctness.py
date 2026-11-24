#!/usr/bin/env python3
"""Demo 1 — Semantic correctness.

A paraphrase is safely reused. A negation and a numeric change that a naive
vector cache would also "match" are refused, because the structural guardrails
compare numbers and polarity before similarity is trusted.
"""

from __future__ import annotations

import sys

from _client import ask, banner, finish, new_client, result, step


def main() -> int:
    banner("Demo 1 — Semantic correctness: reuse the paraphrase, refuse the inversion")
    checks: list[tuple[str, bool]] = []

    with new_client() as client:
        base = "What is the interest rate in 2025?"
        step(f"origin request: {base!r}")
        first = ask(client, base)
        step(f"origin cached={first.get('cached')}")

        paraphrase = "Can you tell me the interest rate in 2025?"
        step(f"paraphrase: {paraphrase!r}")
        p = ask(client, paraphrase)
        reused_paraphrase = bool(p.get("cached"))
        step(f"paraphrase cached={reused_paraphrase} certificate_present={p.get('certificate') is not None}")
        checks.append(("paraphrase was safely reused", reused_paraphrase))

        negation = "What is the interest rate in 2025, and is it not above 8%?"
        step(f"negation: {negation!r}")
        n = ask(client, negation)
        refused_negation = not bool(n.get("cached"))
        step(f"negation cached={n.get('cached')} decision={n.get('decision', {}).get('action')}")
        checks.append(("negation was refused", refused_negation))

        numeric = "What is the interest rate in 2026?"
        step(f"numeric change: {numeric!r}")
        m = ask(client, numeric)
        refused_numeric = not bool(m.get("cached"))
        step(f"numeric change cached={m.get('cached')} decision={m.get('decision', {}).get('action')}")
        checks.append(("changed number was refused", refused_numeric))

    for name, ok in checks:
        result(name, ok)
    return finish(checks)


if __name__ == "__main__":
    sys.exit(main())
