use crate::store::{CacheEntry, ExactCacheStore};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use sunborn_core::Result;
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct MemoryExactCache {
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

impl MemoryExactCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
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
}
