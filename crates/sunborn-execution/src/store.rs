use crate::model::ToolResult;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
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
    async fn count_by_tenant(&self, tenant_id: &TenantId) -> Result<usize, SunbornError>;

    fn verify_tenant_erased_on_disk(&self, _tenant_id: &TenantId) -> bool {
        true
    }

    fn storage_tier(&self) -> &'static str {
        "process-local-ephemeral"
    }

    fn is_durable(&self) -> bool {
        false
    }
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

    async fn count_by_tenant(&self, tenant_id: &TenantId) -> Result<usize, SunbornError> {
        let prefix = format!("tenant:{}:sig:", tenant_id.0);
        let map = self.entries.read().await;
        Ok(map.keys().filter(|k| k.starts_with(&prefix)).count())
    }
}

#[derive(Serialize, Deserialize)]
struct WalExecutionRecord {
    tenant_id: String,
    signature: String,
    result: Option<ToolResult>,
    expires_at: Option<DateTime<Utc>>,
    op: String,
}

#[derive(Clone)]
pub struct PersistentExecutionCacheStore {
    entries: Arc<RwLock<HashMap<String, CacheEntry>>>,
    wal_path: PathBuf,
}

impl PersistentExecutionCacheStore {
    pub fn new_with_wal(path: impl Into<PathBuf>) -> Self {
        let wal_path = path.into();
        let store = Self {
            entries: Arc::new(RwLock::new(HashMap::new())),
            wal_path,
        };
        store.load_from_wal();
        store
    }

    fn key(tenant_id: &TenantId, signature: &str) -> String {
        format!("tenant:{}:sig:{}", tenant_id.0, signature)
    }

    fn load_from_wal(&self) {
        if !self.wal_path.exists() {
            return;
        }
        if let Ok(file) = File::open(&self.wal_path) {
            let reader = BufReader::new(file);
            let mut map = HashMap::new();
            for line in reader.lines().map_while(std::result::Result::ok) {
                if let Ok(record) = serde_json::from_str::<WalExecutionRecord>(&line) {
                    let k = format!("tenant:{}:sig:{}", record.tenant_id, record.signature);
                    match record.op.as_str() {
                        "set" => {
                            if let Some(res) = record.result {
                                if let Some(exp) = record.expires_at {
                                    if Utc::now() > exp {
                                        continue;
                                    }
                                }
                                map.insert(
                                    k,
                                    CacheEntry {
                                        result: res,
                                        expires_at: record.expires_at,
                                    },
                                );
                            }
                        }
                        "evict" => {
                            map.remove(&k);
                        }
                        "evict_by_tool" => {
                            let tool = record.signature;
                            let prefix = format!("tenant:{}:sig:", record.tenant_id);
                            map.retain(|entry_k: &String, entry_v: &mut CacheEntry| {
                                !(entry_k.starts_with(&prefix) && entry_v.result.tool_name == tool)
                            });
                        }
                        "evict_by_tenant" => {
                            let prefix = format!("tenant:{}:sig:", record.tenant_id);
                            map.retain(|entry_k, _| !entry_k.starts_with(&prefix));
                        }
                        _ => {}
                    }
                }
            }
            if let Ok(mut lock) = self.entries.try_write() {
                *lock = map;
            } else {
                let entries = self.entries.clone();
                tokio::spawn(async move {
                    let mut lock = entries.write().await;
                    *lock = map;
                });
            }
        }
    }

    fn append_wal(&self, record: &WalExecutionRecord) {
        if let Some(parent) = self.wal_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.wal_path)
        {
            if let Ok(line) = serde_json::to_string(record) {
                let _ = writeln!(file, "{}", line);
            }
        }
    }

    /// Atomically rewrites the execution WAL file to physically remove all entries
    /// belonging to `tenant_id_str` (GDPR Article 17 / Cryptographic shredding compliance).
    fn rewrite_wal_excluding_tenant(&self, tenant_id_str: &str) -> Result<(), SunbornError> {
        if !self.wal_path.exists() {
            return Ok(());
        }
        let file = File::open(&self.wal_path).map_err(|e| SunbornError::internal(e.to_string()))?;
        let reader = BufReader::new(file);

        let tmp_path = self
            .wal_path
            .with_extension(format!("tmp.{}", uuid::Uuid::new_v4()));
        {
            let mut out = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(&tmp_path)
                .map_err(|e| SunbornError::internal(e.to_string()))?;

            for line in reader.lines().map_while(std::result::Result::ok) {
                if let Ok(rec) = serde_json::from_str::<WalExecutionRecord>(&line) {
                    if rec.tenant_id != tenant_id_str {
                        writeln!(out, "{}", line)
                            .map_err(|e| SunbornError::internal(e.to_string()))?;
                    }
                }
            }
            out.sync_all()
                .map_err(|e| SunbornError::internal(e.to_string()))?;
        }

        #[cfg(windows)]
        {
            if self.wal_path.exists() {
                let _ = std::fs::remove_file(&self.wal_path);
            }
        }
        std::fs::rename(&tmp_path, &self.wal_path).map_err(|e| {
            let _ = std::fs::remove_file(&tmp_path);
            SunbornError::internal(format!("Failed to atomically replace execution WAL: {e}"))
        })?;

        Ok(())
    }
}

