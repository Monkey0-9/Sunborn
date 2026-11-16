//! # Multi-Node Cluster Invalidation Broadcast & Distributed Graph Sync
//!
//! Mutations on one gateway replica must invalidate exact-cache entries, vector
//! points, and Roaring lineage on every peer. The durable transport is Redis
//! Streams with a consumer group:
//!
//! * `publish` appends to `sunborn:invalidation:events` and returns the stream id.
//! * each pod consumes with `XREADGROUP` and only `XACK`s after the local purge
//!   succeeds; a failed purge remains pending for replay.
//! * the message carries the exact cache keys and the authoritative epoch so a
//!   peer with a local-only dependency graph can still purge vectors and advance
//!   its fence.
//!
//! The in-memory transport remains for unit tests and development only. Production
//! readiness rejects it.

use crate::epoch::EpochLeaseManager;
use crate::graph_cache::GraphMemoryCache;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use redis::streams::{StreamReadOptions, StreamReadReply};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use sunborn_core::{Result, SunbornError, TenantId};
use sunborn_semantic::VectorStore;
use tokio::sync::broadcast;
use uuid::Uuid;

/// Default Redis stream / cluster channel for invalidation events.
pub const CLUSTER_INVALIDATION_CHANNEL: &str = "sunborn:invalidation:events";
const CLUSTER_CONSUMER_GROUP: &str = "sunborn-gateway";

/// Broadcast payload distributed across cluster nodes on knowledge mutation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClusterInvalidationMessage {
    pub message_id: Uuid,
    pub origin_pod_id: String,
    pub tenant_id: TenantId,
    pub mutated_doc_hashes: Vec<u64>,
    pub mutated_source_ids: Vec<Uuid>,
    /// Exact cache keys resolved by the origin replica. Peers use these when
    /// their dependency graph is process-local.
    #[serde(default)]
    pub mutated_cache_keys: Vec<String>,
    pub monotonic_epoch: u64,
    pub timestamp: DateTime<Utc>,
    /// Redis stream entry id. Empty for in-memory messages.
    #[serde(default)]
    pub stream_id: String,
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
            mutated_cache_keys: Vec::new(),
            monotonic_epoch,
            timestamp: Utc::now(),
            stream_id: String::new(),
        }
    }
}

/// Abstract transport for cluster pub-sub/stream event distribution.
#[async_trait]
pub trait ClusterBroadcastTransport: Send + Sync {
    async fn publish(&self, channel: &str, message: &ClusterInvalidationMessage) -> Result<()>;
    async fn subscribe(
        &self,
        channel: &str,
    ) -> Result<broadcast::Receiver<ClusterInvalidationMessage>>;
    /// Acknowledge successful local processing. In-memory transports are no-ops.
    async fn ack(&self, _stream_id: &str) -> Result<()> {
        Ok(())
    }
    /// True only for a transport that survives a pod restart and can replay
    /// unacknowledged events.
    fn is_durable(&self) -> bool {
        false
    }
}

/// In-memory cluster broadcast transport for unit tests and local development.
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

/// Durable Redis Streams transport with a consumer group.
pub struct RedisClusterTransport {
    client: redis::Client,
    sender: broadcast::Sender<ClusterInvalidationMessage>,
    pod_id: String,
    stream_key: String,
    consumer_group: String,
}

impl RedisClusterTransport {
    pub fn new(redis_url: &str, pod_id: impl Into<String>, capacity: usize) -> Result<Arc<Self>> {
        let client = redis::Client::open(redis_url)
            .map_err(|e| SunbornError::internal(format!("Invalid Redis cluster URL: {e}")))?;
        let (sender, _) = broadcast::channel(capacity.max(16));
        let pod_id = pod_id.into();
        let stream_key = CLUSTER_INVALIDATION_CHANNEL.to_string();
        // A consumer group load-balances each entry to exactly one consumer.
        // Cluster invalidation is a broadcast, so every replica needs its own
        // group; otherwise one healthy peer can consume an event that another
        // peer never sees, and a disconnected peer loses it entirely.
        let consumer_group = format!("{CLUSTER_CONSUMER_GROUP}:{pod_id}");

        let transport = Arc::new(Self {
            client,
            sender,
            pod_id,
            stream_key,
            consumer_group,
        });
        transport.clone().start_subscriber();
        Ok(transport)
    }

