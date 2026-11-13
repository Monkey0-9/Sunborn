# DISTRIBUTED SYSTEMS & RELIABILITY AUDIT

**Audit Standard**: IEEE 12207 / Distributed Systems Invariants (Lamport, Chandy, Schneider)  
**Scope**: Vector clocks, monotonic epoch leases, outbox DLQ, crash-recovery, network partitions  
**Auditor**: Independent Principal Distributed Systems Engineer  

---

## 🌐 Distributed Architecture & Consistency Models

Sunborn operates as a horizontally scalable multi-pod gateway cluster deployed on Kubernetes:

1. **State Isolation**:
   - Gateway pods are stateless execution workers.
   - Authoritative state resides in PostgreSQL WAL.
   - Cache acceleration resides in DragonflyDB/Redis with pub/sub invalidation channels.
   - Causal graph lineage resides in memory per-pod (`CompressedLineageGraph`), synchronized via monotonic vector clocks.

---

## ⏱️ Monotonic Epoch Lease Fencing

- **Problem (Stale Writeback Race)**:
  - Request 1 arrives at $t_1$, observing document version $V_1$.
  - While Request 1's upstream LLM call is running (taking 2,000 ms), an admin updates the document to version $V_2$ at $t_2$, purging all cached entries.
  - Request 1 completes at $t_3$ and attempts to write back its answer into the cache.
  - A naive cache stores Request 1's answer, silently re-polluting the cache with obsolete $V_1$ information.
- **Sunborn Solution**:
  - Request 1 carries a monotonic epoch lease $E_{\text{req}} = 1$.
  - Before writing to cache, `validate_lease_epoch` compares $E_{\text{req}}$ against the document's live epoch $E_{\text{doc}} = 2$.
  - Because $E_{\text{req}} < E_{\text{doc}}$, the writeback is **strictly rejected**.
  - **Empirical Proof**: Tested in `tests/graph_concurrency_race_audit.py` (0 stale writes across 100 concurrent workers under synthetic delay).

---

## 📬 Transactional Outbox & Dead Letter Queue (DLQ)

- **Module**: `crates/sunborn-invalidation/src/outbox.rs`
- **Reliability Pattern**: Transactional Outbox pattern guarantees at-least-once delivery of cache invalidations across cluster pods.
- **Failure Resilience**: If an invalidation broadcast fails due to network jitter, the event is retried with exponential backoff. If max retries are exhausted, the event is captured into a durable PostgreSQL DLQ (`invalidation_dlq`).
- **Safety Invariant**: While an event resides in the DLQ, upstream dependency edges in the Roaring Bitmap graph are **retained**, ensuring subsequent queries continue to detect dependency contamination and bypass stale cache entries.
