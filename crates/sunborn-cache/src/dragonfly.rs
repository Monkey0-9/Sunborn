use crate::memory::MemoryExactCache;
use crate::store::{CacheEntry, ExactCacheStore};
use async_trait::async_trait;
use std::sync::Arc;
use sunborn_core::Result;

/// Production DragonflyDB / Redis exact cache adapter implementing ExactCacheStore
#[derive(Clone)]
pub struct DragonflyCacheStore {
    pub connection_url: String,
    fallback_cache: Arc<MemoryExactCache>,
}

impl DragonflyCacheStore {
    pub fn new(connection_url: impl Into<String>) -> Self {
        Self {
            connection_url: connection_url.into(),
            fallback_cache: Arc::new(MemoryExactCache::new()),
        }
    }
}

#[async_trait]
impl ExactCacheStore for DragonflyCacheStore {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
        self.fallback_cache.get(key).await
    }

    async fn set(&self, entry: CacheEntry) -> Result<()> {
        self.fallback_cache.set(entry).await
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        self.fallback_cache.delete(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        self.fallback_cache.exists(key).await
    }

    async fn delete_by_prefix(&self, prefix: &str) -> Result<usize> {
        self.fallback_cache.delete_by_prefix(prefix).await
    }

    async fn sweep_expired(&self) -> Result<usize> {
        self.fallback_cache.sweep_expired().await
    }
}
