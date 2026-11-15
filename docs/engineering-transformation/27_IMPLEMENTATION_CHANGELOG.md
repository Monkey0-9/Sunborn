# Implementation Changelog & Engineering Transformation History

**Branch:** `engineering/sunborn-95-quality-transformation`  
**Commits & File Modifications:** 28 core files modified, 11 new production files added  

---

## 1. Summary of Changes

### 1.1 Invalidation Engine (`crates/sunborn-invalidation`)
- **Added:** `src/orchestrator.rs`: Implements `InvalidationOrchestrator` providing two-phase atomic purge across exact cache, Qdrant vector store, and Roaring graph.
- **Modified:** `src/cluster_sync.rs`: Hardened `start_listener()` with `Handle::try_current()` check.
- **Modified:** `src/lib.rs`: Exported `InvalidationOrchestrator` and `PurgeReport`.

### 1.2 Storage & Persistence (`crates/sunborn-db`, `crates/sunborn-gateway`)
- **Added:** `migrations/0005_persistent_epochs_and_dlq.sql`: Monotonic epoch authority and dead-letter queue tables.
- **Modified:** `crates/sunborn-db/src/postgres.rs`: Registered migration 0005 under `_sunborn_migrations`.
- **Modified:** `crates/sunborn-gateway/src/state.rs`: Implemented `PostgresEpochPersistence`, wired `create_cluster_transport`, and added durable outbox background flusher task.
- **Modified:** `crates/sunborn-gateway/src/handlers.rs`: Wired `/v1/invalidation/trigger` to `orchestrator.purge_sources()`.

### 1.3 Semantic Guardrails & Benchmarks (`crates/sunborn-semantic`, `research/`)
- **Added:** `research/calibration/held_out_eval_dataset.json`: Versioned 1.0.0 benchmark dataset.
- **Modified:** `crates/sunborn-semantic/src/guardrails.rs`: Added `test_held_out_evaluation_dataset_precision_and_recall` and updated `HttpCrossEncoderBackend`.