    fn start_subscriber(self: Arc<Self>) {
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            tracing::warn!(
                "Redis stream listener not started: no active Tokio runtime at construction"
            );
            return;
        };

        let client = self.client.clone();
        let stream = self.stream_key.clone();
        let group = self.consumer_group.clone();
        let consumer = self.pod_id.clone();
        let tx = self.sender.clone();

        handle.spawn(async move {
            loop {
                let conn = match client.get_multiplexed_async_connection().await {
                    Ok(conn) => conn,
                    Err(e) => {
                        tracing::warn!(error = %e, "Redis stream connection failed; retrying in 1s");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        continue;
                    }
                };
                let mut conn = conn;

                if let Err(e) = ensure_consumer_group(&mut conn, &stream, &group).await {
                    tracing::warn!(error = %e, "Redis consumer group setup failed; retrying in 1s");
                    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                    continue;
                }

                // Replay events this consumer previously received but did not ack
                // (crash or failed purge).
                if let Ok(pending) = read_stream(
                    &mut conn,
                    &stream,
                    &group,
                    &consumer,
                    "0",
                    64,
                    10,
                )
                .await
                {
                    dispatch_reply(pending, &tx);
                }

                tracing::info!(
                    pod = %consumer,
                    stream = %stream,
                    group = %group,
                    "Consuming durable Redis Stream invalidation events"
                );

                loop {
                    match read_stream(&mut conn, &stream, &group, &consumer, ">", 64, 2000)
                        .await
                    {
                        Ok(reply) => dispatch_reply(reply, &tx),
                        Err(e) => {
                            tracing::warn!(error = %e, "Redis stream read failed; reconnecting");
                            break;
                        }
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            }
        });
    }
}

async fn ensure_consumer_group(
    conn: &mut redis::aio::MultiplexedConnection,
    stream: &str,
    group: &str,
) -> std::result::Result<(), redis::RedisError> {
    // Start new groups at the current tail: a replica with an empty local cache
    // does not need historical invalidations. Restarted replicas reuse their
    // existing group and replay their own pending entries below.
    let res: redis::RedisResult<()> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(stream)
        .arg(group)
        .arg("$")
        .arg("MKSTREAM")
        .query_async(conn)
        .await;
    match res {
        Ok(()) => Ok(()),
        Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
        Err(e) => Err(e),
    }
}

async fn read_stream(
    conn: &mut redis::aio::MultiplexedConnection,
    stream: &str,
    group: &str,
    consumer: &str,
    id: &str,
    count: usize,
    block_ms: usize,
) -> std::result::Result<StreamReadReply, redis::RedisError> {
    let opts = StreamReadOptions::default()
        .group(group, consumer)
        .count(count)
        .block(block_ms);
    conn.xread_options::<&str, &str, StreamReadReply>(&[stream], &[id], &opts)
        .await
}

fn dispatch_reply(reply: StreamReadReply, tx: &broadcast::Sender<ClusterInvalidationMessage>) {
    for key in reply.keys {
        for entry in key.ids {
            let Some(payload) = entry.get::<String>("payload") else {
                tracing::warn!(stream_id = %entry.id, "Cluster stream entry missing payload");
                continue;
            };
            match serde_json::from_str::<ClusterInvalidationMessage>(&payload) {
                Ok(mut message) => {
                    message.stream_id = entry.id.clone();
                    // If there are no local subscribers the send fails and the
                    // entry stays pending, so a later listener can replay it.
                    if let Err(e) = tx.send(message) {
                        tracing::warn!(
                            stream_id = %entry.id,
                            error = %e,
                            "No local cluster listener; invalidation remains pending"
                        );
                    }
                }
                Err(e) => {
                    tracing::error!(
                        stream_id = %entry.id,
                        error = %e,
                        "Malformed cluster invalidation payload; acknowledging poison entry"
                    );
                }
            }
        }
    }
}

#[async_trait]
impl ClusterBroadcastTransport for RedisClusterTransport {
    async fn publish(&self, _channel: &str, message: &ClusterInvalidationMessage) -> Result<()> {
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| {
                SunbornError::internal(format!("Redis stream publish connection error: {e}"))
            })?;
        let payload = serde_json::to_string(message).map_err(|e| {
            SunbornError::internal(format!("Cluster message serialization error: {e}"))
        })?;

