use crate::gdsf::FinOpsGdsfCache;
use crate::store::{CacheEntry, ExactCacheStore};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use sunborn_core::Result;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct MemoryExactCache {
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
    max_capacity: Option<usize>,
    gdsf: Arc<RwLock<Option<FinOpsGdsfCache<CacheEntry>>>>,
}

impl MemoryExactCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_capacity: None,
            gdsf: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_capacity(max_capacity: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_capacity: Some(max_capacity),
            gdsf: Arc::new(RwLock::new(None)),
        }
    }

    pub fn with_byte_budget(max_bytes: usize, latency_weight_gamma: f64) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_capacity: None,
            gdsf: Arc::new(RwLock::new(Some(FinOpsGdsfCache::new(
                max_bytes,
                latency_weight_gamma,
            )))),
        }
    }

    pub async fn entry_count(&self) -> usize {
        let gdsf_guard = self.gdsf.read().await;
        if let Some(ref gdsf) = *gdsf_guard {
            gdsf.len()
        } else {
            self.entries.read().await.len()
        }
    }

    pub async fn current_bytes(&self) -> usize {
        let gdsf_guard = self.gdsf.read().await;
        if let Some(ref gdsf) = *gdsf_guard {
            gdsf.current_bytes()
        } else {
            self.entries
                .read()
                .await
                .values()
                .map(|e| e.key.len() + e.content.len() + 128)
                .sum()
        }
    }
}

#[async_trait]
impl ExactCacheStore for MemoryExactCache {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
        let mut gdsf_guard = self.gdsf.write().await;
        if let Some(ref mut gdsf) = *gdsf_guard {
            let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
            return Ok(gdsf.get(key, now_ms));
        }

