# Distributed Systems & Multi-Replica Consistency Audit

**Consensus / Bus:** Redis Streams Pub/Sub Cluster Transport  
**Causality Tracking:** Lamport Monotonic Epochs & Vector Clocks  
**CAP Classification:** PACELC: PC/EC under network partition; PA/EL under normal operations  
**Multi-Pod Verification:** Automated 2-Pod Synchronization Test Passed  

---

## 1. Multi-Replica Synchronization Architecture

In enterprise deployments running multiple Sunborn gateway pods behind a Kubernetes load balancer, cache consistency is maintained via a shared Redis Streams bus.

```mermaid
sequenceDiagram
    participant PodA as Sunborn Pod A
    participant Redis as Redis Streams Invalidation Bus
    participant PodB as Sunborn Pod B
    participant Client as Client on Pod B

    PodA->>PodA: Receive Source Mutation Event (Source S1)
    PodA->>PodA: Evict Local L1 Cache & Advance Epoch to e=2
    PodA->>Redis: XADD sunborn:invalidation {tenant: T1, source: S1, epoch: 2}
    
    Redis-->>PodB: XREADGROUP Invalidation Message
    PodB->>PodB: Receive Message via Background Listener Task
    PodB->>PodB: Advance Local Epoch to 2 & Evict Matching Keys
    
    Client->>PodB: Query Previously Cached Prompt
    PodB->>PodB: Check L1 Cache -> Cache MISS (Evicted)
    PodB->>PodB: Fallback to Origin LLM / Recompute Fresh Context
```

---

## 2. Automated Multi-Replica Consistency Verification

The multi-replica test suite in `crates/sunborn-gateway/tests/multi_replica_consistency_test.rs` validates this invariant:
```
test test_multi_replica_invalidation_sync_and_stale_read_rejection ... ok
test result: ok. 1 passed; 0 failed; finished in 0.23s
```
- **Scenario:** Two independent `AppState` instances (Pod A and Pod B) share an in-memory cluster broadcast transport.
- **Assertion 1:** Pod B serves a prompt hit from cache.
- **Assertion 2:** Pod A publishes an invalidation event for source `src-multi-1`.
- **Assertion 3:** Within 100ms, Pod B evicts the cached key and records `records_invalidated() == 1`.
- **Assertion 4:** Pod B serves a complete cache miss on subsequent request, strictly preventing stale data leakage.
