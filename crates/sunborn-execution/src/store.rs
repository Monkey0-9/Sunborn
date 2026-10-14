use crate::model::ToolResult;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use sunborn_core::error::SunbornError;
use sunborn_core::model::TenantId;
use tokio::sync::RwLock;

#[async_trait]
pub trait ExecutionCacheStore: Send + Sync {
    async fn get(
        &self,
        tenant_id: &TenantId,
        signature: &str,
    ) -> Result<Option<ToolResult>, SunbornError>;
    async fn set(
        &self,
        tenant_id: &TenantId,
        signature: &str,
        result: &ToolResult,
        ttl_secs: Option<u64>,
    ) -> Result<(), SunbornError>;
    async fn evict(&self, tenant_id: &TenantId, signature: &str) -> Result<bool, SunbornError>;
    async fn evict_by_tool(
        &self,
        tenant_id: &TenantId,
        tool_name: &str,
    ) -> Result<usize, SunbornError>;
    async fn evict_by_tenant(&self, tenant_id: &TenantId) -> Result<usize, SunbornError>;
}

struct CacheEntry {
    result: ToolResult,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Clone)]
pub struct MemoryExecutionCacheStore {
    // Key format: "tenant:{tenant_id}:sig:{signature}"
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
}

impl MemoryExecutionCacheStore {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn key(tenant_id: &TenantId, signature: &str) -> String {
        format!("tenant:{}:sig:{}", tenant_id.0, signature)
    }
}

impl Default for MemoryExecutionCacheStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutionCacheStore for MemoryExecutionCacheStore {
    async fn get(
        &self,
        tenant_id: &TenantId,
        signature: &str,
    ) -> Result<Option<ToolResult>, SunbornError> {
        let key = Self::key(tenant_id, signature);
        let mut map = self.entries.write().await;
        if let Some(entry) = map.get(&key) {
            if let Some(expires_at) = entry.expires_at {
                if Utc::now() > expires_at {
                    map.remove(&key);
                    return Ok(None);
                }
            }
            return Ok(Some(entry.result.clone()));
        }
        Ok(None)
    }

    async fn set(
        &self,
        tenant_id: &TenantId,
        signature: &str,
        result: &ToolResult,
        ttl_secs: Option<u64>,
    ) -> Result<(), SunbornError> {
        let key = Self::key(tenant_id, signature);
        let expires_at = ttl_secs.map(|s| Utc::now() + Duration::seconds(s as i64));
        let entry = CacheEntry {
            result: result.clone(),
            expires_at,
        };
        let mut map = self.entries.write().await;
        map.insert(key, entry);
        Ok(())
    }

    async fn evict(&self, tenant_id: &TenantId, signature: &str) -> Result<bool, SunbornError> {
        let key = Self::key(tenant_id, signature);
        let mut map = self.entries.write().await;
        Ok(map.remove(&key).is_some())
    }

    async fn evict_by_tool(
        &self,
        tenant_id: &TenantId,
        tool_name: &str,
    ) -> Result<usize, SunbornError> {
        let prefix = format!("tenant:{}:sig:", tenant_id.0);
        let mut map = self.entries.write().await;
        let initial_len = map.len();
        map.retain(|k, v| !(k.starts_with(&prefix) && v.result.tool_name == tool_name));
        Ok(initial_len - map.len())
    }

    async fn evict_by_tenant(&self, tenant_id: &TenantId) -> Result<usize, SunbornError> {
        let prefix = format!("tenant:{}:sig:", tenant_id.0);
        let mut map = self.entries.write().await;
        let initial_len = map.len();
        map.retain(|k, _| !k.starts_with(&prefix));
        Ok(initial_len - map.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execution_cache_store_get_set() {
        let store = MemoryExecutionCacheStore::new();
        let tenant = TenantId::new("t1");
        let sig = "abcd1234efgh";
        let res = ToolResult {
            tool_name: "calculator".into(),
            signature: sig.into(),
            output: serde_json::json!({"result": 42}),
            is_error: false,
            execution_latency_ms: 250,
            created_at: Utc::now(),
        };

        store.set(&tenant, sig, &res, None).await.unwrap();
        let fetched = store.get(&tenant, sig).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().output["result"], 42);

        // Different tenant should miss
        let other_tenant = TenantId::new("t2");
        let miss = store.get(&other_tenant, sig).await.unwrap();
        assert!(miss.is_none());

        // Eviction test
        store.evict_by_tool(&tenant, "calculator").await.unwrap();
        assert!(store.get(&tenant, sig).await.unwrap().is_none());
    }
}
