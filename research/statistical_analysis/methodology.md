# Scientific Statistical Analysis & Evidence Methodology

To maintain impeccable scientific integrity in all Sunborn research publications, this document outlines the mandatory statistical standards, confidence interval formulations, and evaluation protocols.

---

## 1. Dual Metric Mandate: Precision & Coverage
Reporting cache hit rate or precision in isolation is scientifically invalid:
* A degenerate cache that accepts 0 reuses achieves 100% precision with 0% utility.
* A naive cache that accepts all queries achieves high coverage with catastrophic false-hit risk.

**Sunborn Standard**:
Every experiment must report:
$$\text{Safe Reuse Precision} = \frac{\text{True Safe Reuses}}{\text{Total Reuses Served}}$$
$$\text{Reuse Coverage} = \frac{\text{True Safe Reuses}}{\text{Total Safely Reusable Queries}}$$

---

## 2. Exact Confidence Intervals: Clopper-Pearson
When reporting zero or near-zero false-positive rates on finite evaluation datasets, normal-approximation intervals fail ($p=0 \implies \sigma=0$).

Sunborn mandates the exact **Clopper-Pearson (Beta distribution) confidence interval** for binomial proportions:
$$\left[ B\left(\frac{\alpha}{2}; x, n - x + 1\right), B\left(1 - \frac{\alpha}{2}; x + 1, n - x\right) \right]$$
For $x = 0$ observed false positives in $n$ trials:
$$\text{Upper Bound} = 1 - \left(\frac{\alpha}{2}\right)^{1/n}$$
At $99.9\%$ confidence ($\alpha = 0.001$) on $n = 320$:
$$\text{Upper Bound} = 1 - (0.0005)^{1/320} \approx 2.34\%$$

---

## 3. Strict Held-Out Split Discipline
1. **No Data Leakage**: The final test split must NEVER inform token normalizers, stopword lists, or embedding threshold tuning.
2. **Deterministic Partitioning**: Case ID hash modulo 100 determines split assignment (Dev 0-49, Calibration 50-74, Test 75-99).
3. **Artifact Provenance**: Datasets and results carry SHA-256 digests and Git commit hashes.

---

## 4. Real FinOps Economic Accounting ($V_{\text{reuse}}$)
Never present algorithmic token-count reductions as audited dollar savings. Always compute net economic benefit:
$$V_{\text{reuse}} = C_{\text{origin avoided}} - C_{\text{lookup}} - C_{\text{verification}} - C_{\text{storage}} - C_{\text{operations}} - C_{\text{risk}}$$
Where all costs are explicitly itemized using published provider rate cards.
