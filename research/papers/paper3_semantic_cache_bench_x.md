# Paper 3: SemanticCacheBench-X: Evaluating Correctness, Freshness, and Security in AI Execution Reuse

**Target Venues**: NeurIPS Datasets and Benchmarks / EMNLP / ACM FAccT  
**Title**: *SemanticCacheBench-X: A Multi-Dimensional Benchmark for Safe Execution Reuse in AI Systems*

---

## Abstract
Existing semantic cache benchmarks evaluate models primarily on lexical paraphrases, neglecting the multidimensional failure modes of production AI applications: negation flipping, numerical changes, temporal staleness, authorization boundaries, tool mutations, and adversarial poisoning. We present **SemanticCacheBench-X**, a comprehensive benchmark suite spanning 10 distinct evaluation dimensions across 55,000 cases. The benchmark establishes strict held-out dev, calibration, and test splits to prevent data leakage into normalization heuristics, enforces fail-closed Git provenance tracking, and reports both Safe Reuse Precision and Reuse Coverage with exact Clopper-Pearson confidence bounds.

## Key Research Contributions
1. **10-Dimensional Taxonomy**: Paraphrase, Negation, Numeric, Temporal, Context, Authorization, Tool DAG, Adversarial, Multilingual, and Long-Context RAG.
2. **Leakage-Free Split Design**: Deterministic hash-based split assignment ensuring zero held-out test contamination during heuristic or conformal calibration.
3. **Dual Execution Harness**: A dry-run synthesizer for reproducible artifact generation and a live gateway RPC harness for true operational latency and behavioral scoring.
4. **Open-Source Artifact**: Release of the benchmark dataset, runner, and validation suites for the AI systems research community.
