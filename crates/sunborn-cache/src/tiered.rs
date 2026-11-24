use crate::memory::MemoryExactCache;
use crate::store::{CacheEntry, ExactCacheStore};
use async_trait::async_trait;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use sunborn_core::Result;

/// Two-Tiered Cache Architecture:
/// L1: Ultra-fast In-Process Cache (sub-50 microsecond latency for ultra-hot entries)
/// L2: Distributed Accelerator / Network Cache (with Epoch Fencing and larger capacity)
#[derive(Clone)]
pub struct TieredExactCache<L2: ExactCacheStore> {
    l1: Arc<MemoryExactCache>,
    l2: Arc<L2>,
    cluster_epoch: Arc<AtomicU64>,
    l1_hits: Arc<AtomicU64>,
    l2_hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
}

impl<L2: ExactCacheStore> TieredExactCache<L2> {
    pub fn new(l1_capacity: usize, l2: L2) -> Self {
        Self {
            l1: Arc::new(MemoryExactCache::with_capacity(l1_capacity)),
            l2: Arc::new(l2),
            cluster_epoch: Arc::new(AtomicU64::new(0)),
            l1_hits: Arc::new(AtomicU64::new(0)),
            l2_hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn with_stores(l1: Arc<MemoryExactCache>, l2: Arc<L2>) -> Self {
        Self {
            l1,
            l2,
            cluster_epoch: Arc::new(AtomicU64::new(0)),
            l1_hits: Arc::new(AtomicU64::new(0)),
            l2_hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn cluster_epoch(&self) -> u64 {
        self.cluster_epoch.load(Ordering::Acquire)
    }

    pub fn set_cluster_epoch(&self, epoch: u64) {
        self.cluster_epoch.store(epoch, Ordering::Release);
    }

    pub fn advance_cluster_epoch(&self) -> u64 {
        self.cluster_epoch.fetch_add(1, Ordering::SeqCst) + 1
    }

    pub fn stats(&self) -> (u64, u64, u64) {
        (
            self.l1_hits.load(Ordering::Relaxed),
            self.l2_hits.load(Ordering::Relaxed),
            self.misses.load(Ordering::Relaxed),
        )
    }

    pub fn l1(&self) -> &Arc<MemoryExactCache> {
        &self.l1
    }

    pub fn l2(&self) -> &Arc<L2> {
        &self.l2
    }
}

#[async_trait]
impl<L2: ExactCacheStore + Send + Sync> ExactCacheStore for TieredExactCache<L2> {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
        let current_epoch = self.cluster_epoch.load(Ordering::Acquire);

        // 1. Check L1 In-Process Cache with monotonic epoch verification
        match self.l1.get(key).await {
            Ok(Some(entry)) => {
                let entry_epoch = entry
                    .metadata
                    .get("epoch")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if entry_epoch < current_epoch {
                    tracing::warn!(
                        key = %key,
                        entry_epoch = entry_epoch,
                        current_epoch = current_epoch,
                        "TieredExactCache: L1 hit has stale cluster epoch; evicting from L1 and falling back to L2"
                    );
                    let _ = self.l1.delete(key).await;
                } else {
                    self.l1_hits.fetch_add(1, Ordering::Relaxed);
                    return Ok(Some(entry));
                }
            }
            Ok(None) => {}
            Err(err) => {
                tracing::warn!(key = %key, error = %err, "TieredExactCache: L1 get returned error; falling back to L2");
            }
        }

        // 2. Check L2 Store
        match self.l2.get(key).await {
            Ok(Some(mut entry)) => {
                let entry_epoch = entry
                    .metadata
                    .get("epoch")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                if entry_epoch < current_epoch {
                    tracing::warn!(
                        key = %key,
                        entry_epoch = entry_epoch,
                        current_epoch = current_epoch,
                        "TieredExactCache: L2 hit has stale cluster epoch; treating as cache miss"
                    );
                    self.misses.fetch_add(1, Ordering::Relaxed);
                    return Ok(None);
                }

                self.l2_hits.fetch_add(1, Ordering::Relaxed);
                // Ensure L1 entry metadata is current before populating L1
                if let Some(obj) = entry.metadata.as_object_mut() {
                    obj.insert("epoch".to_string(), serde_json::json!(current_epoch));
                }
                // Read-repair / populate L1 in background
                if let Err(err) = self.l1.set(entry.clone()).await {
                    tracing::warn!(key = %key, error = %err, "TieredExactCache: L1 populate failed");
                }
                Ok(Some(entry))
            }
            Ok(None) => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                Ok(None)
            }
            Err(err) => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                Err(err)
            }
        }
    }

    async fn set(&self, mut entry: CacheEntry) -> Result<()> {
        let current_epoch = self.cluster_epoch.load(Ordering::Acquire);
        if let Some(obj) = entry.metadata.as_object_mut() {
            obj.insert("epoch".to_string(), serde_json::json!(current_epoch));
        } else {
            entry.metadata = serde_json::json!({ "epoch": current_epoch });
        }

        // Write-through: Update L2 first (authoritative distributed store), then populate L1
        self.l2.set(entry.clone()).await?;
        if let Err(err) = self.l1.set(entry).await {
            tracing::warn!(error = %err, "TieredExactCache: L1 write-through failed");
            if sunborn_core::is_production_mode() {
                return Err(err);
            }
        }
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        self.cluster_epoch.fetch_add(1, Ordering::SeqCst);
        let l1_res = self.l1.delete(key).await;
        if let Err(ref err) = l1_res {
            tracing::error!(key = %key, error = %err, "TieredExactCache: L1 delete failed");
        }
        let l2_res = self.l2.delete(key).await;
        if sunborn_core::is_production_mode() {
            if let Err(err) = l1_res {
                return Err(err);
            }
        }
        let l2_deleted = l2_res?;
        let l1_deleted = l1_res.unwrap_or(false);
        Ok(l1_deleted || l2_deleted)
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        if let Ok(true) = self.l1.exists(key).await {
            return Ok(true);
        }
        self.l2.exists(key).await
    }

    async fn delete_by_prefix(&self, prefix: &str) -> Result<usize> {
        self.cluster_epoch.fetch_add(1, Ordering::SeqCst);
        let l1_res = self.l1.delete_by_prefix(prefix).await;
        if let Err(ref err) = l1_res {
            tracing::error!(prefix = %prefix, error = %err, "TieredExactCache: L1 delete_by_prefix failed");
        }
        let l2_res = self.l2.delete_by_prefix(prefix).await;
        if sunborn_core::is_production_mode() {
            if let Err(err) = l1_res {
                return Err(err);
            }
        }
        let l2_count = l2_res?;
        let l1_count = l1_res.unwrap_or(0);
        Ok(l1_count.max(l2_count))
    }

    async fn count_by_prefix(&self, prefix: &str) -> Result<usize> {
        let l1_count = self.l1.count_by_prefix(prefix).await.unwrap_or(0);
        let l2_count = self.l2.count_by_prefix(prefix).await.unwrap_or(0);
        Ok(l1_count.max(l2_count))
    }

    async fn sweep_expired(&self) -> Result<usize> {
        let l1_swept = self.l1.sweep_expired().await.unwrap_or(0);
        let l2_swept = self.l2.sweep_expired().await.unwrap_or(0);
        Ok(l1_swept + l2_swept)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_tiered_l1_l2_cache_read_through() {
        let l2_store = MemoryExactCache::new();
        let tiered = TieredExactCache::new(100, l2_store);

        let entry = CacheEntry::new("hot_key_1", "hot_payload", Some(60));
        tiered.set(entry).await.unwrap();

        // First read: should hit L1
        let res = tiered.get("hot_key_1").await.unwrap();
        assert!(res.is_some());
        assert_eq!(res.unwrap().content, "hot_payload");

        let (l1_hits, _, _) = tiered.stats();
        assert_eq!(l1_hits, 1);

        // Delete from L1 only to simulate cold L1
        tiered.l1().delete("hot_key_1").await.unwrap();

        // Next read: misses L1, hits L2, and repopulates L1
        let res2 = tiered.get("hot_key_1").await.unwrap();
        assert!(res2.is_some());
        let (l1_hits_after, l2_hits, _) = tiered.stats();
        assert_eq!(l1_hits_after, 1);
        assert_eq!(l2_hits, 1);

        // Third read: hits L1 again because L1 was populated
        let _ = tiered.get("hot_key_1").await.unwrap();
        let (l1_hits_final, _, _) = tiered.stats();
        assert_eq!(l1_hits_final, 2);
    }

    #[tokio::test]
    async fn test_tiered_l1_monotonic_epoch_invalidation_prevents_stale_reads() {
        let l2_store = MemoryExactCache::new();
        let tiered = TieredExactCache::new(100, l2_store);

        let entry = CacheEntry::new("epoch_key_1", "initial_val", Some(60));
        tiered.set(entry).await.unwrap();

        // Reading directly hits L1
        let val1 = tiered.get("epoch_key_1").await.unwrap();
        assert!(val1.is_some());
        assert_eq!(val1.unwrap().content, "initial_val");
        let (l1_hits, _, _) = tiered.stats();
        assert_eq!(l1_hits, 1);

        // Advance cluster epoch (e.g. from an out-of-band invalidation or peer node broadcast)
        tiered.set_cluster_epoch(5);

        // Stale read: L1 has epoch 0 < 5. Must evict from L1 and check L2.
        // L2 also has epoch 0 < 5, so it returns None as stale miss!
        let val2 = tiered.get("epoch_key_1").await.unwrap();
        assert!(val2.is_none());

        // L1 was evicted on stale check, verified by inspecting L1 directly
        assert!(tiered.l1().get("epoch_key_1").await.unwrap().is_none());

        // Set fresh value at epoch 5
        let entry2 = CacheEntry::new("epoch_key_1", "updated_val", Some(60));
        tiered.set(entry2).await.unwrap();

        let val3 = tiered.get("epoch_key_1").await.unwrap();
        assert!(val3.is_some());
        assert_eq!(val3.unwrap().content, "updated_val");
    }
}
