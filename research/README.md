# Sunborn Research Package & Artifact Evaluation Guide

> **Research Preprint**: *Sunborn: Deterministic Equivalence, Multi-Tier Semantic Caching, and Partial Execution Reuse for Agentic RAG Systems*.

---

## 1. Artifact Contents
* `paper/sunborn_ieee.tex`: Source LaTeX preprint manuscript and empirical evaluation.
* `reproduce.sh`: Automated end-to-end reproducibility script reproducing all empirical tables and latency metrics.
* `hypothesis_register.md`: Scientific hypothesis registry tracking controlled ablations against Baselines B0–B6.
* `../benchmarks/semantic_cache_bench.py`: SemanticCacheBench dataset runner.
* `../benchmarks/capacity_soak.py`: Continuous capacity and memory stability simulator.

---

## 2. Reproduction Instructions
To execute the one-command reproduction suite:
```bash
./research/reproduce.sh
```

Expected Outcome:
* 100% test pass across all Rust workspace crates.
* Throughput measurement: $> 10{,}000\text{ req/sec}$.
* Exact cache P99 latency: $< 2.0\text{ ms}$.
* 0% false hits on SemanticCacheBench under risk threshold $\le 0.25$.
* 0 cross-tenant leakages under adversarial prompt injections.