        let mut entries = self.entries.write().await;
        if let Some(entry) = entries.get_mut(key) {
            if entry.is_expired() {
                entries.remove(key);
                Ok(None)
            } else {
                entry.hit_count += 1;
                Ok(Some(entry.clone()))
            }
        } else {
            Ok(None)
        }
    }

    async fn set(&self, entry: CacheEntry) -> Result<()> {
        let mut gdsf_guard = self.gdsf.write().await;
        if let Some(ref mut gdsf) = *gdsf_guard {
            let size_bytes = entry.key.len() + entry.content.len() + 128;
            let cost = entry
                .metadata
                .get("cost_micro_usd")
                .and_then(|v| v.as_u64())
                .unwrap_or(1000);
            let latency = entry
                .metadata
                .get("latency_ms")
                .and_then(|v| v.as_u64())
                .unwrap_or(200);
            let expires_at_ms = entry
                .expires_at
                .map(|exp| exp.timestamp_millis().max(0) as u64)
                .unwrap_or(u64::MAX);

            gdsf.insert(
                entry.key.clone(),
                entry,
                size_bytes,
                cost,
                latency,
                expires_at_ms,
            );
            return Ok(());
        }

        let mut entries = self.entries.write().await;

        if let Some(cap) = self.max_capacity {
            if entries.len() >= cap && !entries.contains_key(&entry.key) {
                // 1. Evict expired entries
                entries.retain(|_, v| !v.is_expired());

                // 2. If still at capacity, evict oldest entry
                if entries.len() >= cap {
                    if let Some(oldest_key) = entries
                        .iter()
                        .min_by_key(|(_, v)| v.created_at)
                        .map(|(k, _)| k.clone())
                    {
                        entries.remove(&oldest_key);
                    }
                }
            }
        }

        entries.insert(entry.key.clone(), entry);
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        let mut gdsf_guard = self.gdsf.write().await;
        if let Some(ref mut gdsf) = *gdsf_guard {
            return Ok(gdsf.remove(key).is_some());
        }

        let mut entries = self.entries.write().await;
        Ok(entries.remove(key).is_some())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let mut gdsf_guard = self.gdsf.write().await;
        if let Some(ref mut gdsf) = *gdsf_guard {
            let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
            return Ok(gdsf.get(key, now_ms).is_some());
        }

        let entries = self.entries.read().await;
        if let Some(entry) = entries.get(key) {
            Ok(!entry.is_expired())
        } else {
            Ok(false)
        }
    }

    async fn delete_by_prefix(&self, prefix: &str) -> Result<usize> {
        let mut gdsf_guard = self.gdsf.write().await;
        if let Some(ref mut gdsf) = *gdsf_guard {
            let matching_keys: Vec<String> = gdsf
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect();
            let count = matching_keys.len();
            for k in matching_keys {
                gdsf.remove(&k);
            }
            return Ok(count);
        }

        let mut entries = self.entries.write().await;
        let before = entries.len();
        entries.retain(|k, _| !k.starts_with(prefix));
        Ok(before - entries.len())
    }

    async fn count_by_prefix(&self, prefix: &str) -> Result<usize> {
        let gdsf_guard = self.gdsf.read().await;
        if let Some(ref gdsf) = *gdsf_guard {
            return Ok(gdsf.keys().filter(|k| k.starts_with(prefix)).count());
        }

        let entries = self.entries.read().await;
        Ok(entries.keys().filter(|k| k.starts_with(prefix)).count())
    }

    async fn sweep_expired(&self) -> Result<usize> {
        let mut gdsf_guard = self.gdsf.write().await;
        if let Some(ref mut gdsf) = *gdsf_guard {
            let now_ms = chrono::Utc::now().timestamp_millis().max(0) as u64;
            let all_keys: Vec<String> = gdsf.keys().cloned().collect();
            let mut expired_keys = Vec::new();
            for k in all_keys {
                if gdsf.get(&k, now_ms).is_none() {
                    expired_keys.push(k);
                }
            }
            let count = expired_keys.len();
            for k in expired_keys {
                gdsf.remove(&k);
            }
            return Ok(count);
        }

        let mut entries = self.entries.write().await;
        let before = entries.len();
        entries.retain(|_, v| !v.is_expired());
        Ok(before - entries.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_cache_get_set() {
        let cache = MemoryExactCache::new();
        let entry = CacheEntry::new("test:key", "cached content", Some(60));

        cache.set(entry).await.unwrap();
        assert!(cache.exists("test:key").await.unwrap());

        let retrieved = cache.get("test:key").await.unwrap();
        assert!(retrieved.is_some());
        let res = retrieved.unwrap();
        assert_eq!(res.content, "cached content");
        assert_eq!(res.hit_count, 1);
    }

    #[tokio::test]
    async fn test_memory_cache_bounded_capacity() {
        let cache = MemoryExactCache::with_capacity(3);
        cache
            .set(CacheEntry::new("k1", "v1", Some(60)))
            .await
            .unwrap();
        cache
            .set(CacheEntry::new("k2", "v2", Some(60)))
            .await
            .unwrap();
        cache
            .set(CacheEntry::new("k3", "v3", Some(60)))
            .await
            .unwrap();
        assert_eq!(cache.entry_count().await, 3);

        // Ingesting 4th entry should evict oldest
        cache
            .set(CacheEntry::new("k4", "v4", Some(60)))
            .await
            .unwrap();
        assert_eq!(cache.entry_count().await, 3);
        assert!(!cache.exists("k1").await.unwrap());
        assert!(cache.exists("k4").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_cache_delete_by_prefix() {
        let cache = MemoryExactCache::new();
        cache
            .set(CacheEntry::new("tenant-a:1", "v1", Some(60)))
            .await
            .unwrap();
        cache
            .set(CacheEntry::new("tenant-a:2", "v2", Some(60)))
            .await
            .unwrap();
        cache
            .set(CacheEntry::new("tenant-b:1", "v3", Some(60)))
            .await
            .unwrap();

        let count = cache.delete_by_prefix("tenant-a:").await.unwrap();
        assert_eq!(count, 2);
        assert!(!cache.exists("tenant-a:1").await.unwrap());
        assert!(!cache.exists("tenant-a:2").await.unwrap());
        assert!(cache.exists("tenant-b:1").await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_cache_finops_gdsf_byte_bounded() {
        // 2 KB budget
        let cache = MemoryExactCache::with_byte_budget(2048, 0.5);

        let mut e1 = CacheEntry::new("k1", "x".repeat(600), Some(60));
        e1.metadata = serde_json::json!({ "cost_micro_usd": 100, "latency_ms": 50 });
        cache.set(e1).await.unwrap();

        let mut e2 = CacheEntry::new("k2", "y".repeat(600), Some(60));
        e2.metadata = serde_json::json!({ "cost_micro_usd": 50000, "latency_ms": 3000 });
        cache.set(e2).await.unwrap();

        assert_eq!(cache.entry_count().await, 2);
        assert!(cache.current_bytes().await > 1200);

        // Third entry will trigger eviction of lower-utility k1
        let mut e3 = CacheEntry::new("k3", "z".repeat(700), Some(60));
        e3.metadata = serde_json::json!({ "cost_micro_usd": 20000, "latency_ms": 1000 });
        cache.set(e3).await.unwrap();

        assert!(
            !cache.exists("k1").await.unwrap(),
            "Low utility k1 should be evicted"
        );
        assert!(
            cache.exists("k2").await.unwrap(),
            "High utility k2 should remain"
        );
        assert!(
            cache.exists("k3").await.unwrap(),
            "New entry k3 should be admitted"
        );
    }
}
