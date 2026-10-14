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
    l1_hits: Arc<AtomicU64>,
    l2_hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
}

impl<L2: ExactCacheStore> TieredExactCache<L2> {
    pub fn new(l1_capacity: usize, l2: L2) -> Self {
        Self {
            l1: Arc::new(MemoryExactCache::with_capacity(l1_capacity)),
            l2: Arc::new(l2),
            l1_hits: Arc::new(AtomicU64::new(0)),
            l2_hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
        }
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
        // 1. Check L1 In-Process Cache
        if let Ok(Some(entry)) = self.l1.get(key).await {
            self.l1_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(Some(entry));
        }

        // 2. Check L2 Store
        match self.l2.get(key).await {
            Ok(Some(entry)) => {
                self.l2_hits.fetch_add(1, Ordering::Relaxed);
                // Read-repair / populate L1 in background
                let _ = self.l1.set(entry.clone()).await;
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

    async fn set(&self, entry: CacheEntry) -> Result<()> {
        // Write-through: Update L2, then populate L1
        self.l2.set(entry.clone()).await?;
        let _ = self.l1.set(entry).await;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        let _ = self.l1.delete(key).await;
        self.l2.delete(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        if self.l1.exists(key).await.unwrap_or(false) {
            return Ok(true);
        }
        self.l2.exists(key).await
    }

    async fn delete_by_prefix(&self, prefix: &str) -> Result<usize> {
        let _ = self.l1.delete_by_prefix(prefix).await;
        self.l2.delete_by_prefix(prefix).await
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
}
