//! # Multi-Node Cluster Invalidation Broadcast & Distributed Graph Sync
//!
//! Resolves Gap 4: Node-Local Roaring Bitmaps in Multi-Node Clusters.
//!
//! In horizontally scaled deployments (e.g. 10 gateway pods behind a load balancer),
//! mutations on Pod A must immediately invalidate local in-memory L1 caches and
//! `CompressedLineageGraph` Roaring bitmaps across all peer pods (Pod B, Pod C, ...)
//! without awaiting TTL expiration or falling into stale cache read windows.
//!
//! Provides `ClusterInvalidationMessage`, `ClusterBroadcastTransport`,
//! and `DistributedGraphSyncEngine` communicating over `sunborn:invalidation:events`.

use crate::graph_cache::GraphMemoryCache;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use sunborn_core::{Result, TenantId};
use tokio::sync::broadcast;
use uuid::Uuid;

/// Default Redis / cluster pub-sub topic for invalidation events
pub const CLUSTER_INVALIDATION_CHANNEL: &str = "sunborn:invalidation:events";

/// Broadcast payload distributed across cluster nodes on knowledge mutation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClusterInvalidationMessage {
    pub message_id: Uuid,
    pub origin_pod_id: String,
    pub tenant_id: TenantId,
    pub mutated_doc_hashes: Vec<u64>,
    pub mutated_source_ids: Vec<Uuid>,
    pub monotonic_epoch: u64,
    pub timestamp: DateTime<Utc>,
}

impl ClusterInvalidationMessage {
    pub fn new(
        origin_pod_id: impl Into<String>,
        tenant_id: TenantId,
        mutated_doc_hashes: Vec<u64>,
        mutated_source_ids: Vec<Uuid>,
        monotonic_epoch: u64,
    ) -> Self {
        Self {
            message_id: Uuid::new_v4(),
            origin_pod_id: origin_pod_id.into(),
            tenant_id,
            mutated_doc_hashes,
            mutated_source_ids,
            monotonic_epoch,
            timestamp: Utc::now(),
        }
    }
}

/// Abstract transport for cluster pub-sub event distribution
#[async_trait]
pub trait ClusterBroadcastTransport: Send + Sync {
    async fn publish(&self, channel: &str, message: &ClusterInvalidationMessage) -> Result<()>;
    async fn subscribe(
        &self,
        channel: &str,
    ) -> Result<broadcast::Receiver<ClusterInvalidationMessage>>;
}

/// In-memory cluster broadcast transport using tokio broadcast channels
pub struct InMemoryClusterTransport {
    sender: broadcast::Sender<ClusterInvalidationMessage>,
}

impl InMemoryClusterTransport {
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity.max(16));
        Self { sender }
    }
}

impl Default for InMemoryClusterTransport {
    fn default() -> Self {
        Self::new(1024)
    }
}

#[async_trait]
impl ClusterBroadcastTransport for InMemoryClusterTransport {
    async fn publish(&self, _channel: &str, message: &ClusterInvalidationMessage) -> Result<()> {
        let _ = self.sender.send(message.clone());
        Ok(())
    }

    async fn subscribe(
        &self,
        _channel: &str,
    ) -> Result<broadcast::Receiver<ClusterInvalidationMessage>> {
        Ok(self.sender.subscribe())
    }
}

/// Distributed Graph Synchronization Engine
///
/// Coordinates cross-pod invalidation propagation:
/// 1. Emits cluster broadcast when local pod processes knowledge mutations.
/// 2. Subscribes to cluster broadcast and immediately purges local in-memory L1 cache
///    and Roaring bitmap lineage structures when peer pods broadcast.
/// 3. Suppresses local self-echoes to avoid redundant local invalidation work.
pub struct DistributedGraphSyncEngine {
    pod_id: String,
    transport: Arc<dyn ClusterBroadcastTransport>,
    graph_cache: Arc<GraphMemoryCache>,
    messages_published: AtomicU64,
    messages_received: AtomicU64,
    echoes_suppressed: AtomicU64,
    records_invalidated: AtomicU64,
}

impl DistributedGraphSyncEngine {
    pub fn new(
        pod_id: impl Into<String>,
        transport: Arc<dyn ClusterBroadcastTransport>,
        graph_cache: Arc<GraphMemoryCache>,
    ) -> Self {
        Self {
            pod_id: pod_id.into(),
            transport,
            graph_cache,
            messages_published: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            echoes_suppressed: AtomicU64::new(0),
            records_invalidated: AtomicU64::new(0),
        }
    }

    pub fn pod_id(&self) -> &str {
        &self.pod_id
    }

    pub fn messages_published(&self) -> u64 {
        self.messages_published.load(Ordering::Relaxed)
    }

    pub fn messages_received(&self) -> u64 {
        self.messages_received.load(Ordering::Relaxed)
    }

    pub fn echoes_suppressed(&self) -> u64 {
        self.echoes_suppressed.load(Ordering::Relaxed)
    }

    pub fn records_invalidated(&self) -> u64 {
        self.records_invalidated.load(Ordering::Relaxed)
    }

