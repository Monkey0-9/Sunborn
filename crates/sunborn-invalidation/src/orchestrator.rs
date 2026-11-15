//! # Unified Invalidation Orchestrator
//!
//! Atomically coordinates multi-tier cache purging across:
//! 1. Authoritative Monotonic Epoch advancing & Watermark write-back fencing (strictly pre-delete)
//! 2. L1/L2 Exact Cache key eviction
//! 3. Semantic Vector Store point purging
//! 4. Roaring Bitmap SIMD lineage invalidation
//! 5. Dependency Graph edge pruning
//! 6. Cluster Broadcast event emission
//!
//! Enforces zero-tolerance partial purge failure: if ANY store delete fails,
//! an explicit error is returned, failed keys are recorded in `PurgeReport`,
//! and lineage mappings are preserved for retry.

use crate::cluster_sync::{
    ClusterBroadcastTransport, ClusterInvalidationMessage, CLUSTER_INVALIDATION_CHANNEL,
};
use crate::epoch::EpochLeaseManager;
use crate::graph::DependencyGraph;
use crate::graph_cache::GraphMemoryCache;
use crate::watermark::WatermarkLineageFencer;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sunborn_cache::ExactCacheStore;
use sunborn_core::{Result, SunbornError, TenantId};
use sunborn_semantic::VectorStore;
use uuid::Uuid;

/// Comprehensive report produced by atomic multi-tier cache purge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PurgeReport {
    pub purged_cache_keys: Vec<String>,
    pub failed_cache_keys: Vec<(String, String)>,
    pub affected_records_count: usize,
    pub bumped_epoch: u64,
    pub watermark_advanced: bool,
    pub cluster_broadcast_sent: bool,
}

impl PurgeReport {
    pub fn is_fully_successful(&self) -> bool {
        self.failed_cache_keys.is_empty()
    }
}

pub struct InvalidationOrchestrator {
    cache: Arc<dyn ExactCacheStore>,
    vector_store: Arc<dyn VectorStore>,
    graph_memory_cache: Option<Arc<GraphMemoryCache>>,
    dependency_graph: Arc<DependencyGraph>,
    epoch_manager: Arc<EpochLeaseManager>,
    watermark_fencer: Arc<WatermarkLineageFencer>,
    cluster_transport: Option<Arc<dyn ClusterBroadcastTransport>>,
    pod_id: String,
}

