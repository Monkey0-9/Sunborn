# Sunborn Semantic Safety, NLI Verification & Conformal Freshness Review

**Document Version:** 2.0.0  
**Effective Commit:** `7f1f161`  
**Classification:** AI Systems Research & Correctness Specification

---

## 1. The Semantic Inversion Fallacy

Standard vector semantic caching relies on cosine similarity in dense embedding spaces. While effective for semantic topic matching, dense vector retrieval suffers from the **Semantic Inversion Fallacy**:

| Prompt A | Prompt B | Cosine Similarity | Human Ground Truth | Naive Cache Decision | Sunborn Safe Decision |
|---|---|---|---|---|---|
| "Transfer $5,000 from 102 to 889" | "Transfer $5,000 from 889 to 102" | 0.982 | **Incompatible (Contradiction)** | Reused (Catastrophic Error) | **REJECTED (Gap 2 Transposition)** |
| "Client loan is approved" | "Client loan is not approved" | 0.971 | **Incompatible (Negation)** | Reused (False Hit) | **REJECTED (Polarity Bit)** |
| "Show revenue for Q1 2024" | "Show revenue for Q2 2024" | 0.965 | **Incompatible (Temporal Shift)** | Reused (Stale/Wrong Data) | **REJECTED (Temporal Token)** |
| "Deploy to production environment" | "Deploy to staging environment" | 0.954 | **Incompatible (Entity Scope)** | Reused (Security Breach) | **REJECTED (Entity Drift)** |

In standard caching, these pairs achieve high cosine similarity (>0.95) because 90% of their tokens and syntactic structure are identical. In enterprise AI, reusing Prompt A's response for Prompt B produces catastrophic financial, legal, or operational errors.

---

## 2. Tier C3: Deterministic Structural Guardrails

To eliminate false hits before vector reuse, Sunborn executes deterministic structural fingerprint extraction:

```rust
pub struct StructuralFingerprint {
    pub sorted_numeric_literals: Vec<String>,
    pub negation_polarity_bit: bool,
    pub negation_antonym_tokens: Vec<String>,
    pub temporal_tokens: Vec<String>,
    pub entity_tokens: Vec<String>,
    pub content_tokens: Vec<String>,
    pub quantifier_scope_tokens: Vec<String>,
    pub role_bound_numeric_tokens: Vec<String>,
    pub ordered_numeric_sequence: Vec<String>,
    pub token_count: usize,
    pub fingerprint_hash: String,
}
```

### Invariant Checks
1. **Negation & Antonym Invariance**: Both queries must agree on negation polarity bit and antonym vocabularies.
2. **Gap 2 Transposition Defense**: Numeric literals are bound to preceding role markers (e.g. `from:102`, `to:889`). Transposing roles invalidates the match even if sorted numbers are identical.
3. **Temporal Invariance**: Calendar keywords, quarters (`Q1`-`Q4`), and fiscal years must match strictly.
4. **Quantifier Scope**: Universal and existential quantifiers ("all", "none", "every", "some") must match.

---

## 3. Tier C4: Cross-Encoder NLI & Split-Conformal Risk Control

### Joint Cross-Encoder NLI Verification
Unlike bi-encoders (which encode queries into separate vector points), Sunborn uses a joint Natural Language Inference (NLI) cascade via the `CrossEncoderBackend` trait:
- Premise: Cached Prompt $Q_{\text{cached}}$
- Hypothesis: Incoming Prompt $Q_{\text{req}}$
- Output: Entailment probability distribution:
  $$p_{\text{entailment}} + p_{\text{neutral}} + p_{\text{contradiction}} = 1.0$$
- **Acceptance Gate**: $p_{\text{entailment}} \ge 0.50$ and $p_{\text{contradiction}} \le 0.20$. Contradictions immediately abort reuse.

### Non-Parametric Conformal Risk Control ($\alpha \le 0.001$)
Sunborn computes calibrated nonconformity thresholds on held-out calibration sets:
$$\hat{\tau} = \text{Quantile}\left(\frac{\lceil(n+1)(1-\alpha)\rceil}{n}; \{s_i\}_{i=1}^n\right)$$
- If query similarity is below the task-stratified threshold $\hat{\tau}$, Sunborn safely abstains and routes to origin.
- The streaming Out-Of-Distribution (OOD) detector tracks rolling mean and variance using Welford's algorithm, triggering automatic recalibration under distribution drift.
