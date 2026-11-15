use std::sync::Arc;
use sunborn_cache::{CacheEntry, ExactCacheStore, MemoryExactCache};
use sunborn_core::TenantId;
use sunborn_invalidation::{
    CompressedLineageGraph, DependencyGraph, DistributedGraphSyncEngine, GraphMemoryCache,
    InMemoryClusterTransport,
};
use uuid::Uuid;

#[tokio::test]
async fn test_multi_replica_invalidation_sync_and_stale_read_rejection() {
    // 1. Setup shared cluster broadcast bus between Replica A and Replica B
    let shared_transport = Arc::new(InMemoryClusterTransport::new(1024));

    // Replica A Components
    let cache_a = Arc::new(MemoryExactCache::new());
    let graph_a = Arc::new(DependencyGraph::new());
    let lineage_a = Arc::new(CompressedLineageGraph::new());
    let gmc_a = Arc::new(GraphMemoryCache::new(
        cache_a.clone(),
        graph_a.clone(),
        lineage_a.clone(),
    ));
    let sync_a = Arc::new(DistributedGraphSyncEngine::new(
        "gateway-pod-a",
        shared_transport.clone(),
        gmc_a.clone(),
    ));
    let _handle_a = sync_a.clone().start_listener();

    // Replica B Components
    let cache_b = Arc::new(MemoryExactCache::new());
    let graph_b = Arc::new(DependencyGraph::new());
    let lineage_b = Arc::new(CompressedLineageGraph::new());
    let gmc_b = Arc::new(GraphMemoryCache::new(
        cache_b.clone(),
        graph_b.clone(),
        lineage_b.clone(),
    ));
    let sync_b = Arc::new(DistributedGraphSyncEngine::new(
        "gateway-pod-b",
        shared_transport.clone(),
        gmc_b.clone(),
    ));
    let _handle_b = sync_b.clone().start_listener();

    // Ensure async listener subscriptions have initialized on both nodes
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    let tenant = TenantId::new("enterprise-tenant-alpha");
    let source_id = Uuid::new_v4();
    let cache_key = "exact:sha256:enterprise-fintech-q1-earnings";

    // 2. Both replicas cache the initial computation
    let entry_a = CacheEntry::new(
        cache_key,
        "{\"status\": \"approved\", \"ebitda\": 42000000}",
        Some(3600),
    );
    let entry_b = CacheEntry::new(
        cache_key,
        "{\"status\": \"approved\", \"ebitda\": 42000000}",
        Some(3600),
    );
    cache_a.set(entry_a).await.unwrap();
    cache_b.set(entry_b).await.unwrap();

    // Register dependency on both
    graph_a
        .record_dependency(source_id, cache_key)
        .await
        .unwrap();
    graph_b
        .record_dependency(source_id, cache_key)
        .await
        .unwrap();

    // Assert cache hits on both nodes before mutation
    assert!(cache_a.get(cache_key).await.unwrap().is_some());
    assert!(cache_b.get(cache_key).await.unwrap().is_some());

    // 3. Mutation event occurs on Replica A: triggers atomic multi-tier purge & cluster broadcast
    let affected = sync_a
        .publish_and_invalidate(tenant.clone(), vec![], vec![source_id], 2)
        .await
        .unwrap();
    assert_eq!(affected, 1);

    // 4. Allow asynchronous propagation across the shared transport bus
    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;

    // 5. Verify: Replica A has purged the key locally
    assert!(
        cache_a.get(cache_key).await.unwrap().is_none(),
        "Replica A must have evicted the key"
    );

    // 6. Verify: Replica B listener consumed message and purged the key!
    assert!(
        cache_b.get(cache_key).await.unwrap().is_none(),
        "Replica B must have received cluster invalidation and evicted the key"
    );
    assert_eq!(sync_b.records_invalidated(), 1);
}
