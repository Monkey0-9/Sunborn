# Sunborn Statistical & Mathematical Review

**Document ID:** `SUNBORN-VERIFY-MATH-2026.1`  
**Classification:** Theoretical Machine Learning & Conformal Prediction Audit  
**Author:** Independent Statistical & Distributed Systems Review Board

---

## 1. Conformal Prediction Formulation & Theoretical Framework

Sunborn implements split-conformal risk control to govern semantic cache admission:
- Let $X_i \in \mathcal{X}$ be an incoming query, and let $\tilde{X}_i \in \mathcal{X}$ be a candidate cached query.
- Non-conformity score function:
  $$S_i = (1 - \text{sim}(X_i, \tilde{X}_i)) + \beta \cdot \text{Risk}(X_i)$$
  where $\beta = 0.35$ weights business risk.
- Calibration threshold: Given $n$ exchangeable calibration non-conformity scores $\{S_1, \dots, S_n\}$, the empirical quantile at confidence level $1 - \alpha$ is:
  $$\hat{\tau} = \text{Quantile}\left(\frac{\lceil (n + 1)(1 - \alpha) \rceil}{n}, \{S_1, \dots, S_n\}\right)$$

---

## 2. Evaluation of Theoretical Assumptions

### Assumption 1: Exchangeability (i.i.d. Traffic)
- **Mathematical Requirement:** Standard split-conformal inference assumes observations $(X_1, Y_1), \dots, (X_{n+1}, Y_{n+1})$ are exchangeable.
- **Production Reality:** Production LLM prompt distributions exhibit severe non-exchangeability:
  1. Temporal burstiness (e.g. breaking news, quarterly earnings releases).
  2. Concept drift (evolving product documentation, prompt engineering changes).
  3. Causal dependencies (follow-up conversational turns).
- **Audit Finding:** Standard exchangeability **DOES NOT HOLD** in open-domain LLM production traffic.
- **Sunborn Mitigations in Code:**
  1. **Streaming Welford OOD Detector (`crates/sunborn-semantic/src/conformal.rs`):** Tracks online running mean $\mu_t$ and variance $\sigma^2_t$. If $Z_t = \frac{\mu_t - \text{sim}}{\sigma_t} > 2.8$, triggers fail-safe abstention.
  2. **Non-Exchangeable Calibrator:** Uses exponential forgetting factor $\rho = 0.995$ to weight recent observations more heavily than historical data.
- **Theoretical Limitation:** Exponential weighting heuristic maintains practical coverage, but invalidates the finite-sample distribution-free theoretical coverage guarantee $P(\text{Loss} > \epsilon) \le \alpha$. Claims of "mathematically proven zero risk" are theoretically invalid under non-exchangeable drift.

### Assumption 2: Calibration Sample Size ($n \ge 20$)
- In `TaskStratifiedConformalCalibrator::record_observation`, threshold recomputation requires $n \ge 20$ samples per stratum.
- For $\alpha = 0.001$ (SecuritySensitive), the formula $\frac{\lceil (n + 1)(1 - \alpha) \rceil}{n}$ requires at least $n \ge 1,000$ calibration samples to distinguish the $0.001$ quantile from the maximum observed score!
- With $n = 20$, the minimum achievable empirical alpha is $\frac{1}{21} \approx 0.0476$ (4.76%).
- **Audit Finding:** The sample size threshold ($n=20$) is insufficient to guarantee 99.9% coverage ($\alpha = 0.001$). A minimum sample size of $n \ge 1,000$ must be enforced for high-stakes strata before closing the fallback threshold.

### Assumption 3: Task Stratification
- Stratifying queries into `LowRiskGeneral`, `FinancialAccounting`, `HealthcareMedical`, `SystemAdministrative`, and `SecuritySensitive` is theoretically sound and prevents high-risk false hits from being drowned out by high volumes of generic queries.

---

## 3. Empirical vs. Theoretical Zero-Error Distinction

| Statement | Status | Mathematical Assessment |
| :--- | :--- | :--- |
| "0.0% false-hit rate observed on benchmark suite" | **TRUE** | Verified empirically on the 124-query static dataset ($FP = 0$). |
| "Proven 0.0% false-hit rate across all LLM queries" | **FALSE** | Impossible for vector embedding similarity; adversarial counterexamples exist. |
| "Distribution-free risk bound holds under arbitrary drift"| **FALSE** | Distribution-free bounds require exchangeability, which fails under concept drift. |

---

## 4. Required Corrective Actions

1. Enforce $N_{\min} \ge \lceil 1 / \alpha \rceil$ before activating calibrated quantiles (e.g. $N \ge 1,000$ for $\alpha = 0.001$).
2. Replace static fallback thresholds with conservative fail-safe defaults ($0.95$ for security strata) during cold-start warmup.
3. Document that Sunborn provides *empirical risk control under adaptive drift*, rather than unconditional distribution-free guarantees.
