use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Monotonic Epoch Fencing Leases (Meta TAO / McLease style)
/// Prevents stale cache re-population races where an asynchronous LLM request
/// finishes after an outbox invalidation event has already advanced the lineage version.
#[derive(Debug, Clone)]
pub struct EpochFence {
    pub source_id: Uuid,
    pub read_epoch: u64,
}

#[derive(Clone, Default)]
pub struct EpochLeaseManager {
    epochs: Arc<RwLock<HashMap<Uuid, Arc<AtomicU64>>>>,
}

impl EpochLeaseManager {
    pub fn new() -> Self {
        Self {
            epochs: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Read current monotonic epoch for a knowledge source before querying origin
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

    /// Check if write back to accelerator tiers is safe or if it was invalidated mid-flight
    pub async fn validate_lease(&self, source_id: Uuid, read_epoch: u64) -> bool {
        let map = self.epochs.read().await;
        if let Some(counter) = map.get(&source_id) {
            counter.load(Ordering::SeqCst) == read_epoch
        } else {
            read_epoch == 1
        }
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
}
