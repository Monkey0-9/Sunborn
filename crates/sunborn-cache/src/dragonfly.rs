use crate::memory::MemoryExactCache;
use crate::store::{CacheEntry, ExactCacheStore};
use async_trait::async_trait;
use redis::AsyncCommands;
use std::sync::Arc;
use sunborn_core::{Result, SunbornError};
use tokio::sync::RwLock;

/// Production DragonflyDB / Redis exact cache adapter implementing ExactCacheStore
#[derive(Clone)]
pub struct DragonflyCacheStore {
    pub connection_url: String,
    client: Option<redis::Client>,
    connection_manager: Arc<RwLock<Option<redis::aio::ConnectionManager>>>,
    fallback_cache: Arc<MemoryExactCache>,
    pub strict_mode: bool,
}

impl DragonflyCacheStore {
    pub fn new(connection_url: impl Into<String>) -> Self {
        let url_str = connection_url.into();
        let client = redis::Client::open(url_str.as_str()).ok();
        let strict = match sunborn_core::config::parse_bool_env("SUNBORN_CACHE_STRICT", false) {
            Ok(v) => {
                v || std::env::var("SUNBORN_CACHE_BACKEND")
                    .map(|b| b.to_lowercase() == "dragonfly")
                    .unwrap_or(false)
            }
            Err(_) => true, // Invalid config triggers fail-closed strict mode
        };

        Self {
            connection_url: url_str,
            client,
            connection_manager: Arc::new(RwLock::new(None)),
            fallback_cache: Arc::new(MemoryExactCache::new()),
            strict_mode: strict,
        }
    }

    pub fn new_strict(connection_url: impl Into<String>) -> Self {
        let url_str = connection_url.into();
        let client = redis::Client::open(url_str.as_str()).ok();
        Self {
            connection_url: url_str,
            client,
            connection_manager: Arc::new(RwLock::new(None)),
            fallback_cache: Arc::new(MemoryExactCache::new()),
            strict_mode: true,
        }
    }

    /// Actively validates DragonflyDB connection health via Redis PING command.
    pub async fn verify_connection(&self) -> Result<()> {
        let ping_fut = async {
            if let Some(mut conn) = self.get_connection().await {
                let res: redis::RedisResult<String> =
                    redis::cmd("PING").query_async(&mut conn).await;
                match res {
                    Ok(_) => Ok(()),
                    Err(err) => Err(SunbornError::cache_unavailable(format!(
                        "DragonflyDB ping health check failed at {}: {err}",
                        self.connection_url
                    ))),
                }
            } else {
                Err(SunbornError::cache_unavailable(format!(
                    "Unable to establish connection to DragonflyDB at {}",
                    self.connection_url
                )))
            }
        };

        match tokio::time::timeout(std::time::Duration::from_millis(1500), ping_fut).await {
            Ok(res) => res,
            Err(_) => Err(SunbornError::cache_unavailable(format!(
                "DragonflyDB connection timed out at {}",
                self.connection_url
            ))),
        }
    }

    async fn get_connection(&self) -> Option<redis::aio::ConnectionManager> {
        {
            let guard = self.connection_manager.read().await;
            if let Some(conn) = guard.as_ref() {
                return Some(conn.clone());
            }
        }

        if let Some(client) = &self.client {
            if let Ok(conn) = redis::aio::ConnectionManager::new(client.clone()).await {
                let mut guard = self.connection_manager.write().await;
                *guard = Some(conn.clone());
                return Some(conn);
            }
        }

        None
    }
}

#[async_trait]
impl ExactCacheStore for DragonflyCacheStore {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
        if let Some(mut conn) = self.get_connection().await {
            match conn.get::<_, Option<String>>(key).await {
                Ok(Some(raw_json)) => {
                    if let Ok(entry) = serde_json::from_str::<CacheEntry>(&raw_json) {
                        if !entry.is_expired() {
                            return Ok(Some(entry));
                        } else {
                            let _ = conn.del::<_, ()>(key).await;
                            return Ok(None);
                        }
                    }
                }
                Ok(None) => return Ok(None),
                Err(err) => {
                    tracing::warn!(error = %err, "DragonflyDB get command failed; degrading to origin miss");
                    if self.strict_mode {
                        return Ok(None); // Clean cache miss degradation; do not read split-brain shadow
                    }
                }
            }
        } else if self.strict_mode {
            tracing::warn!("DragonflyDB connection unavailable; degrading to origin miss");
            return Ok(None);
        }