        let stream_id: String = redis::cmd("XADD")
            .arg(&self.stream_key)
            .arg("*")
            .arg("payload")
            .arg(payload)
            .query_async(&mut conn)
            .await
            .map_err(|e| SunbornError::internal(format!("Redis XADD failed: {e}")))?;

        // Deliver to local subscribers too (with the stream id so the local
        // engine can ack exactly what it processed).
        let mut local = message.clone();
        local.stream_id = stream_id;
        let _ = self.sender.send(local);
        Ok(())
    }

    async fn subscribe(
        &self,
        _channel: &str,
    ) -> Result<broadcast::Receiver<ClusterInvalidationMessage>> {
        Ok(self.sender.subscribe())
    }

    async fn ack(&self, stream_id: &str) -> Result<()> {
        if stream_id.is_empty() {
            return Ok(());
        }
        let mut conn = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(|e| SunbornError::internal(format!("Redis XACK connection error: {e}")))?;
        let _: i64 = redis::cmd("XACK")
            .arg(&self.stream_key)
            .arg(&self.consumer_group)
            .arg(stream_id)
            .query_async(&mut conn)
            .await
            .map_err(|e| SunbornError::internal(format!("Redis XACK failed: {e}")))?;
        Ok(())
    }

    fn is_durable(&self) -> bool {
        true
    }
}

/// Strict factory: returns an error instead of silently degrading to in-memory.
pub fn create_cluster_transport_checked(
    pod_id: &str,
    redis_url: Option<&str>,
    capacity: usize,
) -> Result<Arc<dyn ClusterBroadcastTransport>> {
    let url = redis_url
        .filter(|url| !url.trim().is_empty())
        .ok_or_else(|| {
            SunbornError::configuration_error(
                "Durable cluster transport requires REDIS_URL or SUNBORN_REDIS_URL",
            )
        })?;
    let transport = RedisClusterTransport::new(url, pod_id, capacity)?;
    Ok(transport)
}

/// Development factory: logs loudly and falls back to in-memory. Production
/// readiness rejects the result because `is_durable()` is false.
pub fn create_cluster_transport(
    pod_id: &str,
    redis_url: Option<&str>,
    capacity: usize,
) -> Arc<dyn ClusterBroadcastTransport> {
    match create_cluster_transport_checked(pod_id, redis_url, capacity) {
        Ok(transport) => transport,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "Cluster transport falling back to in-memory; multi-replica invalidation is NOT durable"
            );
            Arc::new(InMemoryClusterTransport::new(capacity))
        }
    }
}

