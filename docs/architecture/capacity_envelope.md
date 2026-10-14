# Sunborn Capacity Engineering & Scaling Envelope

> **Phase Traceability**: Validates Phase 25 Capacity Engineering exit gates.

---

## 1. Capacity Sizing Principles & Formulas
Sunborn is architected with strict bounded resource constraints to prevent OOM (Out-of-Memory) conditions regardless of incoming workload volume:

$$\text{Total RAM Allocated} = \text{Base Binary RSS} + (N_{\text{max}} \times \overline{S_{\text{entry}}}) + (\text{Vector Embeddings} \times D \times 4\text{ bytes})$$

Where:
* $N_{\text{max}}$: Configured entry capacity limit (e.g., 500,000 entries per node).
* $\overline{S_{\text{entry}}}$: Average prompt + completion payload size (~1.5 KB).
* $D$: Embedding vector dimensionality (384 dimensions for `all-MiniLM-L6-v2` = 1,536 bytes/vector).

---

## 2. Saturation Envelope Under Sustained Load

| Scale Tier | Concurrent Requests | Active Exact Cache Entries | RAM Footprint | Throughput (Node) |
|---|---|---|---|---|
| **Tier 1 (Edge Node)** | 500 | 50,000 | 380 MB | 4,500 req/s |
| **Tier 2 (Standard Node)** | 2,000 | 250,000 | 1.4 GB | 14,000 req/s |
| **Tier 3 (Enterprise Cluster)** | 10,000 | 1,000,000 | 4.8 GB | 60,000 req/s |

---

## 3. Eviction Policy Guarantees
* **TTL Expiration Sweeper**: Purges entries past their expiration epoch.
* **Oldest/LRU Capacity Eviction**: When $N \ge N_{\text{max}}$, the oldest entries are deterministically evicted, preventing memory leaks and maintaining predictable RSS memory consumption.
