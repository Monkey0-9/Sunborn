# 13 — Mathematical Model Validation

---

## 1. Cosine similarity

Definition: `cos(a,b) = (a·b) / (||a|| ||b||)`.
Implementation `embedding.rs:6-24` matches. Guarded against zero norms. ✔

## 2. Conformal risk control

Claim: distribution-free upper bound on false-reuse risk with α ≤ 0.001.

What the code does on the decision path:
- seeded thresholds per stratum (`conformal.rs:78-82`);
- threshold updated to the α-quantile of observed similarities after ≥20 observations, clamped `[0.70,0.99]` (`:103-123`).

What a valid split-conformal guarantee requires:
- exchangeability (explicitly violated by streaming, the code names a "non-exchangeable" variant);
- a held-out calibration set with labels;
- a coverage statement bound to that set;
- monotone risk control.

**Finding:** the production threshold is a heuristic similarity cutoff. The rigorous non-exchangeable calibrator (`conformal.rs:205-278`) and Wilson interval (`:307-321`) exist but are documented as unwired/shadow. Therefore the **α ≤ 0.001 guarantee is not established** on the runtime path.

## 3. Wilson score interval

Implementation `conformal.rs:307-321` is the standard Wilson formula. Let's verify:
`(p + z²/2n ± z√(p(1−p)/n + z²/4n²)) / (1 + z²/n)`.
The implementation matches the usual closed form. ✔ (math correct; usage unwired).

## 4. FinOps net savings

`net = avoided_tokens * price − infra_overhead`. Deterministic; no statistical claim. Frontend's `$280.0` infra constant and `142.5` fallback are **presentational**, not derived (`frontend/src/app/finops/page.tsx:239`). Not a math error in core; a frontend fabrication.

## 5. Latency/throughput claims

- `571,428 req/s` = 10,000 iterations / 17.5ms (single-thread in-process map). Not a gateway measurement; cited file `throughput_test.rs` does not exist.
- `3,257 req/s` "socket stack" — the underlying test uses `tower::ServiceExt::oneshot` (no socket).
- `142,857 ops/s` = 1/7µs; the recorded p50 is 4.0µs (= 250,000 ops/s) → the 142,857 figure is not derivable.
- Memory `2.15 MB` = `edges*1.8` evaluated in Python (`graph_invalidation_deep_benchmark.py:298`), where the "roaring" is `dict[int,set]` (~60–200 B/edge realistically).

**Finding:** four incompatible performance tables exist (README, `docs/master/10`, `docs/architecture/performance_benchmark_report.md`, `research/paper/sunborn_ieee.tex`). None trace to a single reproducible artifact.

## 6. "0.0% false hits"

`precision = tp/(tp+fp) if (tp+fp)>0 else 100.0` (`benchmarks/adversarial_evaluation_suite.py:820-824`).
Artifact `research/results/adversarial_run.json:75-92`: `reused 0, tp 0, fp 0, precision 100.0, recall 0.0`.
Mathematically, precision is undefined (0/0) when nothing is reused; defaulting to 100% is a **mathematical misstatement**. The system is "safe" only because it abstains entirely.

## 7. Statistical validity of the held-out suite

- Ground-truth labels are author-assigned on an author-written query set.
- Default embeddings are hash vectors; with a real embedding model the numbers would change.
- 70 scenarios exist in the held-out script; it prints "75" (`heldout_semantic_generalization.py:399`) — a hardcoded string.

## Verdict

Core arithmetic (cosine, Wilson, FinOps) is correct. The **safety and performance mathematics presented publicly is not valid**: the conformal guarantee is not established online, precision is reported as 100% from an empty-reuse denominator, and memory/throughput constants are fabricated. Mathematical correctness score: **4/10**.