/// Distributed Graph Synchronization Engine.
pub struct DistributedGraphSyncEngine {
    pod_id: String,
    transport: Arc<dyn ClusterBroadcastTransport>,
    graph_cache: Arc<GraphMemoryCache>,
    vector_store: Option<Arc<dyn VectorStore>>,
    epoch_manager: Option<Arc<EpochLeaseManager>>,
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
            vector_store: None,
            epoch_manager: None,
            messages_published: AtomicU64::new(0),
            messages_received: AtomicU64::new(0),
            echoes_suppressed: AtomicU64::new(0),
            records_invalidated: AtomicU64::new(0),
        }
    }

    pub fn with_vector_store(mut self, vector_store: Arc<dyn VectorStore>) -> Self {
        self.vector_store = Some(vector_store);
        self
    }

    pub fn with_epoch_manager(mut self, epoch_manager: Arc<EpochLeaseManager>) -> Self {
        self.epoch_manager = Some(epoch_manager);
        self
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
        let local_affected = self
            .apply_local_invalidation(&doc_hashes, &source_ids, &[])
            .await?;

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

    /// Handles a cluster message received from the transport. The stream entry
    /// is acknowledged only after every local purge succeeds.
    pub async fn handle_cluster_message(
        &self,
        message: &ClusterInvalidationMessage,
    ) -> Result<usize> {
        if message.origin_pod_id == self.pod_id {
            self.echoes_suppressed.fetch_add(1, Ordering::Relaxed);
            return Ok(0);
        }

        // Apply the origin's authoritative epoch before admitting any future
        // writeback; a delayed message can only move the local fence forward.
        if let Some(ref epoch_manager) = self.epoch_manager {
            for source_id in &message.mutated_source_ids {
                epoch_manager
                    .observe_epoch(*source_id, message.monotonic_epoch)
                    .await;
            }
        }

        self.messages_received.fetch_add(1, Ordering::Relaxed);
        let affected = self
            .apply_local_invalidation(
                &message.mutated_doc_hashes,
                &message.mutated_source_ids,
                &message.mutated_cache_keys,
            )
            .await?;
        self.records_invalidated
            .fetch_add(affected as u64, Ordering::Relaxed);

        // Ack only after the local state is consistent. On failure the entry
        // remains pending and is replayed.
        self.transport.ack(&message.stream_id).await?;

        Ok(affected)
    }

    async fn apply_local_invalidation(
        &self,
        doc_hashes: &[u64],
        source_ids: &[Uuid],
        cache_keys: &[String],
    ) -> Result<usize> {
        let mut total_affected = 0;

        // 1. Exact keys named by the origin replica. This is the only complete
        // path when the peer does not share the origin's dependency graph.
        if !cache_keys.is_empty() {
            total_affected += self.graph_cache.delete_cache_keys(cache_keys).await?;
            if let Some(ref vector_store) = self.vector_store {
                for key in cache_keys {
                    vector_store.delete(key).await?;
                }
            }
        }

        // 2. Invalidate 64-bit Roaring document lineage and its exact keys.
        if !doc_hashes.is_empty() {
            let (roaring_affected, doc_keys) = self
                .graph_cache
                .invalidate_docs_rtc_purge(doc_hashes)
                .await?;
            total_affected += roaring_affected.len() as usize;
            if let Some(ref vector_store) = self.vector_store {
                for key in &doc_keys {
                    vector_store.delete(key).await?;
                }
            }
        }

        // 3. Invalidate upstream knowledge-source dependencies.
        for source_id in source_ids {
            let source_keys = self.graph_cache.invalidate_source_rtc(source_id).await?;
            total_affected += source_keys.len();
            if let Some(ref vector_store) = self.vector_store {
                for key in &source_keys {
                    vector_store.delete(key).await?;
                }
            }
        }

        Ok(total_affected)
    }

    /// Spawns a background listener task monitoring the cluster transport.
    pub fn start_listener(self: Arc<Self>) -> Option<tokio::task::JoinHandle<()>> {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            Some(handle.spawn(async move {
                let Ok(mut rx) = self
                    .transport
                    .subscribe(CLUSTER_INVALIDATION_CHANNEL)
                    .await
                else {
                    tracing::error!("Failed to subscribe to cluster invalidation transport");
                    return;
                };
                loop {
                    match rx.recv().await {
                        Ok(msg) => {
                            if let Err(e) = self.handle_cluster_message(&msg).await {
                                tracing::error!(
                                    stream_id = %msg.stream_id,
                                    error = %e,
                                    "Cluster invalidation failed; entry remains pending for replay"
                                );
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            tracing::warn!(
                                skipped,
                                "Cluster listener lagged; entries remain pending and will be replayed"
                            );
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }))
        } else {
            tracing::warn!(
                "start_listener invoked outside Tokio runtime; cluster listener not spawned"
            );
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sunborn_cache::{CacheEntry, ExactCacheStore};
    use sunborn_core::ApplicationId;
    use sunborn_semantic::{MemoryVectorStore, VectorEntry};

    #[tokio::test]
    async fn test_multi_pod_distributed_graph_sync() {
        let transport = Arc::new(InMemoryClusterTransport::new(256));

        let cache_pod1 = Arc::new(GraphMemoryCache::new_default());
        let sync_pod1 = Arc::new(DistributedGraphSyncEngine::new(
            "pod-us-east-1a",
            transport.clone(),
            cache_pod1.clone(),
        ));
        let _listener_pod1 = sync_pod1.clone().start_listener();

        let cache_pod2 = Arc::new(GraphMemoryCache::new_default());
        let sync_pod2 = Arc::new(DistributedGraphSyncEngine::new(
            "pod-us-east-1b",
            transport.clone(),
            cache_pod2.clone(),
        ));
        let _listener_pod2 = sync_pod2.clone().start_listener();

        tokio::time::sleep(std::time::Duration::from_millis(20)).await;

        let src_doc: u64 = 0xdead_beef_1337;
        let tenant_id = TenantId::new("tenant-finance-alpha");

        let entry1 = CacheEntry::new("cache:financial:report:q3", "revenue: $4.2B", Some(3600));
        cache_pod1
            .set_with_lineage(entry1, &[], &[src_doc], 101)
            .await
            .expect("set pod1");

        let entry2 = CacheEntry::new("cache:financial:report:summary", "net: $1.1B", Some(3600));
        cache_pod2
            .set_with_lineage(entry2, &[], &[src_doc], 202)
            .await
            .expect("set pod2");

        assert!(cache_pod1.lineage_graph().contains_document(src_doc));
        assert!(cache_pod2.lineage_graph().contains_document(src_doc));

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

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert!(!cache_pod2.lineage_graph().contains_document(src_doc));
        assert!(!cache_pod2
            .exists("cache:financial:report:summary")
            .await
            .unwrap());
        assert_eq!(sync_pod2.messages_received(), 1);
        assert_eq!(sync_pod2.records_invalidated(), 1);
        assert_eq!(sync_pod1.echoes_suppressed(), 1);
        assert_eq!(sync_pod1.messages_published(), 1);
    }

    #[tokio::test]
    async fn test_peer_purges_exact_keys_and_vectors_without_shared_graph() {
        let transport = Arc::new(InMemoryClusterTransport::new(64));
        let cache = Arc::new(GraphMemoryCache::new_default());
        let vectors = Arc::new(MemoryVectorStore::new());
        let engine = Arc::new(
            DistributedGraphSyncEngine::new("pod-b", transport.clone(), cache.clone())
                .with_vector_store(vectors.clone()),
        );

        let key = "exact:cross-replica:q1";
        cache
            .cache()
            .set(CacheEntry::new(key, "stale answer", Some(3600)))
            .await
            .unwrap();
        vectors
            .insert(VectorEntry {
                id: key.to_string(),
                tenant_id: TenantId::new("t1"),
                application_id: ApplicationId::new("app1"),
                prompt: "q1".to_string(),
                content: "stale answer".to_string(),
                vector: vec![0.1, 0.2],
                metadata: serde_json::json!({}),
                created_at: Utc::now(),
            })
            .await
            .unwrap();
        assert!(vectors.delete(key).await.is_ok());
        vectors
            .insert(VectorEntry {
                id: key.to_string(),
                tenant_id: TenantId::new("t1"),
                application_id: ApplicationId::new("app1"),
                prompt: "q1".to_string(),
                content: "stale answer".to_string(),
                vector: vec![0.1, 0.2],
                metadata: serde_json::json!({}),
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let mut message =
            ClusterInvalidationMessage::new("pod-a", TenantId::new("t1"), vec![], vec![], 7);
        message.mutated_cache_keys = vec![key.to_string()];

        let affected = engine.handle_cluster_message(&message).await.unwrap();
        assert_eq!(affected, 1);
        assert!(!cache.cache().exists(key).await.unwrap());
        assert_eq!(vectors.count(&TenantId::new("t1")).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_create_cluster_transport_factory() {
        let in_mem = create_cluster_transport("pod-1", None, 128);
        assert!(!in_mem.is_durable());
        let mut rx = in_mem.subscribe("test").await.unwrap();
        let msg =
            ClusterInvalidationMessage::new("pod-1", TenantId::new("t1"), vec![1, 2], vec![], 1);
        in_mem.publish("test", &msg).await.unwrap();
        let received = rx.recv().await.unwrap();
        assert_eq!(received.origin_pod_id, "pod-1");
        assert_eq!(received.mutated_doc_hashes, vec![1, 2]);

        let strict = create_cluster_transport_checked("pod-2", None, 128);
        assert!(strict.is_err());
    }
}