    /// Publishes a knowledge mutation to peer pods and invalidates local cache.
    pub async fn publish_and_invalidate(
        &self,
        tenant_id: TenantId,
        doc_hashes: Vec<u64>,
        source_ids: Vec<Uuid>,
        monotonic_epoch: u64,
    ) -> Result<usize> {
        // 1. Invalidate locally first
        let local_affected = self
            .apply_local_invalidation(&doc_hashes, &source_ids)
            .await?;

        // 2. Broadcast to cluster peers
        let msg = ClusterInvalidationMessage::new(
            &self.pod_id,
            tenant_id,
            doc_hashes,
            source_ids,
            monotonic_epoch,
        );

        self.transport
            .publish(CLUSTER_INVALIDATION_CHANNEL, &msg)
            .await?;
        self.messages_published.fetch_add(1, Ordering::Relaxed);

        Ok(local_affected)
    }

    /// Handles a cluster message received from the broadcast channel.
    pub async fn handle_cluster_message(
        &self,
        message: &ClusterInvalidationMessage,
    ) -> Result<usize> {
        // Echo suppression: Ignore notifications originating from self
        if message.origin_pod_id == self.pod_id {
            self.echoes_suppressed.fetch_add(1, Ordering::Relaxed);
            return Ok(0);
        }

        self.messages_received.fetch_add(1, Ordering::Relaxed);
        let affected = self
            .apply_local_invalidation(&message.mutated_doc_hashes, &message.mutated_source_ids)
            .await?;
        self.records_invalidated
            .fetch_add(affected as u64, Ordering::Relaxed);

        Ok(affected)
    }

    async fn apply_local_invalidation(
        &self,
        doc_hashes: &[u64],
        source_ids: &[Uuid],
    ) -> Result<usize> {
        let mut total_affected = 0;

        // 1. Invalidate 64-bit Roaring Treemap document lineage and purge resolved cache keys
        if !doc_hashes.is_empty() {
            let (roaring_affected, _doc_keys) = self
                .graph_cache
                .invalidate_docs_rtc_purge(doc_hashes)
                .await?;
            total_affected += roaring_affected.len() as usize;
        }

        // 2. Invalidate upstream knowledge source dependencies
        for source_id in source_ids {
            let source_keys = self.graph_cache.invalidate_source_rtc(source_id).await?;
            total_affected += source_keys.len();
        }

        Ok(total_affected)
    }

    /// Spawns a background listener task that monitors the cluster transport channel.
    pub fn start_listener(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if let Ok(mut rx) = self.transport.subscribe(CLUSTER_INVALIDATION_CHANNEL).await {
                while let Ok(msg) = rx.recv().await {
                    let _ = self.handle_cluster_message(&msg).await;
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sunborn_cache::{CacheEntry, ExactCacheStore};

    #[tokio::test]
    async fn test_multi_pod_distributed_graph_sync() {
        // Shared cluster transport bus
        let transport = Arc::new(InMemoryClusterTransport::new(256));

        // Pod 1
        let cache_pod1 = Arc::new(GraphMemoryCache::new_default());
        let sync_pod1 = Arc::new(DistributedGraphSyncEngine::new(
            "pod-us-east-1a",
            transport.clone(),
            cache_pod1.clone(),
        ));
        let _listener_pod1 = sync_pod1.clone().start_listener();

        // Pod 2
        let cache_pod2 = Arc::new(GraphMemoryCache::new_default());
        let sync_pod2 = Arc::new(DistributedGraphSyncEngine::new(
            "pod-us-east-1b",
            transport.clone(),
            cache_pod2.clone(),
        ));
        let _listener_pod2 = sync_pod2.clone().start_listener();

        // Wait briefly for subscribers to attach
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let src_doc: u64 = 0xdead_beef_1337;
        let tenant_id = TenantId::new("tenant-finance-alpha");

        // Seed Pod 1 with cached query dependent on src_doc
        let entry1 = CacheEntry::new("cache:financial:report:q3", "revenue: $4.2B", Some(3600));
        cache_pod1
            .set_with_lineage(entry1, &[], &[src_doc], 101)
            .await
            .expect("set pod1");

        // Seed Pod 2 with cached query dependent on src_doc
        let entry2 = CacheEntry::new("cache:financial:report:summary", "net: $1.1B", Some(3600));
        cache_pod2
            .set_with_lineage(entry2, &[], &[src_doc], 202)
            .await
            .expect("set pod2");

        assert!(cache_pod1.lineage_graph().contains_document(src_doc));
        assert!(cache_pod2.lineage_graph().contains_document(src_doc));

        // Pod 1 receives an external document mutation and broadcasts cluster sync
        let local_evicted = sync_pod1
            .publish_and_invalidate(tenant_id, vec![src_doc], vec![], 42)
            .await
            .expect("broadcast");

        assert_eq!(local_evicted, 1);
        assert!(!cache_pod1.lineage_graph().contains_document(src_doc));
        assert!(!cache_pod1
            .exists("cache:financial:report:q3")
            .await
            .unwrap());

        // Allow cluster broadcast propagation to Pod 2
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Pod 2 must have processed the cluster broadcast and evicted its local Roaring bitmap and cache key!
        assert!(!cache_pod2.lineage_graph().contains_document(src_doc));
        assert!(!cache_pod2
            .exists("cache:financial:report:summary")
            .await
            .unwrap());
        assert_eq!(sync_pod2.messages_received(), 1);
        assert_eq!(sync_pod2.records_invalidated(), 1);

        // Echo suppression: Pod 1 must have suppressed its own echo
        assert_eq!(sync_pod1.echoes_suppressed(), 1);
        assert_eq!(sync_pod1.messages_published(), 1);
    }
}