impl InvalidationOrchestrator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cache: Arc<dyn ExactCacheStore>,
        vector_store: Arc<dyn VectorStore>,
        graph_memory_cache: Option<Arc<GraphMemoryCache>>,
        dependency_graph: Arc<DependencyGraph>,
        epoch_manager: Arc<EpochLeaseManager>,
        watermark_fencer: Arc<WatermarkLineageFencer>,
        cluster_transport: Option<Arc<dyn ClusterBroadcastTransport>>,
        pod_id: impl Into<String>,
    ) -> Self {
        Self {
            cache,
            vector_store,
            graph_memory_cache,
            dependency_graph,
            epoch_manager,
            watermark_fencer,
            cluster_transport,
            pod_id: pod_id.into(),
        }
    }

    /// Purges all cache entries and vectors dependent on mutated knowledge sources.
    pub async fn purge_sources(
        &self,
        tenant_id: &TenantId,
        source_ids: &[Uuid],
    ) -> Result<PurgeReport> {
        let mut purged_keys = Vec::new();
        let mut failed_keys = Vec::new();
        let mut last_bumped_epoch = 1;

        for src in source_ids {
            // Step 1: Pre-fencing invariant - advance epoch & watermark BEFORE delete (D-10)
            last_bumped_epoch = self.epoch_manager.bump_epoch(*src).await?;
            self.watermark_fencer.advance_watermark(&src.to_string());

            // Step 2: Query dependency graph for affected keys
            let keys = self.dependency_graph.get_affected_cache_keys(src).await;
            for k in &keys {
                let mut key_failed = false;
                // Evict exact cache
                match self.cache.delete(k).await {
                    Ok(_) => purged_keys.push(k.clone()),
                    Err(e) => {
                        key_failed = true;
                        failed_keys.push((k.clone(), format!("exact_cache: {e}")));
                    }
                }

                // Evict vector store
                if let Err(e) = self.vector_store.delete(k).await {
                    key_failed = true;
                    failed_keys.push((k.clone(), format!("vector_store: {e}")));
                }

                // Remove the dependency edge only when every store for THIS key
                // succeeded. A failure for another source must not block cleanup.
                if !key_failed {
                    if let Err(e) = self.dependency_graph.remove_dependency(src, k).await {
                        failed_keys.push((k.clone(), format!("dependency_graph: {e}")));
                    }
                }
            }

            // Step 3: Evict from GraphMemoryCache if active
            if let Some(ref gmc) = self.graph_memory_cache {
                match gmc.invalidate_source_rtc(src).await {
                    Ok(gmc_keys) => purged_keys.extend(gmc_keys),
                    Err(e) => failed_keys.push((format!("gmc:{src}"), e.to_string())),
                }
            }
        }

        purged_keys.sort();
        purged_keys.dedup();

        // Step 4: Emit cluster broadcast to peer replicas
        let mut broadcast_sent = false;
        if let Some(ref transport) = self.cluster_transport {
            let mut msg = ClusterInvalidationMessage::new(
                &self.pod_id,
                tenant_id.clone(),
                vec![],
                source_ids.to_vec(),
                last_bumped_epoch,
            );
            // Carry the exact keys so a peer with a local-only dependency graph
            // can still purge the same exact-cache and vector entries.
            msg.mutated_cache_keys = purged_keys.clone();
            if let Err(e) = transport.publish(CLUSTER_INVALIDATION_CHANNEL, &msg).await {
                failed_keys.push(("cluster_broadcast".to_string(), e.to_string()));
            } else {
                broadcast_sent = true;
            }
        }

        let report = PurgeReport {
            purged_cache_keys: purged_keys,
            failed_cache_keys: failed_keys,
            affected_records_count: source_ids.len(),
            bumped_epoch: last_bumped_epoch,
            watermark_advanced: true,
            cluster_broadcast_sent: broadcast_sent,
        };

        if !report.is_fully_successful() {
            return Err(SunbornError::internal(format!(
                "Orchestrated purge encountered {} store failures",
                report.failed_cache_keys.len()
            ))
            .with_details(serde_json::to_value(&report).unwrap_or_default()));
        }

        Ok(report)
    }

    /// Purges cache entries and vectors by mutated document 64-bit Roaring hashes.
    pub async fn purge_documents(
        &self,
        tenant_id: &TenantId,
        doc_hashes: &[u64],
    ) -> Result<PurgeReport> {
        let mut purged_keys = Vec::new();
        let mut failed_keys = Vec::new();
        let mut affected_records_count = 0;

        if let Some(ref gmc) = self.graph_memory_cache {
            match gmc.invalidate_docs_rtc_purge(doc_hashes).await {
                Ok((affected, doc_keys)) => {
                    affected_records_count = affected.len() as usize;
                    for k in &doc_keys {
                        match self.cache.delete(k).await {
                            Ok(_) => purged_keys.push(k.clone()),
                            Err(e) => failed_keys.push((k.clone(), format!("cache: {e}"))),
                        }
                        if let Err(e) = self.vector_store.delete(k).await {
                            failed_keys.push((k.clone(), format!("vector_store: {e}")));
                        }
                    }
                }
                Err(e) => {
                    failed_keys.push(("docs_rtc_purge".to_string(), e.to_string()));
                }
            }
        }

        purged_keys.sort();
        purged_keys.dedup();

        let mut broadcast_sent = false;
        if let Some(ref transport) = self.cluster_transport {
            let mut msg = ClusterInvalidationMessage::new(
                &self.pod_id,
                tenant_id.clone(),
                doc_hashes.to_vec(),
                vec![],
                0,
            );
            msg.mutated_cache_keys = purged_keys.clone();
            if let Err(e) = transport.publish(CLUSTER_INVALIDATION_CHANNEL, &msg).await {
                failed_keys.push(("cluster_broadcast".to_string(), e.to_string()));
            } else {
                broadcast_sent = true;
            }
        }

        let report = PurgeReport {
            purged_cache_keys: purged_keys,
            failed_cache_keys: failed_keys,
            affected_records_count,
            bumped_epoch: 1,
            watermark_advanced: false,
            cluster_broadcast_sent: broadcast_sent,
        };

        if !report.is_fully_successful() {
            return Err(SunbornError::internal(format!(
                "Document purge failed with {} errors",
                report.failed_cache_keys.len()
            ))
            .with_details(serde_json::to_value(&report).unwrap_or_default()));
        }

        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sunborn_cache::memory::MemoryExactCache;
    use sunborn_semantic::vector_store::MemoryVectorStore;

    #[tokio::test]
    async fn test_invalidation_orchestrator_purge_sources() {
        let cache = Arc::new(MemoryExactCache::new());
        let vector_store = Arc::new(MemoryVectorStore::new());
        let dependency_graph = Arc::new(DependencyGraph::new());
        let epoch_manager = Arc::new(EpochLeaseManager::new());
        let watermark_fencer = Arc::new(WatermarkLineageFencer::new());

        let src = Uuid::new_v4();
        let key = "cache:query:report:alpha";

        // Seed dependencies
        let _ = dependency_graph.record_dependency(src, key).await;
        cache
            .set(sunborn_cache::CacheEntry::new(key, "data", Some(3600)))
            .await
            .unwrap();

        let orchestrator = InvalidationOrchestrator::new(
            cache.clone(),
            vector_store.clone(),
            None,
            dependency_graph.clone(),
            epoch_manager.clone(),
            watermark_fencer.clone(),
            None,
            "pod-test",
        );

        let report = orchestrator
            .purge_sources(&TenantId::new("t1"), &[src])
            .await
            .unwrap();

        assert!(report.is_fully_successful());
        assert_eq!(report.purged_cache_keys, vec![key]);
        assert_eq!(report.bumped_epoch, 2);
        assert!(!cache.exists(key).await.unwrap());
    }
}
