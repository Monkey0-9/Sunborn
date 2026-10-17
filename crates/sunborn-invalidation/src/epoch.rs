use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use sunborn_core::certificate::SourceVersionSnapshot;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Monotonic Epoch Fencing Leases (Meta TAO / McLease style)
/// Prevents stale cache re-population races where an asynchronous LLM request
/// finishes after an outbox invalidation event has already advanced the lineage version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EpochFence {
    pub source_id: Uuid,
    pub read_epoch: u64,
}

/// Logical Vector Clock for multi-source causality tracking.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct VectorClock {
    pub versions: HashMap<String, u64>,
}

impl VectorClock {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, node: &str) -> u64 {
        self.versions.get(node).copied().unwrap_or(0)
    }

    pub fn increment(&mut self, node: &str) -> u64 {
        let v = self.versions.entry(node.to_string()).or_insert(0);
        *v += 1;
        *v
    }

    pub fn merge(&mut self, other: &VectorClock) {
        for (node, &ver) in &other.versions {
            let entry = self.versions.entry(node.clone()).or_insert(0);
            if ver > *entry {
                *entry = ver;
            }
        }
    }

    /// Returns true if self strictly happened-before other (self < other)
    pub fn happened_before(&self, other: &VectorClock) -> bool {
        let mut strictly_less = false;
        for (node, &v1) in &self.versions {
            let v2 = other.get(node);
            if v1 > v2 {
                return false;
            }
            if v1 < v2 {
                strictly_less = true;
            }
        }
        for (node, &v2) in &other.versions {
            if !self.versions.contains_key(node) && v2 > 0 {
                strictly_less = true;
            }
        }
        strictly_less
    }

    /// Returns true if two vector clocks are concurrent (neither dominates the other)
    pub fn concurrent_with(&self, other: &VectorClock) -> bool {
        !self.happened_before(other) && !other.happened_before(self) && self != other
    }
}

/// Thread-safe Epoch and Causality Lease Manager supporting multi-source validation.
#[derive(Clone, Default)]
pub struct EpochLeaseManager {
    epochs: Arc<RwLock<HashMap<Uuid, Arc<AtomicU64>>>>,
    vector_clocks: Arc<RwLock<HashMap<String, VectorClock>>>,
}

impl EpochLeaseManager {
    pub fn new() -> Self {
        Self {
            epochs: Arc::new(RwLock::new(HashMap::new())),
            vector_clocks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Read current monotonic epoch for a knowledge source
    pub async fn acquire_lease(&self, source_id: Uuid) -> u64 {
        let mut map = self.epochs.write().await;
        let counter = map
            .entry(source_id)
            .or_insert_with(|| Arc::new(AtomicU64::new(1)));
        counter.load(Ordering::SeqCst)
    }

    /// Advance epoch upon knowledge update / document invalidation
    pub async fn bump_epoch(&self, source_id: Uuid) -> u64 {
        let mut map = self.epochs.write().await;
        let counter = map
            .entry(source_id)
            .or_insert_with(|| Arc::new(AtomicU64::new(1)));
        counter.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Check if write back is safe for a single source
    pub async fn validate_lease(&self, source_id: Uuid, read_epoch: u64) -> bool {
        let map = self.epochs.read().await;
        if let Some(counter) = map.get(&source_id) {
            counter.load(Ordering::SeqCst) == read_epoch
        } else {
            read_epoch == 1
        }
    }

    /// Multi-source atomic lease validation (Meta TAO / McLease multi-key fence)
    /// Validates all dependency source snapshots against current active monotonic epochs.
    /// If ANY dependency has been bumped or modified, the lease is rejected immediately.
    pub async fn validate_multi_source_leases(&self, snapshots: &[SourceVersionSnapshot]) -> bool {
        if snapshots.is_empty() {
            return true;
        }
        let map = self.epochs.read().await;
        for snap in snapshots {
            let current = if let Some(counter) = map.get(&snap.source_id) {
                counter.load(Ordering::SeqCst)
            } else {
                1
            };
            if current != snap.snapshot_epoch {
                return false;
            }
        }
        true
    }

    /// Record or advance the vector clock for a source
    pub async fn update_vector_clock(&self, source_id: &str, node: &str) -> VectorClock {
        let mut map = self.vector_clocks.write().await;
        let clock = map.entry(source_id.to_string()).or_default();
        clock.increment(node);
        clock.clone()
    }

    /// Get current vector clock for a source
    pub async fn get_vector_clock(&self, source_id: &str) -> VectorClock {
        let map = self.vector_clocks.read().await;
        map.get(source_id).cloned().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_epoch_fence_prevents_stale_cache_writeback() {
        let manager = EpochLeaseManager::new();
        let doc_id = Uuid::new_v4();

        // 1. Request starts at t=10ms and acquires lease (epoch=1)
        let lease_epoch = manager.acquire_lease(doc_id).await;
        assert_eq!(lease_epoch, 1);

        // 2. Document mutation arrives at t=50ms and bumps epoch (epoch=2)
        let new_epoch = manager.bump_epoch(doc_id).await;
        assert_eq!(new_epoch, 2);

        // 3. Stale LLM response finishes at t=200ms and attempts writeback
        let is_valid = manager.validate_lease(doc_id, lease_epoch).await;
        assert!(
            !is_valid,
            "Stale writeback with old epoch must be rejected by fence"
        );

        // 4. Fresh request with latest epoch succeeds
        let fresh_epoch = manager.acquire_lease(doc_id).await;
        assert!(manager.validate_lease(doc_id, fresh_epoch).await);
    }

    #[tokio::test]
    async fn test_multi_source_atomic_lease_validation() {
        let manager = EpochLeaseManager::new();

        let s1 = Uuid::new_v4();
        let s2 = Uuid::new_v4();
        let s3 = Uuid::new_v4();

        let e1 = manager.acquire_lease(s1).await;
        let e2 = manager.acquire_lease(s2).await;
        let e3 = manager.acquire_lease(s3).await;

        let snapshots = vec![
            SourceVersionSnapshot {
                source_id: s1,
                snapshot_epoch: e1,
                content_hash: "hash1".to_string(),
            },
            SourceVersionSnapshot {
                source_id: s2,
                snapshot_epoch: e2,
                content_hash: "hash2".to_string(),
            },
            SourceVersionSnapshot {
                source_id: s3,
                snapshot_epoch: e3,
                content_hash: "hash3".to_string(),
            },
        ];

        // All 3 sources at active epoch -> atomic validation passes
        assert!(manager.validate_multi_source_leases(&snapshots).await);

        // Invalidate single source s2 (mid-flight mutation)
        manager.bump_epoch(s2).await;

        // Multi-source validation must reject reuse immediately
        assert!(!manager.validate_multi_source_leases(&snapshots).await);
    }

    #[test]
    fn test_vector_clock_causality_and_concurrency() {
        let mut vc_a = VectorClock::new();
        let mut vc_b = VectorClock::new();

        vc_a.increment("node-1");
        assert_eq!(vc_a.get("node-1"), 1);

        // vc_a happened before vc_b after vc_b merges and increments
        vc_b.merge(&vc_a);
        vc_b.increment("node-2");

        assert!(vc_a.happened_before(&vc_b));
        assert!(!vc_b.happened_before(&vc_a));
        assert!(!vc_a.concurrent_with(&vc_b));

        // Concurrent scenario: vc_a increments node-1 independently
        vc_a.increment("node-1");
        assert!(vc_a.concurrent_with(&vc_b));
    }
}
