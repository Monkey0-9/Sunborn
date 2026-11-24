# Sunborn Benchmarks (`benchmarks/`)

Empirical evaluation suites, reproducibility datasets, latency measurements, and FinOps cost benchmarks for Sunborn.

## Benchmark Harnesses & Scripts

- **`semantic_cache_bench_x.py`**: Multi-dimensional benchmark evaluating Exact Hit, Semantic Hit, Linguistic Guardrail Abstention, and Origin Fallback latency.
- **`adversarial_evaluation_suite.py`**: Adversarial test suite evaluating semantic guardrails against negation flips, numeric magnitude shifts, currency substitution, and prompt injections.
- **`capacity_soak.py`**: Capacity soak and memory stability harness exercising burst concurrency, connection pools, and long-running memory footprints.
- **`competitive_evaluation.py`**: Comparative benchmark evaluating Sunborn dual caching against naive cosine thresholding and direct origin baselines across 25 standardized queries.
- **`graph_invalidation_deep_benchmark.py`**: Graph traversal benchmark comparing standard HashSet, AdjacencyList, Compressed Sparse Row (CSR), and 64-bit Roaring Treemap bitsets.
- **`graph_storage_and_policy_bench.py`**: Evaluates graph storage overhead (~1.8 bytes/edge) and FinOps volatility-discounted cache eviction policies.
- **`heldout_semantic_generalization.py`**: Generalization benchmark evaluating precision and recall across 320 stratified evaluation pairs.
- **`http_load_test.py`**: Real HTTP/1.1 keep-alive load generator measuring throughput, P50, and P99 latency percentiles across exact hits and misses.
- **`semantic_cache_bench.py` & `semantic_cache_bench_t.py`**: Stateful temporal mutation runners verifying causal invalidation and reproducibility.
- **`sunborn_bench.py`**: Developer-friendly CLI benchmark utility with configurable models, concurrency levels, and JSON export.
- **`telemetry_stress_eval.py`**: Multi-model FinOps stress evaluation measuring token avoidance and net savings across commercial LLM pricing tiers.

## Benchmark Artifacts (`benchmarks/results/`)

- **`gateway_performance_run.json`**: Measured single-machine sequential gateway throughput and L1 exact cache latency.
- **`l1_throughput_run.json`**: High-concurrency mixed get/insert microbenchmarks for the L1 in-memory tier.
- **`competitive_benchmark_run.json`**: Recorded results from the 25-query competitive evaluation.
- **`production_soak_results.json`**: 16-worker concurrent load burst results over 10,000 requests.
- **`telemetry_stress_eval_results.json`**: Empirical multi-model FinOps stress evaluation output.

## Running Benchmarks

```bash
# Run multi-dimensional semantic cache benchmark
python benchmarks/semantic_cache_bench_x.py

# Run comparative baseline evaluation
python benchmarks/competitive_evaluation.py

# Run CLI benchmark runner
python benchmarks/sunborn_bench.py --help
```
