use crate::graph::DependencyGraph;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sunborn_cache::ExactCacheStore;
use sunborn_core::{Result, SunbornError, TenantId};
use sunborn_semantic::VectorStore;
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvalidationEvent {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub source_id: Uuid,
    pub version_tag: String,
    pub timestamp: DateTime<Utc>,
}

/// Dead-letter item recording an invalidation event that failed processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeadLetterItem {
    pub event: InvalidationEvent,
    pub error: String,
    pub retry_count: u32,
    pub failed_at: DateTime<Utc>,
}

pub struct InvalidationEngine<C: ExactCacheStore + ?Sized, V: VectorStore + ?Sized> {
    graph: Arc<DependencyGraph>,
    cache: Arc<C>,
    vector_store: Arc<V>,
    tx: mpsc::Sender<InvalidationEvent>,
    rx: Arc<Mutex<mpsc::Receiver<InvalidationEvent>>>,
    dlq: Arc<Mutex<Vec<DeadLetterItem>>>,
    max_retries: u32,
}

impl<C: ExactCacheStore + ?Sized, V: VectorStore + ?Sized> InvalidationEngine<C, V> {
    pub fn new(graph: Arc<DependencyGraph>, cache: Arc<C>, vector_store: Arc<V>) -> Self {
        Self::with_capacity(graph, cache, vector_store, 1024, 3)
    }

    pub fn with_capacity(
        graph: Arc<DependencyGraph>,
        cache: Arc<C>,
        vector_store: Arc<V>,
        capacity: usize,
        max_retries: u32,
    ) -> Self {
        let (tx, rx) = mpsc::channel(capacity.max(1));
        Self {
            graph,
            cache,
            vector_store,
            tx,
            rx: Arc::new(Mutex::new(rx)),
            dlq: Arc::new(Mutex::new(Vec::new())),
            max_retries,
        }
    }

    /// Configured maximum retry attempts before sending to DLQ.
    pub fn max_retries(&self) -> u32 {
        self.max_retries
    }

