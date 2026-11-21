//! Multi-replica consistency proofs.
//!
//! These tests exercise two replicas sharing one broadcast transport and assert
//! the four properties the cluster invalidation design claims:
//!
//! 1. a mutation on replica A purges the same keys on replica B,
//! 2. the origin suppresses (and acknowledges) its own broadcast echo,
//! 3. replaying the same message twice is idempotent, and
//! 4. a peer that observed a newer epoch rejects a read fenced at an older one.
//!
//! Waits are bounded polls over observable state rather than fixed sleeps, so a
//! slow machine affects duration but never the verdict.

use std::sync::Arc;
use std::time::Duration;
use sunborn_cache::{CacheEntry, ExactCacheStore, MemoryExactCache};
use sunborn_core::TenantId;
use sunborn_invalidation::{
    ClusterInvalidationMessage, CompressedLineageGraph, DependencyGraph,
    DistributedGraphSyncEngine, EpochLeaseManager, GraphMemoryCache, InMemoryClusterTransport,
};
use uuid::Uuid;

const CACHE_KEY: &str = "exact:sha256:enterprise-fintech-q1-earnings";
const TTL_SECS: u64 = 3600;

type Cache = Arc<MemoryExactCache>;
type Graph = Arc<DependencyGraph>;

struct Replica {
    cache: Cache,
    graph: Graph,
    sync: Arc<DistributedGraphSyncEngine>,
    _listener: tokio::task::JoinHandle<()>,
}

fn build_replica(pod_id: &str, transport: Arc<InMemoryClusterTransport>) -> Replica {
    let cache: Cache = Arc::new(MemoryExactCache::new());
    let graph: Graph = Arc::new(DependencyGraph::new());
    let graph_cache = Arc::new(GraphMemoryCache::new(
        cache.clone(),
        graph.clone(),
        Arc::new(CompressedLineageGraph::new()),
    ));
    let sync = Arc::new(DistributedGraphSyncEngine::new(
        pod_id,
        transport,
        graph_cache,
    ));
    let listener = sync
        .clone()
        .start_listener()
        .expect("listener requires a Tokio runtime");
    Replica {
        cache,
        graph,
        sync,
        _listener: listener,
    }
}

