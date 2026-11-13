# 33 — ACM Assessment

Assessed against ACM/IEEE Software Engineering Code of Ethics and ACM computing-body knowledge areas; alignment only, not certification.

## 1. Ethics (ACM Code 1.3, 1.5, 1.6 — honesty, avoiding harm, confidentiality)

| Principle | Status | Note |
|---|---|---|
| 1.3 Be honest and trustworthy | **VIOLATED in public claims** | README performance/safety numbers are false or vacuous (F-007) |
| 1.5 Honor confidentiality | OK | No credentials in tracked source; secrets handled |
| 1.6 Be fair | OK | Apache-2.0 |
| 2.5 Give comprehensive evaluation | **WEAK** | Prior self-audits overclaim; this audit corrects |
| 2.9 Design for safety | PARTIAL | Safety thesis sound; evidence unmeasured |

The most serious ethics issue is **presenting unverified benchmarks and a self-attested safety guarantee as established**. The fix is disclosure + measurement, not marketing.

## 2. Computing-body coverage

| Area | Coverage |
|---|---|
| Algorithms & complexity | Good (Roaring, Merkle, Kahn, GDSF) |
| Distributed computing | Weak (claims exceed implementation) |
| Database theory | Good (schema, indexes, transactions needed) |
| Security | Mixed (crypto sound; auth/freshness weak) |
| AI/IR | Weak (no real embeddings/ANN by default) |
| HCI | Weak (fake UI, a11y violations) |
| Software engineering | Mixed (good modularity; poor claim hygiene) |

## 3. Recommendation

Adopt a public **claims register** where every quantified claim cites a reproducible artifact, and remove all unmeasured numbers. This is the single highest-leverage ethical fix.
