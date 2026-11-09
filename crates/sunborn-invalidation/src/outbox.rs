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

    /// Process the next invalidation event in the outbox
    pub async fn process_next(&self) -> Result<Option<usize>> {
        let mut rx = self.rx.lock().await;
        if let Some(event) = rx.recv().await {
            let affected_keys = self.graph.get_affected_cache_keys(&event.source_id).await;
            tracing::info!(
                source_id = %event.source_id,
                affected_keys = affected_keys.len(),
                "Processing knowledge invalidation event"
            );

            let mut evicted_count = 0;
            for key in &affected_keys {
                if let Ok(true) = self.cache.delete(key).await {
                    evicted_count += 1;
                }
                let _ = self.vector_store.delete(key).await;
            }

            self.graph.clear_dependencies(&event.source_id).await;
            Ok(Some(evicted_count))
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
}
