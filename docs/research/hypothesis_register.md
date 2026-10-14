# Sunborn Scientific Hypothesis Register (Phase 16)

> **Preregistered Research Hypotheses, Falsification Criteria & Evaluation Methodology**  
> *Target Standard: IEEE/ACM Peer-Reviewed Venue (IEEE ICDE / ACM SIGMOD / USENIX ATC)*

---

## Hypothesis 1 ($H_1$): Semantic Equivalence & Calibrated Risk Bounding

### Research Question
Does calibrating vector distance with domain risk assessment and temperature penalization significantly reduce semantic false-positives without degrading cache hit volume compared to static cosine thresholding?

### Formal Statement
* **Null Hypothesis ($H_{1,0}$)**:
  $$\text{FPR}_{\text{Sunborn}} \ge \text{FPR}_{\text{FixedCosine}} \quad (\alpha = 0.01)$$
* **Alternative Hypothesis ($H_{1,a}$)**:
  $$\text{FPR}_{\text{Sunborn}} \le 0.65 \times \text{FPR}_{\text{FixedCosine}} \quad \text{and} \quad \text{HR}_{\text{Sunborn}} \ge 0.95 \times \text{HR}_{\text{FixedCosine}}$$

### Falsification Protocol
- **Dataset**: `SemanticCacheBench-v1` (10,000 paired queries comprising paraphrase equivalents, antonymic inversions, parameter mutations, and injection attempts).
- **Statistical Method**: Paired McNemar's test for categorical error differences and Wilcoxon signed-rank test on token-level answer preservation.
- **Stopping Rule**: 10,000 evaluation iterations.
- **Falsification Condition**: Reject if $p \ge 0.01$ or if false-positive rate reduction is less than $35\%$.

---

## Hypothesis 2 ($H_2$): Event-Driven Knowledge Freshness Invariant

### Research Question
Does an outbox-driven dependency graph eliminate stale semantic cache responses under continuous knowledge source mutations?

### Formal Statement
* **Null Hypothesis ($H_{2,0}$)**:
  $$\text{StalenessViolationRate}_{\text{Outbox}} = \text{StalenessViolationRate}_{\text{TTL-Only}} \quad (\alpha = 0.05)$$
* **Alternative Hypothesis ($H_{2,a}$)**:
  $$\text{StalenessViolationRate}_{\text{Outbox}} = 0.00\% \quad \text{for all queries post-invalidation window } (\Delta t > 500\text{ ms})$$

### Falsification Protocol
- **Workload**: 5,000 document mutations interspersed with 50,000 concurrent reader queries against mutated knowledge sources.
- **Metric**: Proportion of returned responses containing outdated document version content.
- **Falsification Condition**: Reject if any query returning cached content after $500\text{ ms}$ post-event contains stale knowledge.

---

## Hypothesis 3 ($H_3$): Net Economic Utility & Break-Even Curve

### Research Question
Under what workload distributions does multi-tier execution reuse achieve positive net financial return over raw origin API inference?

### Formal Statement
* **Null Hypothesis ($H_{3,0}$)**:
  $$\text{Cost}_{\text{Total}}(\text{Sunborn}) \ge \text{Cost}_{\text{Origin}}(\text{Raw}) \quad \text{for cache hit rates } \le 25\%$$
* **Alternative Hypothesis ($H_{3,a}$)**:
  $$\text{Cost}_{\text{Total}}(\text{Sunborn}) < \text{Cost}_{\text{Origin}}(\text{Raw}) \quad \text{for cache hit rates } \ge 14\% \quad (\text{GPT-4o class})$$

### Falsification Protocol
- **Workload**: Trace-driven replaying of Enterprise RAG query logs (100,000 queries).
- **Cost Accounting**:
  - LLM token charges: $C_{\text{input}} = \$2.50/\text{M}$, $C_{\text{output}} = \$10.00/\text{M}$.
  - Memory storage cost: $\$0.01/\text{GB-hour}$.
  - Vector compute cost: $\$0.001/1,000$ queries.
- **Falsification Condition**: Reject if the empirical break-even hit rate exceeds $14\%$.
