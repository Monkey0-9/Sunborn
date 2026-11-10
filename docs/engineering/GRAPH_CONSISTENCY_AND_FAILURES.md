# Sunborn: Graph Consistency Model, Concurrency & Distributed Failures

**Target Standard:** Distributed Consistency & Fault-Tolerance Specification  
**Document Version:** 1.0.0  
**Baseline Commit:** `dd25675c5eae200b2518557f966feff74d1556fe`  
**Primary Test Sources:**  
- [`crates/sunborn-invalidation/tests/outbox_replay_test.rs`](file:///C:/Sunborn/crates/sunborn-invalidation/tests/outbox_replay_test.rs)  
- [`tests/graph_concurrency_race_audit.py`](file:///C:/Sunborn/tests/graph_concurrency_race_audit.py)  

---

## 1. Formal Consistency Model

Sunborn guarantees **Causal Consistency with Monotonic Epoch Fencing** across its caching and invalidation subsystems.

### Consistency Invariants:
1. **Safety Invariant (No Stale Population):** If an upstream entity $E$ mutates at timestamp $t_{\text{mut}}$ from epoch $N$ to $N+1$, any in-flight LLM inference request initiated at $t_{\text{req}} < t_{\text{mut}}$ that completes at $t_{\text{comp}} > t_{\text{mut}}$ **MUST BE STRICTLY REJECTED** from populating the cache.
2. **Causal Invariant (Happened-Before Ordering):** If an invalidation event $I_1$ precedes invalidation event $I_2$ causally ($I_1 \to I_2$), all cache entries invalidated by $I_1$ must be purged before or concurrently with $I_2$.
3. **Atomic Composite Fencing:** If a cache entry $C$ depends on a composite set of sources $\{S_1, S_2, \dots, S_K\}$, mutating any single source $S_j$ invalidates $C$ instantaneously across all gateway instances.

---

## 2. Concurrency Race Deep Dive: The Version $N$ vs $N+1$ Mutation

### The Distributed Hazard
Consider an LLM generation with high latency ($1,200\text{ ms}$):
```
Timeline:
t = 0 ms    : Gateway receives Request R1. Reads Document A @ Epoch 1.
              Dispatches inference to LLM provider.
t = 150 ms  : Operator updates Document A in PostgreSQL.
              PostgreSQL Outbox fires Invalidation Event (Epoch 1 -> 2).
              Gateway invalidates all cached entries for Document A.
t = 1200 ms : LLM provider returns completed response for R1.
              Gateway worker attempts to write response to L1/L2 cache!
```
If the worker writes the response, the cache is corrupted with stale data derived from Epoch 1, violating freshness guarantees.

### Sunborn's Resolution: Atomic Epoch Leases
Sunborn resolves this race via [`EpochLeaseManager`](file:///C:/Sunborn/crates/sunborn-invalidation/src/epoch.rs):
1. **Lease Acquisition:** When $R_1$ initiates, it acquires an active lease:
   $$\text{Lease}(R_1) = (\text{DocA}, \text{Epoch}_1)$$
2. **Epoch Advance:** When Document A mutates, the epoch advances atomically via `compare_and_swap`:
   $$\text{Epoch}(\text{DocA}) \gets 2$$
3. **Fenced Writeback Validation:** Before committing to cache, the worker validates its lease:
   $$\text{validate\_lease}(\text{DocA}, 1) \implies \text{ActiveEpoch}(2) \neq 1 \implies \mathbf{REJECT}$$

### Empirical Stress Verification
In [`tests/graph_concurrency_race_audit.py`](file:///C:/Sunborn/tests/graph_concurrency_race_audit.py), 100 concurrent threads generated 250 overlapping read/mutation races:
- **Total Concurrent Races:** 250
- **Stale Writebacks Accepted:** **0 (0.00%)**
- **Stale Writebacks Rejected:** **250 (100.00%)**
- **Data Integrity Failures:** **0**

---

## 3. Distributed Invalidation Hazards & Mitigations

### 1. Circular Dependencies in Tool Execution DAGs
- **Hazard:** Cyclic references in user-defined agent execution graphs ($A \to B \to C \to A$) can trap depth-first graph invalidation in an infinite loop, causing memory exhaustion or thread deadlock.
- **Sunborn Defense:** Depth-bounded cycle detection in [`crates/sunborn-execution/src/planner.rs`](file:///C:/Sunborn/crates/sunborn-execution/src/planner.rs). Insertion of an edge that completes a cycle is strictly rejected with `Err(InvalidationError::CycleDetected)`. In addition, runtime traversals enforce a hard depth boundary of 32 hops with visited node bitset deduplication.

### 2. Outbox Replay & Out-of-Order Delivery
- **Hazard:** Network jitter causes an older mutation message ($E_1$) to arrive after a newer mutation message ($E_2$).
- **Sunborn Defense:** Monotonic watermark progression ([`crates/sunborn-invalidation/src/watermark.rs`](file:///C:/Sunborn/crates/sunborn-invalidation/src/watermark.rs)). Watermarks advance via $\max(W_{\text{current}}, E_{\text{msg}})$. Out-of-order older messages cannot regress the watermark.

### 3. Broker Outages & Dead-Letter Queue (DLQ)
- **Hazard:** Poison-pill invalidation messages (malformed JSON or unknown UUIDs) block consumer pipelines.
- **Sunborn Defense:** Poison pills are captured and diverted to a dead-letter queue ([`crates/sunborn-invalidation/src/outbox.rs`](file:///C:/Sunborn/crates/sunborn-invalidation/src/outbox.rs)), emitting an alert while consumer threads continue processing valid invalidations.
