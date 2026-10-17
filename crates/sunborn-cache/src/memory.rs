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
}

impl MemoryExactCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_capacity: None,
        }
    }

    pub fn with_capacity(max_capacity: usize) -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            max_capacity: Some(max_capacity),
        }
    }

    pub async fn entry_count(&self) -> usize {
        self.entries.read().await.len()
    }
}

#[async_trait]
impl ExactCacheStore for MemoryExactCache {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
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
        let mut entries = self.entries.write().await;
        Ok(entries.remove(key).is_some())
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let entries = self.entries.read().await;
        if let Some(entry) = entries.get(key) {
            Ok(!entry.is_expired())
        } else {
            Ok(false)
        }
    }

    async fn delete_by_prefix(&self, prefix: &str) -> Result<usize> {
        let mut entries = self.entries.write().await;
        let before = entries.len();
        entries.retain(|k, _| !k.starts_with(prefix));
        Ok(before - entries.len())
    }

    async fn count_by_prefix(&self, prefix: &str) -> Result<usize> {
        let entries = self.entries.read().await;
        Ok(entries.keys().filter(|k| k.starts_with(prefix)).count())
    }

    async fn sweep_expired(&self) -> Result<usize> {
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
}