/// Poll `condition` until it holds, failing the test after a generous deadline.
/// Used instead of sleeping so the assertions are about state, not timing.
async fn eventually<F, Fut>(label: &str, mut condition: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if condition().await {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for: {label}"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// A broadcast channel has no replay: a message published before a peer
/// subscribed is lost. Retrying a harmless empty invalidation until the peer
/// reports it removes that timing assumption entirely.
async fn await_subscription(
    origin: &DistributedGraphSyncEngine,
    peer: &DistributedGraphSyncEngine,
) {
    let tenant = TenantId::new("subscription-probe");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    while peer.messages_received() == 0 {
        origin
            .publish_and_invalidate(tenant.clone(), vec![], vec![], 1)
            .await
            .expect("warm-up publish");
        assert!(
            tokio::time::Instant::now() < deadline,
            "peer never subscribed to the cluster transport"
        );
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// Populate a replica's exact cache and register the dependency edge that lets
/// source-level invalidation find the key on this replica.
async fn seed(replica: &Replica, source_id: Uuid) {
    replica
        .cache
        .set(CacheEntry::new(
            CACHE_KEY,
            "{\"status\":\"approved\",\"ebitda\":42000000}",
            Some(TTL_SECS),
        ))
        .await
        .expect("seed cache");
    replica
        .graph
        .record_dependency(source_id, CACHE_KEY)
        .await
        .expect("record dependency");
}

#[tokio::test]
async fn test_mutation_on_one_replica_purges_the_peer() {
    let transport = Arc::new(InMemoryClusterTransport::new(1024));
    let a = build_replica("gateway-pod-a", transport.clone());
    let b = build_replica("gateway-pod-b", transport.clone());
    await_subscription(&a.sync, &b.sync).await;

    let source_id = Uuid::new_v4();
    seed(&a, source_id).await;
    seed(&b, source_id).await;
    assert!(a.cache.get(CACHE_KEY).await.unwrap().is_some());
    assert!(b.cache.get(CACHE_KEY).await.unwrap().is_some());

    let tenant = TenantId::new("enterprise-tenant-alpha");
    let affected = a
        .sync
        .publish_and_invalidate(tenant, vec![], vec![source_id], 2)
        .await
        .expect("publish invalidation");
    assert_eq!(affected, 1, "origin must purge the key locally");

    assert!(a.cache.get(CACHE_KEY).await.unwrap().is_none());
    let baseline = b.sync.records_invalidated();
    eventually("peer replica purges the invalidated key", || {
        let cache = b.cache.clone();
        async move { cache.get(CACHE_KEY).await.unwrap().is_none() }
    })
    .await;
    assert_eq!(
        b.sync.records_invalidated(),
        baseline + 1,
        "the peer records exactly one invalidated dependency record"
    );
}

#[tokio::test]
async fn test_origin_suppresses_and_acks_its_own_echo() {
    let transport = Arc::new(InMemoryClusterTransport::new(1024));
    let a = build_replica("gateway-pod-a", transport.clone());
    let b = build_replica("gateway-pod-b", transport.clone());
    await_subscription(&a.sync, &b.sync).await;

    let echoes_before = a.sync.echoes_suppressed();
    let source_id = Uuid::new_v4();
    seed(&a, source_id).await;

    let tenant = TenantId::new("enterprise-tenant-alpha");
    a.sync
        .publish_and_invalidate(tenant, vec![], vec![source_id], 3)
        .await
        .expect("publish invalidation");

    eventually("origin observes its own echo", || {
        let sync = a.sync.clone();
        async move { sync.echoes_suppressed() > echoes_before }
    })
    .await;

    // The echo must not be counted as peer work or re-purge anything.
    assert_eq!(a.sync.records_invalidated(), 0);
    assert!(
        a.cache.get(CACHE_KEY).await.unwrap().is_none(),
        "the origin's own purge happened before publishing, not via the echo"
    );
}

#[tokio::test]
async fn test_duplicate_cluster_delivery_is_idempotent() {
    let transport = Arc::new(InMemoryClusterTransport::new(1024));
    let b = build_replica("gateway-pod-b", transport);
    b.cache
        .set(CacheEntry::new(
            CACHE_KEY,
            "{\"status\":\"approved\"}",
            Some(TTL_SECS),
        ))
        .await
        .expect("seed cache");

    let mut message =
        ClusterInvalidationMessage::new("gateway-pod-a", TenantId::new("t"), vec![], vec![], 4);
    message.mutated_cache_keys = vec![CACHE_KEY.to_string()];

    let first = b.sync.handle_cluster_message(&message).await.unwrap();
    assert_eq!(first, 1, "first delivery purges the key");
    let after_first = b.sync.records_invalidated();

    // Redis consumer-group replay after a crash or unacked entry may deliver the
    // same entry twice. Purges are idempotent, so the second delivery must be a
    // no-op rather than an error or a double count.
    let second = b.sync.handle_cluster_message(&message).await.unwrap();
    assert_eq!(second, 0, "second delivery must purge nothing");
    assert_eq!(
        b.sync.records_invalidated(),
        after_first,
        "duplicate delivery must not double-count invalidations"
    );
    assert!(b.cache.get(CACHE_KEY).await.unwrap().is_none());
}

#[tokio::test]
async fn test_peer_rejects_stale_read_after_observing_newer_epoch() {
    let transport = Arc::new(InMemoryClusterTransport::new(1024));
    let epoch_manager = Arc::new(EpochLeaseManager::new());
    let cache: Cache = Arc::new(MemoryExactCache::new());
    let graph_cache = Arc::new(GraphMemoryCache::new(
        cache.clone(),
        Arc::new(DependencyGraph::new()),
        Arc::new(CompressedLineageGraph::new()),
    ));
    let sync = Arc::new(
        DistributedGraphSyncEngine::new("gateway-pod-b", transport, graph_cache)
            .with_epoch_manager(epoch_manager.clone()),
    );

    let source_id = Uuid::new_v4();
    let message = ClusterInvalidationMessage::new(
        "gateway-pod-a",
        TenantId::new("t"),
        vec![],
        vec![source_id],
        7,
    );
    sync.handle_cluster_message(&message).await.unwrap();

    assert!(
        !epoch_manager.validate_lease(source_id, 6).await,
        "a read fenced at epoch 6 must be rejected after observing epoch 7"
    );
    assert!(
        epoch_manager.validate_lease(source_id, 7).await,
        "a read fenced at the observed epoch must be admitted"
    );
}