        self.fallback_cache.get(key).await
    }

    async fn set(&self, entry: CacheEntry) -> Result<()> {
        let serialized = serde_json::to_string(&entry).unwrap_or_default();
        let key = entry.key.clone();

        if let Some(mut conn) = self.get_connection().await {
            let res: redis::RedisResult<()> = if let Some(exp) = entry.expires_at {
                let now = chrono::Utc::now();
                let remaining_secs = (exp - now).num_seconds().max(1) as u64;
                conn.set_ex(&key, serialized.clone(), remaining_secs).await
            } else {
                conn.set(&key, serialized.clone()).await
            };

            match res {
                Ok(_) => {
                    let _ = self.fallback_cache.set(entry).await;
                    return Ok(());
                }
                Err(err) => {
                    tracing::error!(error = %err, "DragonflyDB set command failed");
                    if self.strict_mode {
                        return Err(sunborn_core::SunbornError::cache(format!(
                            "DragonflyDB set failed: {}",
                            err
                        )));
                    }
                }
            }
        } else if self.strict_mode {
            return Err(sunborn_core::SunbornError::cache(
                "DragonflyDB connection unavailable; fail-closed cache write enforced",
            ));
        }

        self.fallback_cache.set(entry).await
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        let mut redis_deleted = false;
        if let Some(mut conn) = self.get_connection().await {
            match conn.del::<_, usize>(key).await {
                Ok(count) => {
                    redis_deleted = count > 0;
                }
                Err(err) => {
                    tracing::error!(error = %err, "DragonflyDB delete command failed");
                    if self.strict_mode {
                        return Err(sunborn_core::SunbornError::cache(format!(
                            "DragonflyDB delete failed: {}",
                            err
                        )));
                    }
                }
            }
        } else if self.strict_mode {
            return Err(sunborn_core::SunbornError::cache(
                "DragonflyDB connection unavailable; fail-closed cache deletion enforced",
            ));
        }

        let memory_deleted = self.fallback_cache.delete(key).await?;
        Ok(redis_deleted || memory_deleted)
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        if let Some(mut conn) = self.get_connection().await {
            if let Ok(exists) = conn.exists::<_, bool>(key).await {
                if exists {
                    return Ok(true);
                }
            }
        } else if self.strict_mode {
            return Ok(false);
        }

        self.fallback_cache.exists(key).await
    }

    async fn delete_by_prefix(&self, prefix: &str) -> Result<usize> {
        let mut deleted_count = 0;
        if let Some(mut conn) = self.get_connection().await {
            let pattern = format!("{}*", prefix);
            match conn.keys::<_, Vec<String>>(&pattern).await {
                Ok(keys) => {
                    if !keys.is_empty() {
                        if let Ok(count) = conn.del::<_, usize>(&keys).await {
                            deleted_count += count;
                        }
                    }
                }
                Err(err) => {
                    tracing::error!(error = %err, "DragonflyDB keys scan failed");
                    if self.strict_mode {
                        return Err(sunborn_core::SunbornError::cache(format!(
                            "DragonflyDB delete_by_prefix failed: {}",
                            err
                        )));
                    }
                }
            }
        } else if self.strict_mode {
            return Err(sunborn_core::SunbornError::cache(
                "DragonflyDB connection unavailable; fail-closed prefix deletion enforced",
            ));
        }

        let mem_count = self.fallback_cache.delete_by_prefix(prefix).await?;
        Ok(deleted_count.max(mem_count))
    }

    async fn count_by_prefix(&self, prefix: &str) -> Result<usize> {
        let mut df_count = 0;
        if let Some(mut conn) = self.get_connection().await {
            let pattern = format!("{}*", prefix);
            match conn.keys::<_, Vec<String>>(&pattern).await {
                Ok(keys) => {
                    df_count = keys.len();
                }
                Err(err) => {
                    tracing::error!(error = %err, "DragonflyDB keys scan failed");
                    if self.strict_mode {
                        return Err(sunborn_core::SunbornError::cache(format!(
                            "DragonflyDB count_by_prefix failed: {}",
                            err
                        )));
                    }
                }
            }
        } else if self.strict_mode {
            return Err(sunborn_core::SunbornError::cache(
                "DragonflyDB connection unavailable; fail-closed prefix count enforced",
            ));
        }

        let mem_count = self.fallback_cache.count_by_prefix(prefix).await?;
        Ok(df_count.max(mem_count))
    }

    async fn sweep_expired(&self) -> Result<usize> {
        self.fallback_cache.sweep_expired().await
    }
}
