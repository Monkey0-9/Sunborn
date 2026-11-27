#!/usr/bin/env python3
"""
Generate a deterministic, blind out-of-sample adversarial benchmark (N = 2,400 pairs:
400 equivalent paraphrases + 2,000 hard negatives) across 5 trap families:
  1. Negation / polarity flips (400 negatives)
  2. Quantifier scope shifts (400 negatives)
  3. Numeric drift (400 negatives)
  4. Entity swaps (400 negatives)
  5. Indic / Hinglish code-mixed traps (400 negatives)

This generator is independent of guardrail rules and must remain frozen before evaluation.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

DOMAINS = [
    ("treasury", "account", "transfer"),
    ("healthcare", "patient", "prescription"),
    ("security", "firewall", "token"),
    ("devops", "cluster", "deployment"),
    ("legal", "contract", "clause"),
    ("billing", "invoice", "refund"),
    ("identity", "session", "permission"),
    ("storage", "bucket", "snapshot"),
]

ENTITIES_A = ["AlphaCorp", "MeridianBank", "KaveriHealth", "IndusCloud", "VanguardOps", "NarmadaPay", "SuryaGrid", "DeccanFin"]
ENTITIES_B = ["BetaLogistics", "ApexTrust", "GangaClinic", "SaharaCompute", "ZenithSec", "YamunaPay", "ChandraPower", "KonkanInsure"]

PARAPHRASE_TEMPLATES = [
    ("Show the {noun} status for {ent_a} in {domain}.", "Display the {noun} status for {ent_a} in {domain}."),
    ("Fetch the {verb} summary for {ent_a}.", "Retrieve the {verb} summary for {ent_a}."),
    ("List all {noun} entries for {ent_a}.", "Get all {noun} entries for {ent_a}."),
    ("Check {domain} {noun} quota for {ent_a}.", "Inspect {domain} {noun} quota for {ent_a}."),
]


def build_dataset() -> dict:
    pairs = []
    idx = 0

    # 400 equivalent paraphrases (50 * 8 domains)
    for i in range(400):
        domain, noun, verb = DOMAINS[i % len(DOMAINS)]
        ent_a = ENTITIES_A[(i // len(DOMAINS)) % len(ENTITIES_A)]
        t1, t2 = PARAPHRASE_TEMPLATES[i % len(PARAPHRASE_TEMPLATES)]
        pairs.append({
            "id": f"BLIND-EQ-{i:04d}",
            "category": "equivalent_paraphrase",
            "expected_equivalent": True,
            "query_a": f"{t1.format(domain=domain, noun=noun, verb=verb, ent_a=ent_a)} [ref #{i+100}]",
            "query_b": f"{t2.format(domain=domain, noun=noun, verb=verb, ent_a=ent_a)} [ref #{i+100}]",
        })
        idx += 1

    # 400 negation / polarity flips
    neg_pairs = [
        ("Enable", "Disable"),
        ("Allow", "Block"),
        ("Approve", "Reject"),
        ("Grant", "Revoke"),
    ]
    for i in range(400):
        domain, noun, _ = DOMAINS[i % len(DOMAINS)]
        ent_a = ENTITIES_A[(i // len(DOMAINS)) % len(ENTITIES_A)]
        pos_v, neg_v = neg_pairs[i % len(neg_pairs)]
        pairs.append({
            "id": f"BLIND-NEG-{i:04d}",
            "category": "negation_polarity",
            "expected_equivalent": False,
            "query_a": f"{pos_v} {domain} {noun} access for {ent_a} on port {8000 + i}.",
            "query_b": f"{neg_v} {domain} {noun} access for {ent_a} on port {8000 + i}.",
        })

    # 400 quantifier scope shifts
    quant_pairs = [
        ("all", "none"),
        ("every", "only"),
        ("always", "never"),
        ("at least", "at most"),
    ]
    for i in range(400):
        domain, noun, _ = DOMAINS[i % len(DOMAINS)]
        ent_a = ENTITIES_A[(i // len(DOMAINS)) % len(ENTITIES_A)]
        q1, q2 = quant_pairs[i % len(quant_pairs)]
        pairs.append({
            "id": f"BLIND-QUANT-{i:04d}",
            "category": "quantifier_scope",
            "expected_equivalent": False,
            "query_a": f"Audit {q1} {domain} {noun} records for {ent_a} batch {i + 1}.",
            "query_b": f"Audit {q2} {domain} {noun} records for {ent_a} batch {i + 1}.",
        })

    # 400 numeric drift traps
    for i in range(400):
        domain, noun, _ = DOMAINS[i % len(DOMAINS)]
        ent_a = ENTITIES_A[(i // len(DOMAINS)) % len(ENTITIES_A)]
        val_a = 100 + i * 5
        val_b = val_a + 25
        pairs.append({
            "id": f"BLIND-NUM-{i:04d}",
            "category": "numeric_drift",
            "expected_equivalent": False,
            "query_a": f"Set {domain} {noun} limit for {ent_a} to {val_a} units.",
            "query_b": f"Set {domain} {noun} limit for {ent_a} to {val_b} units.",
        })

    # 400 entity swap traps
    for i in range(400):
        domain, noun, _ = DOMAINS[i % len(DOMAINS)]
        ent_a = ENTITIES_A[i % len(ENTITIES_A)]
        ent_b = ENTITIES_B[i % len(ENTITIES_B)]
        pairs.append({
            "id": f"BLIND-ENT-{i:04d}",
            "category": "entity_swap",
            "expected_equivalent": False,
            "query_a": f"Verify {domain} {noun} compliance for {ent_a} ticket {500 + i}.",
            "query_b": f"Verify {domain} {noun} compliance for {ent_b} ticket {500 + i}.",
        })

    # 400 Indic / Hinglish code-mixed traps
    indic_traps = [
        ("Hinglish negation", "Kripya {ent_a} ka {noun} enable karo ID {id_num}", "Kripya {ent_a} ka {noun} band karo ID {id_num}"),
        ("Hinglish nahi", "{ent_a} ke {noun} ko approve karein {id_num}", "{ent_a} ke {noun} ko approve nahi karein {id_num}"),
        ("Hindi Devanagari negation", "{ent_a} का {noun} सक्रिय करें {id_num}", "{ent_a} का {noun} बंद करें {id_num}"),
        ("Devanagari numeral shift", "{ent_a} खाता सीमा ₹५०० सेट करें", "{ent_a} खाता सीमा ₹५००० सेट करें"),
    ]
    for i in range(400):
        _, noun, _ = DOMAINS[i % len(DOMAINS)]
        ent_a = ENTITIES_A[i % len(ENTITIES_A)]
        _, ta, tb = indic_traps[i % len(indic_traps)]
        pairs.append({
            "id": f"BLIND-INDIC-{i:04d}",
            "category": "indic_hinglish_trap",
            "expected_equivalent": False,
            "query_a": ta.format(ent_a=ent_a, noun=noun, id_num=1000 + i),
            "query_b": tb.format(ent_a=ent_a, noun=noun, id_num=1000 + i),
        })

    canonical = json.dumps(pairs, sort_keys=True).encode("utf-8")
    digest = hashlib.sha256(canonical).hexdigest()
    return {
        "dataset": "blind_out_of_sample_2400",
        "total_pairs": len(pairs),
        "equivalent_pairs": 400,
        "hard_negative_pairs": 2000,
        "sha256": digest,
        "pairs": pairs,
    }


def main() -> None:
    out = Path("research/calibration/blind_out_of_sample_2400.json")
    out.parent.mkdir(parents=True, exist_ok=True)
    data = build_dataset()
    out.write_text(json.dumps(data, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"Wrote {data['total_pairs']} pairs (sha256={data['sha256']}) to {out}")


if __name__ == "__main__":
    main()