#[async_trait]
impl ExecutionCacheStore for PersistentExecutionCacheStore {
    fn storage_tier(&self) -> &'static str {
        "durable-wal"
    }

    fn is_durable(&self) -> bool {
        true
    }

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
        {
            let mut map = self.entries.write().await;
            map.insert(key, entry);
        }
        self.append_wal(&WalExecutionRecord {
            tenant_id: tenant_id.0.clone(),
            signature: signature.to_string(),
            result: Some(result.clone()),
            expires_at,
            op: "set".to_string(),
        });
        Ok(())
    }

    async fn evict(&self, tenant_id: &TenantId, signature: &str) -> Result<bool, SunbornError> {
        let key = Self::key(tenant_id, signature);
        let removed = {
            let mut map = self.entries.write().await;
            map.remove(&key).is_some()
        };
        if removed {
            self.append_wal(&WalExecutionRecord {
                tenant_id: tenant_id.0.clone(),
                signature: signature.to_string(),
                result: None,
                expires_at: None,
                op: "evict".to_string(),
            });
        }
        Ok(removed)
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
        let count = initial_len - map.len();
        if count > 0 {
            self.append_wal(&WalExecutionRecord {
                tenant_id: tenant_id.0.clone(),
                signature: tool_name.to_string(),
                result: None,
                expires_at: None,
                op: "evict_by_tool".to_string(),
            });
        }
        Ok(count)
    }

    async fn evict_by_tenant(&self, tenant_id: &TenantId) -> Result<usize, SunbornError> {
        let prefix = format!("tenant:{}:sig:", tenant_id.0);
        let mut map = self.entries.write().await;
        let initial_len = map.len();
        map.retain(|k, _| !k.starts_with(&prefix));
        let count = initial_len - map.len();

        // Physically rewrite the WAL to ensure zero lingering ciphertext/plaintext records on disk
        self.rewrite_wal_excluding_tenant(&tenant_id.0)?;
        Ok(count)
    }

    async fn count_by_tenant(&self, tenant_id: &TenantId) -> Result<usize, SunbornError> {
        let prefix = format!("tenant:{}:sig:", tenant_id.0);
        let map = self.entries.read().await;
        Ok(map.keys().filter(|k| k.starts_with(&prefix)).count())
    }

    fn verify_tenant_erased_on_disk(&self, tenant_id: &TenantId) -> bool {
        if !self.wal_path.exists() {
            return true;
        }
        let Ok(file) = File::open(&self.wal_path) else {
            return false;
        };
        let reader = BufReader::new(file);
        for line in reader.lines().map_while(std::result::Result::ok) {
            if let Ok(rec) = serde_json::from_str::<WalExecutionRecord>(&line) {
                if rec.tenant_id == tenant_id.0 {
                    return false;
                }
            }
        }
        true
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

    #[tokio::test]
    async fn test_persistent_execution_cache_store_wal_replay() {
        let temp_dir =
            std::env::temp_dir().join(format!("sunborn_test_wal_{}", uuid::Uuid::new_v4()));
        let wal_file = temp_dir.join("execution.wal");

        let tenant = TenantId::new("tenant-wal");
        let sig = "sig-compute-hash-1";
        let res = ToolResult {
            tool_name: "vector_matrix_multiply".into(),
            signature: sig.into(),
            output: serde_json::json!({"matrix_determinant": 128.5}),
            is_error: false,
            execution_latency_ms: 12,
            created_at: Utc::now(),
        };

        // 1. Write to first store instance
        {
            let store = PersistentExecutionCacheStore::new_with_wal(&wal_file);
            assert!(store.is_durable());
            assert_eq!(store.storage_tier(), "durable-wal");

            store.set(&tenant, sig, &res, None).await.unwrap();
            let retrieved = store.get(&tenant, sig).await.unwrap();
            assert!(retrieved.is_some());
            assert_eq!(retrieved.unwrap().output["matrix_determinant"], 128.5);
        }

        // 2. Instantiate second store pointing to same WAL (simulating process restart)
        {
            let restored_store = PersistentExecutionCacheStore::new_with_wal(&wal_file);
            let restored_val = restored_store.get(&tenant, sig).await.unwrap();
            assert!(
                restored_val.is_some(),
                "Execution entry must survive store restart via WAL"
            );
            assert_eq!(restored_val.unwrap().output["matrix_determinant"], 128.5);
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
