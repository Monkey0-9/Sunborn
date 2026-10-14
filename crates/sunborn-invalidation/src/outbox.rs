use crate::graph::DependencyGraph;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sunborn_cache::ExactCacheStore;
use sunborn_core::{Result, TenantId};
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

pub struct InvalidationEngine<C: ExactCacheStore, V: VectorStore> {
    graph: Arc<DependencyGraph>,
    cache: Arc<C>,
    vector_store: Arc<V>,
    tx: mpsc::Sender<InvalidationEvent>,
    rx: Arc<Mutex<mpsc::Receiver<InvalidationEvent>>>,
}

impl<C: ExactCacheStore, V: VectorStore> InvalidationEngine<C, V> {
    pub fn new(graph: Arc<DependencyGraph>, cache: Arc<C>, vector_store: Arc<V>) -> Self {
        let (tx, rx) = mpsc::channel(1024);
        Self {
            graph,
            cache,
            vector_store,
            tx,
            rx: Arc::new(Mutex::new(rx)),
        }
    }

    /// Publish an event to the outbox queue
    pub async fn publish(&self, event: InvalidationEvent) -> Result<()> {
        let _ = self.tx.send(event).await;
        Ok(())
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

        let source_id = Uuid::new_v4();
        let cache_key = "sunborn:tenant1:app1:gpt-4o:v1:hash123";

        // 1. Populate cache and record dependency
        cache
            .set(CacheEntry::new(cache_key, "Answer from old policy", Some(3600)))
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
        assert!(!cache.exists(cache_key).await.unwrap(), "Cache entry must be purged after invalidation");
    }
}
