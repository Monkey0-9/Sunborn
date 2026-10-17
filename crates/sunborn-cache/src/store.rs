use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sunborn_core::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub hit_count: u64,
    pub metadata: serde_json::Value,
}

impl CacheEntry {
    pub fn new(key: impl Into<String>, content: impl Into<String>, ttl_secs: Option<u64>) -> Self {
        let now = Utc::now();
        let expires_at = ttl_secs.map(|s| now + chrono::Duration::seconds(s as i64));
        Self {
            key: key.into(),
            content: content.into(),
            created_at: now,
            expires_at,
            hit_count: 0,
            metadata: serde_json::json!({}),
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(exp) = self.expires_at {
            Utc::now() > exp
        } else {
            false
        }
    }
}

#[async_trait]
pub trait ExactCacheStore: Send + Sync {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>>;
    async fn set(&self, entry: CacheEntry) -> Result<()>;
    async fn delete(&self, key: &str) -> Result<bool>;
    async fn exists(&self, key: &str) -> Result<bool>;
    async fn delete_by_prefix(&self, prefix: &str) -> Result<usize>;
    async fn count_by_prefix(&self, prefix: &str) -> Result<usize>;
    async fn sweep_expired(&self) -> Result<usize>;
}

#[async_trait]
impl ExactCacheStore for std::sync::Arc<dyn ExactCacheStore> {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
        (**self).get(key).await
    }

    async fn set(&self, entry: CacheEntry) -> Result<()> {
        (**self).set(entry).await
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        (**self).delete(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        (**self).exists(key).await
    }

    async fn delete_by_prefix(&self, prefix: &str) -> Result<usize> {
        (**self).delete_by_prefix(prefix).await
    }

    async fn count_by_prefix(&self, prefix: &str) -> Result<usize> {
        (**self).count_by_prefix(prefix).await
    }

    async fn sweep_expired(&self) -> Result<usize> {
        (**self).sweep_expired().await
    }
}