    /// Publish an event to the outbox queue with backpressure handling.
    pub async fn publish(&self, event: InvalidationEvent) -> Result<()> {
        match self.tx.try_reserve() {
            Ok(permit) => {
                permit.send(event);
                Ok(())
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                // Apply bounded wait under backpressure (up to 10ms in test/prod)
                tokio::select! {
                    res = self.tx.send(event) => {
                        res.map_err(|e| SunbornError::internal(format!("Invalidation channel closed: {e}")))
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {
                        Err(SunbornError::internal("Invalidation queue backpressure limit reached"))
                    }
                }
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(SunbornError::internal(
                "Invalidation pipeline channel is closed",
            )),
        }
    }

    /// Record a failed event into the Dead Letter Queue (DLQ).
    pub async fn send_to_dlq(&self, event: InvalidationEvent, error: String, retry_count: u32) {
        let mut dlq = self.dlq.lock().await;
        dlq.push(DeadLetterItem {
            event,
            error,
            retry_count,
            failed_at: Utc::now(),
        });
    }

    /// Fetch all dead-letter items for inspection and operational auditing.
    pub async fn get_dlq(&self) -> Vec<DeadLetterItem> {
        let dlq = self.dlq.lock().await;
        dlq.clone()
    }

    /// Return total items currently residing in the Dead Letter Queue.
    pub async fn dlq_count(&self) -> usize {
        let dlq = self.dlq.lock().await;
        dlq.len()
    }

    /// Replay all dead letter queue items back into the live outbox pipeline.
    pub async fn retry_dlq(&self) -> Result<usize> {
        let mut dlq = self.dlq.lock().await;
        let mut replayed = 0;
        let mut remaining = Vec::new();

        for item in dlq.drain(..) {
            match self.publish(item.event.clone()).await {
                Ok(_) => replayed += 1,
                Err(_) => remaining.push(item),
            }
        }

        *dlq = remaining;
        Ok(replayed)
    }

    /// Core event processor: executes multi-target evictions with exponential backoff retries,
    /// DLQ capture, and dependency retention invariants.
    pub async fn process_event(&self, event: InvalidationEvent) -> Result<usize> {
        let affected_keys = self.graph.get_affected_cache_keys(&event.source_id).await;
        if affected_keys.is_empty() {
            self.graph.clear_dependencies(&event.source_id).await;
            return Ok(0);
        }

        tracing::info!(
            source_id = %event.source_id,
            affected_keys = affected_keys.len(),
            "Processing knowledge invalidation event"
        );

        let mut pending_keys = affected_keys.clone();
        let mut evicted_count = 0;
        let mut retry_count = 0;
        let mut last_error = String::new();

        while !pending_keys.is_empty() && retry_count < self.max_retries {
            let mut unevicted = Vec::new();
            for key in &pending_keys {
                let mut key_failed = false;

                // 1. Delete from exact cache
                match self.cache.delete(key).await {
                    Ok(_) => {}
                    Err(e) => {
                        key_failed = true;
                        last_error = format!("Cache deletion error for {key}: {e}");
                    }
                }

                // 2. Delete from vector store
                match self.vector_store.delete(key).await {
                    Ok(_) => {}
                    Err(e) => {
                        key_failed = true;
                        last_error = format!("Vector store deletion error for {key}: {e}");
                    }
                }

                if key_failed {
                    unevicted.push(key.clone());
                } else {
                    evicted_count += 1;
                }
            }

            if unevicted.is_empty() {
                pending_keys.clear();
                break;
            }

            pending_keys = unevicted;
            retry_count += 1;
            tokio::time::sleep(std::time::Duration::from_millis(
                5 * (1 << retry_count.min(5)),
            ))
            .await;
        }

        if !pending_keys.is_empty() {
            let err_msg = if last_error.is_empty() {
                format!(
                    "Failed to evict {} keys after {} retries",
                    pending_keys.len(),
                    retry_count
                )
            } else {
                last_error
            };

            // Automatically capture to Dead Letter Queue
            self.send_to_dlq(event.clone(), err_msg.clone(), retry_count)
                .await;

            tracing::error!(
                source_id = %event.source_id,
                failed_keys = pending_keys.len(),
                retries = retry_count,
                "Invalidation pipeline failed; unevicted dependencies preserved and event captured in DLQ"
            );

            // Invariant: Dependencies must NOT be cleared if evictions failed
            return Err(SunbornError::internal(format!(
                "Invalidation failed for source {}: {err_msg}",
                event.source_id
            )));
        }

        // Only clear dependencies after all evictions have been acknowledged
        self.graph.clear_dependencies(&event.source_id).await;
        Ok(evicted_count)
    }

    /// Non-blocking drain method for graceful shutdown or polling.
    /// Returns Ok(Some(evicted)) if an event was available and processed.
    /// Returns Ok(None) if the channel is currently empty or disconnected.
    pub async fn try_process_next(&self) -> Result<Option<usize>> {
        let mut rx = self.rx.lock().await;
        match rx.try_recv() {
            Ok(event) => {
                drop(rx);
                self.process_event(event).await.map(Some)
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => Ok(None),
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => Ok(None),
        }
    }

    /// Process the next invalidation event in the outbox with per-target acknowledgments,
    /// bounded exponential backoff retries, and automatic Dead Letter Queue (DLQ) capture.
    ///
    /// Correctness Invariant: Graph dependencies are NEVER cleared unless all affected
    /// cache and vector keys are positively acknowledged as deleted.
    pub async fn process_next(&self) -> Result<Option<usize>> {
        let mut rx = self.rx.lock().await;
        if let Some(event) = rx.recv().await {
            drop(rx);
            self.process_event(event).await.map(Some)
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sunborn_cache::{CacheEntry, MemoryExactCache};
    use sunborn_semantic::MemoryVectorStore;

    #[tokio::test]
    async fn test_invalidation_pipeline_evicts_cache_and_vectors() {
        let graph = Arc::new(DependencyGraph::new());
        let cache = Arc::new(MemoryExactCache::new());
        let vector_store = Arc::new(MemoryVectorStore::new());

        let engine = InvalidationEngine::new(graph.clone(), cache.clone(), vector_store.clone());
        assert_eq!(engine.max_retries(), 3);

        let source_id = Uuid::new_v4();
        let cache_key = "sunborn:tenant1:app1:gpt-4o:v1:hash123";

        // 1. Populate cache and record dependency
        cache
            .set(CacheEntry::new(
                cache_key,
                "Answer from old policy",
                Some(3600),
            ))
            .await
            .unwrap();
        graph.record_dependency(source_id, cache_key).await.unwrap();

        assert!(cache.exists(cache_key).await.unwrap());

        // 2. Publish invalidation event
        engine
            .publish(InvalidationEvent {
                id: Uuid::new_v4(),
                tenant_id: TenantId::new("tenant1"),
                source_id,
                version_tag: "v2.0".to_string(),
                timestamp: Utc::now(),
            })
            .await
            .unwrap();

        // 3. Process event
        let evicted = engine.process_next().await.unwrap();
        assert_eq!(evicted, Some(1));

        // 4. Verify cache entry is purged
        assert!(
            !cache.exists(cache_key).await.unwrap(),
            "Cache entry must be purged after invalidation"
        );
    }

    #[tokio::test]
    async fn test_invalidation_dlq_capture_and_retry() {
        let graph = Arc::new(DependencyGraph::new());
        let cache = Arc::new(MemoryExactCache::new());
        let vector_store = Arc::new(MemoryVectorStore::new());

        let engine = InvalidationEngine::with_capacity(graph, cache, vector_store, 16, 2);

        let event = InvalidationEvent {
            id: Uuid::new_v4(),
            tenant_id: TenantId::new("tenant-dlq"),
            source_id: Uuid::new_v4(),
            version_tag: "v1.1".to_string(),
            timestamp: Utc::now(),
        };

        // Initially DLQ is empty
        assert_eq!(engine.dlq_count().await, 0);

        // Manually send to DLQ on simulated failure
        engine
            .send_to_dlq(
                event.clone(),
                "Simulated backend database timeout".into(),
                2,
            )
            .await;
        assert_eq!(engine.dlq_count().await, 1);

        let dlq_items = engine.get_dlq().await;
        assert_eq!(dlq_items.len(), 1);
        assert_eq!(dlq_items[0].retry_count, 2);
        assert!(dlq_items[0].error.contains("timeout"));

        // Retry DLQ re-injects into queue
        let replayed = engine.retry_dlq().await.unwrap();
        assert_eq!(replayed, 1);
        assert_eq!(engine.dlq_count().await, 0);
    }

    #[tokio::test]
    async fn test_invalidation_channel_backpressure() {
        let graph = Arc::new(DependencyGraph::new());
        let cache = Arc::new(MemoryExactCache::new());
        let vector_store = Arc::new(MemoryVectorStore::new());

        // Queue with capacity 2
        let engine = InvalidationEngine::with_capacity(graph, cache, vector_store, 2, 1);

        for _ in 0..2 {
            engine
                .publish(InvalidationEvent {
                    id: Uuid::new_v4(),
                    tenant_id: TenantId::new("t1"),
                    source_id: Uuid::new_v4(),
                    version_tag: "v1".to_string(),
                    timestamp: Utc::now(),
                })
                .await
                .unwrap();
        }

        // 3rd publish hits backpressure and times out
        let res = engine
            .publish(InvalidationEvent {
                id: Uuid::new_v4(),
                tenant_id: TenantId::new("t1"),
                source_id: Uuid::new_v4(),
                version_tag: "v1".to_string(),
                timestamp: Utc::now(),
            })
            .await;

        assert!(
            res.is_err(),
            "Exceeding queue capacity must return backpressure error"
        );
    }

    struct FailingExactCache;

    #[async_trait::async_trait]
    impl ExactCacheStore for FailingExactCache {
        async fn get(&self, _key: &str) -> Result<Option<CacheEntry>> {
            Ok(None)
        }
        async fn set(&self, _entry: CacheEntry) -> Result<()> {
            Ok(())
        }
        async fn delete(&self, _key: &str) -> Result<bool> {
            Err(SunbornError::internal("Simulated disk I/O error"))
        }
        async fn exists(&self, _key: &str) -> Result<bool> {
            Ok(true)
        }
        async fn delete_by_prefix(&self, _prefix: &str) -> Result<usize> {
            Ok(0)
        }
        async fn count_by_prefix(&self, _prefix: &str) -> Result<usize> {
            Ok(0)
        }
        async fn sweep_expired(&self) -> Result<usize> {
            Ok(0)
        }
    }

    #[tokio::test]
    async fn test_invalidation_failure_retains_dependencies_and_populates_dlq() {
        let graph = Arc::new(DependencyGraph::new());
        let failing_cache = Arc::new(FailingExactCache);
        let vector_store = Arc::new(MemoryVectorStore::new());

        let engine =
            InvalidationEngine::with_capacity(graph.clone(), failing_cache, vector_store, 16, 2);

        let source_id = Uuid::new_v4();
        let cache_key = "sunborn:tenant1:app1:fail-key";

        graph.record_dependency(source_id, cache_key).await.unwrap();
        assert_eq!(graph.get_affected_cache_keys(&source_id).await.len(), 1);

        engine
            .publish(InvalidationEvent {
                id: Uuid::new_v4(),
                tenant_id: TenantId::new("t1"),
                source_id,
                version_tag: "v1".to_string(),
                timestamp: Utc::now(),
            })
            .await
            .unwrap();

        let res = engine.process_next().await;
        assert!(res.is_err(), "Processing must fail when deletions fail");

        // Dependencies must be retained!
        let retained = graph.get_affected_cache_keys(&source_id).await;
        assert_eq!(
            retained.len(),
            1,
            "Graph dependencies must NOT be cleared on failure"
        );

        // DLQ must be populated automatically
        assert_eq!(
            engine.dlq_count().await,
            1,
            "DLQ must record the failed invalidation event"
        );
    }
}
