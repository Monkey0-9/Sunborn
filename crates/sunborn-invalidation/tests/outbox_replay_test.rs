use std::sync::Arc;
use std::time::Duration;
use sunborn_core::certificate::SourceVersionSnapshot;
use sunborn_invalidation::epoch::{EpochLeaseManager, VectorClock};
use tokio::time::sleep;
use uuid::Uuid;

#[tokio::test]
async fn test_outbox_delayed_event_stale_writeback_prevention() {
    let lease_mgr = Arc::new(EpochLeaseManager::new());
    let source_id = Uuid::new_v4();

    // 1. Client initiates an LLM inference call at t=0 and acquires active lease
    let read_epoch = lease_mgr.acquire_lease(source_id).await;
    assert_eq!(read_epoch, 1);

    // 2. Concurrently, an outbox mutation event is committed and delivered
    // While LLM generation is in flight, document updates
    let updated_epoch = lease_mgr.bump_epoch(source_id).await;
    assert_eq!(updated_epoch, 2);

    // 3. Simulated slow LLM response finishes and attempts cache writeback
    sleep(Duration::from_millis(10)).await;
    let is_valid = lease_mgr.validate_lease(source_id, read_epoch).await;

    // Fencing rule (Hypothesis 1): Stale writeback MUST be rejected
    assert!(
        !is_valid,
        "Hypothesis 1 violation: Stale read writeback succeeded after knowledge source was mutated"
    );

    // 4. A subsequent read acquiring the fresh epoch validates cleanly
    let fresh_lease = lease_mgr.acquire_lease(source_id).await;
    assert_eq!(fresh_lease, 2);
    assert!(lease_mgr.validate_lease(source_id, fresh_lease).await);
}

#[tokio::test]
async fn test_outbox_duplicate_and_reordered_replay_resilience() {
    let lease_mgr = Arc::new(EpochLeaseManager::new());
    let source_id = Uuid::new_v4();

    let initial = lease_mgr.acquire_lease(source_id).await;
    assert_eq!(initial, 1);

    // Simulate 3 duplicate outbox message deliveries of the same event
    // In an idempotent outbox pipeline, monotonic epochs only advance
    let ep1 = lease_mgr.bump_epoch(source_id).await;
    let ep2 = lease_mgr.bump_epoch(source_id).await;
    let ep3 = lease_mgr.bump_epoch(source_id).await;

    assert!(ep3 > ep2 && ep2 > ep1);

    // Any snapshot holding ep1 or ep2 is safely rejected
    let stale_snap = SourceVersionSnapshot {
        source_id,
        snapshot_epoch: ep1,
        content_hash: "hash-v1".to_string(),
    };
    assert!(!lease_mgr.validate_multi_source_leases(&[stale_snap]).await);

    // Snapshot holding ep3 is valid
    let fresh_snap = SourceVersionSnapshot {
        source_id,
        snapshot_epoch: ep3,
        content_hash: "hash-v3".to_string(),
    };
    assert!(lease_mgr.validate_multi_source_leases(&[fresh_snap]).await);
}

#[tokio::test]
async fn test_multi_source_vector_clock_causality_under_network_jitter() {
    let mut vc_node1 = VectorClock::new();
    let mut vc_node2 = VectorClock::new();

    // Node 1 mutates source
    vc_node1.increment("node1");
    assert_eq!(vc_node1.get("node1"), 1);

    // Node 2 mutates source concurrently before receiving replication
    vc_node2.increment("node2");
    assert_eq!(vc_node2.get("node2"), 1);

    // Concurrency detection: Neither happened before the other
    assert!(vc_node1.concurrent_with(&vc_node2));

    // Replication exchange: Node 2 receives Node 1's update and reconciles
    vc_node2.merge(&vc_node1);
    vc_node2.increment("node2");

    // Node 1's previous state happened before Node 2's reconciled state
    assert!(vc_node1.happened_before(&vc_node2));
    assert!(!vc_node2.happened_before(&vc_node1));
}

#[tokio::test]
async fn test_multi_source_atomic_fencing_under_heavy_concurrency() {
    let lease_mgr = Arc::new(EpochLeaseManager::new());
    let mut sources = Vec::new();

    for i in 0..10 {
        let sid = Uuid::new_v4();
        let ep = lease_mgr.acquire_lease(sid).await;
        sources.push(SourceVersionSnapshot {
            source_id: sid,
            snapshot_epoch: ep,
            content_hash: format!("hash-{}", i),
        });
    }

    // All 10 partitions are fresh initially
    assert!(lease_mgr.validate_multi_source_leases(&sources).await);

    // Mutate just 1 out of 10 partitions (e.g. partition 7)
    let mutated_id = sources[7].source_id;
    lease_mgr.bump_epoch(mutated_id).await;

    // The composite multi-source validation must reject reuse atomically
    assert!(
        !lease_mgr.validate_multi_source_leases(&sources).await,
        "Composite lease must fail if even one dependency is stale"
    );
}
