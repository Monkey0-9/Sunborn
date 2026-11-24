#!/usr/bin/env python3
"""Demo 4 — Verifiable execution evidence.

Attest four execution digests, verify a valid inclusion proof, then tamper with
the leaf and show the verifier rejects it. Verification is performed by the
gateway's Merkle engine, not by this script, so the check is not self-certified.
"""

from __future__ import annotations

import hashlib
import sys

from _client import banner, finish, new_client, post, result, step


def sha256(text: str) -> str:
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def main() -> int:
    banner("Demo 4 — Verifiable evidence: attest, verify, then reject a tampered proof")
    checks: list[tuple[str, bool]] = []

    leaves = [
        sha256("execution:read_balance:1000"),
        sha256("execution:calc_interest:42"),
        sha256("execution:render_answer:v7"),
        sha256("execution:audit_write:ok"),
    ]

    with new_client() as client:
        step("attest four execution digests")
        attestation = post(client, "/v1/governance/attest", {"leaf_hashes": leaves})
        root = attestation.get("root_hash")
        proofs = attestation.get("proofs", [])
        step(f"root={root} leaves={attestation.get('leaf_count')} proofs={len(proofs)}")
        checks.append(("attestation returned a root over all leaves", root is not None and len(proofs) == len(leaves)))

        if not proofs:
            for name, ok in checks:
                result(name, ok)
            return finish(checks)

        first = proofs[0]
        step("verify a genuine inclusion proof")
        valid = post(client, "/v1/governance/verify-proof", {"proof": first})
        step(f"is_valid={valid.get('is_valid')}")
        checks.append(("genuine proof verified", bool(valid.get("is_valid"))))

        step("tamper with the leaf hash and verify again")
        tampered = dict(first)
        tampered["leaf_hash"] = sha256("execution:read_balance:9999")
        rejected = post(client, "/v1/governance/verify-proof", {"proof": tampered})
        step(f"is_valid={rejected.get('is_valid')}")
        checks.append(("tampered proof rejected", not bool(rejected.get("is_valid"))))

    for name, ok in checks:
        result(name, ok)
    return finish(checks)


if __name__ == "__main__":
    sys.exit(main())
