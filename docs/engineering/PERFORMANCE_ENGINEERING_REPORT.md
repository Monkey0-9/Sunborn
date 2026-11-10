# Sunborn: Production Performance Engineering & Benchmark Analysis

**Target Standard:** Systems Performance Engineering & Micro-Benchmarking  
**Document Version:** 1.0.0  
**Baseline Commit:** `dd25675c5eae200b2518557f966feff74d1556fe`  
**Primary Test Sources:**  
- [`crates/sunborn-gateway/tests/throughput_test.rs`](file:///C:/Sunborn/crates/sunborn-gateway/tests/throughput_test.rs)  
- [`benchmarks/graph_invalidation_deep_benchmark.py`](file:///C:/Sunborn/benchmarks/graph_invalidation_deep_benchmark.py)  
- [`research/results/graph_deep_audit_results.json`](file:///C:/Sunborn/research/results/graph_deep_audit_results.json)  

---

## 1. End-to-End Gateway Throughput & Latency

Performance characteristics were measured across two distinct evaluation layers:
1. **Layer 1: In-Process Engine (Zero-Network L1 Micro-Benchmark):**  
   Evaluates raw CPU execution, Moka cache lookup, JCS canonicalization, and in-memory policy checking without socket overhead.
2. **Layer 2: Full Axum Gateway Stack (Complete HTTP Socket Pipeline):**  
   Evaluates full TCP connection handling, Axum middleware (Auth, Rate Limiting, Request ID), JSON deserialization, and HTTP response serialization.

### Performance Profile Table

| Evaluation Tier | Requests Evaluated | Throughput (QPS) | Latency P50 | Latency P95 | Latency P99 | Memory Envelope |
|:---|:---:|:---:|:---:|:---:|:---:|:---:|
| **In-Process Engine (L1 Hot Path)** | 100,000 | **$571,719.3\text{ req/s}$** | **$0.001\text{ ms}$** ($1\text{ µs}$) | **$0.001\text{ ms}$** ($1\text{ µs}$) | **$0.009\text{ ms}$** ($9\text{ µs}$) | $< 64\text{ MB}$ |
| **Full Axum Gateway Stack (HTTP)** | 5,000 | **$3,257.6\text{ req/s}$** | **$0.294\text{ ms}$** ($294\text{ µs}$) | **$0.372\text{ ms}$** ($372\text{ µs}$) | **$0.475\text{ ms}$** ($475\text{ µs}$) | $< 128\text{ MB}$ |

---

## 2. Graph Invalidation Deep Benchmark (12 Operations Across 4 Scales)

To eliminate ambiguity regarding graph invalidation latencies, all 12 discrete operations of the invalidation pipeline were isolated and benchmarked across 4 scales: 10,000 edges, 100,000 edges, 352,578 edges (production baseline), and 1,000,000 edges.

### Empirical Latency Breakdown at 352,578 Edges

| Operation ID | Operation Description | P50 Latency | P95 Latency | P99 Latency | Max Latency | Primary Computational Bottleneck |
|:---|:---|:---:|:---:|:---:|:---:|:---|
| **OP-01** | Node Lookup (Single Pointer) | **$0.12\text{ µs}$** | $0.15\text{ µs}$ | $0.38\text{ µs}$ | $1.20\text{ µs}$ | Single hash map index dereference (`dict.get`). |
| **OP-02** | Reverse Dependency Lookup | **$0.14\text{ µs}$** | $0.19\text{ µs}$ | $0.45\text{ µs}$ | $1.85\text{ µs}$ | Reverse index hash lookup. |
| **OP-03** | Single-Source Traversal | **$1.85\text{ µs}$** | $3.20\text{ µs}$ | $8.40\text{ µs}$ | $22.10\text{ µs}$ | Depth-1 bitset iteration. |
| **OP-04** | Multi-Source Union Traversal | **$62.40\text{ µs}$** | $112.50\text{ µs}$ | $245.80\text{ µs}$ | $580.00\text{ µs}$ | Roaring Treemap SIMD bitmap union. |
| **OP-05** | Transitive Invalidation (Depth $\le 5$) | **$185.20\text{ µs}$** | $420.10\text{ µs}$ | $890.50\text{ µs}$ | $2,150.00\text{ µs}$ | Breadth-first traversal with visited bitset. |
| **OP-06** | Graph Edge Insertion | **$0.28\text{ µs}$** | $0.45\text{ µs}$ | $1.10\text{ µs}$ | $4.50\text{ µs}$ | Bitmap container allocation and insertion. |
| **OP-07** | Graph Edge Deletion | **$0.32\text{ µs}$** | $0.52\text{ µs}$ | $1.25\text{ µs}$ | $5.10\text{ µs}$ | Roaring bit clear and container trim. |
| **OP-08** | Graph Compaction / Trim | **$12.50\text{ µs}$** | $28.00\text{ µs}$ | $65.00\text{ µs}$ | $145.00\text{ µs}$ | In-place run-length encoding optimization. |
| **OP-09** | Invalidation Planning | **$0.45\text{ µs}$** | $0.85\text{ µs}$ | $2.10\text{ µs}$ | $7.50\text{ µs}$ | Epoch comparison and candidate sorting. |
| **OP-10** | L1/L2 Cache Purge | **$4.00\text{ µs}$** | $850.00\text{ µs}$ | **$3,806.40\text{ µs}$** | $8,500.00\text{ µs}$ | Memory cache lock acquisition and key deletion. |
| **OP-11** | Outbox Event Serialization | **$1.20\text{ µs}$** | $2.40\text{ µs}$ | $5.80\text{ µs}$ | $18.20\text{ µs}$ | JSON formatting for outbox broker. |
| **OP-12** | Interrupted Replay Recovery | **$210.00\text{ µs}$** | $520.00\text{ µs}$ | $1,150.00\text{ µs}$ | $3,400.00\text{ µs}$ | Watermark reconciliation and log replay. |

---

## 3. Memory & Resource Footprint Analysis

- **Roaring Bitmaps Scalability:** At 1,000,000 edges, Roaring Bitmaps consume **$2.15\text{ MB}$** ($1.8\text{ bytes/edge}$), representing an $11\times$ reduction compared to standard hash sets ($23.76\text{ MB}$).
- **Lock-Free Read Paths:** Cache reads and epoch lease validations utilize lock-free atomic references (`ArcSwap` and `AtomicU64`), preventing thread contention under high concurrency.
- **Overload & Backpressure:** When client request concurrency exceeds gateway capacity, spend velocity controllers and token bucket rate limiters reject excessive traffic with HTTP 429 in $< 50\text{ µs}$, protecting downstream LLMs and databases from cascading failure.
