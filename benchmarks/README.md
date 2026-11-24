# Sunborn Benchmarks (`benchmarks/`)

Empirical evaluation suites, reproducibility datasets, and latency/cost benchmarks for Sunborn.

## Contents

- **`semantic_cache_bench_x.py`**: Multi-dimensional benchmark harness evaluating Exact Hit, Semantic Hit, Linguistic Guardrail Abstention, and Origin Fallback latency.
- **Datasets**: Paired evaluation queries across technical, financial, and conversational domains with grounded truth labels.
- **Comparative Baselines**: Standardized benchmarks comparing Sunborn's non-exchangeable conformal risk control against naive vector similarity thresholding.

## Running the Benchmarks

```bash
python benchmarks/semantic_cache_bench_x.py
```
