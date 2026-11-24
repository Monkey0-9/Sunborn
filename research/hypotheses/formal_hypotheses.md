# Formal Research Hypotheses & Experimental Methodology

The Sunborn research agenda is organized around 7 formal, falsifiable hypotheses.

---

## Hypothesis Register

### H1: Proof-Carrying Semantic Reuse (PCSR)
> **Hypothesis**: Attaching an independently verifiable, domain-separated cryptographic certificate $\pi$ to reusable AI computation outputs enables client-side tamper and replay detection with sub-100 microsecond verification overhead, eliminating 100% of unauthorized cross-tenant reuse events.
* **Baseline**: Unauthenticated direct cache retrieval (Redis / GPTCache).
* **Metric**: Evidence Verification Rate, Cross-Tenant Leakage Rate, Verification Latency ($\mu$s).
* **Falsification Condition**: Verification latency exceeds 500 $\mu$s or an adversary successfully forges cross-tenant reuse.

### H2: Causal Freshness Engine
> **Hypothesis**: Evaluating a 5-dimensional causal freshness vector $F(q,t) = f(D_t, P_t, A_t, M_t, E_t)$ reduces the stale-reuse exposure window to zero under dependency mutation events while maintaining $\ge 90\%$ retention of valid unrelated cache entries compared with global-epoch flushing.
* **Baseline**: Fixed TTL caching (300s, 3600s) and global-epoch cache purging.
* **Metric**: Stale Reuse Rate, Unnecessary Invalidation Rate, Invalidation Completion Time (ms).

### H3: Effect-Aware Partial Agent Execution Reuse
> **Hypothesis**: Classifying agent tool executions into typed effect classes (Pure, ReadOnlyStateful, ExternalMutableRead, IdempotentMutation, NonIdempotentMutation) and pruning mutating subgraphs eliminates 100% of unintended side-effect replays while saving $\ge 40\%$ of execution steps in multi-turn workflows.
* **Baseline**: Whole-response caching (0% intermediate savings) and naive DAG memoization (unsafe side-effect replay).
* **Metric**: Computation Saved Ratio, Unsafe Mutation Replay Rate (strictly 0).

### H4: Joint Risk-Budgeted Calibration
> **Hypothesis**: Jointly calibrating semantic similarity, structural tokens, and context lineage via non-exchangeable conformal risk control guarantees an empirical false-hit rate $\le \alpha$ (e.g., $\alpha = 0.005$) on non-stationary production distributions.
* **Baseline**: Fixed-threshold similarity matching ($\tau = 0.90$).
* **Metric**: Empirical False-Positive Rate, Coverage, Operational Retention Rate.

### H5: Adversarial Poisoning & Injection Defense
> **Hypothesis**: Combining embedding length-anomaly detection, structural inversion pre-filtering, and tenant-scoped quarantine pools detects $\ge 95\%$ of targeted cache poisoning and injection queries with $< 1\%$ false rejection of benign paraphrases.
* **Baseline**: Standard cosine similarity search without anomaly filtering.
* **Metric**: Attack Detection Precision/Recall, Benign False Rejection Rate.

### H6: Safety-Constrained Online Learning
> **Hypothesis**: Evaluating candidate cache admission and threshold policies via offline shadow replay against strict multi-criteria safety gates guarantees zero regression in safe reuse precision while optimizing hit rates under distribution drift.
* **Baseline**: Static manual cache threshold tuning.
* **Metric**: Change in Net Economic Savings, Zero Invariant Violation in Canary Deployment.

### H7: Net Economic Optimization ($V_{\text{reuse}}$)
> **Hypothesis**: Governing admission by net economic value $V_{\text{reuse}} = C_{\text{origin avoided}} - C_{\text{lookup}} - C_{\text{verification}} - C_{\text{storage}} - C_{\text{operations}} - C_{\text{risk}}$ achieves positive FinOps ROI even in low-hit-rate environments ($< 10\%$) where naive semantic caching operates at an economic loss.
* **Baseline**: Hit-rate-only caching metrics.
* **Metric**: Net Economic Savings (USD), Break-Even Hit Rate.
