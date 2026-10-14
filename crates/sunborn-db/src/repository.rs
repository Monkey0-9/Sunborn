use crate::models::{AuditRecord, DurableCacheRecord, OutboxRecord, TenantRecord};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use sunborn_core::Result;
use tokio::sync::RwLock;

#[async_trait]
pub trait DatabaseStore: Send + Sync {
    async fn insert_tenant(&self, tenant: TenantRecord) -> Result<()>;
    async fn get_tenant(&self, id: &str) -> Result<Option<TenantRecord>>;
    async fn insert_cache_entry(&self, entry: DurableCacheRecord) -> Result<()>;
    async fn get_cache_entry(&self, key: &str) -> Result<Option<DurableCacheRecord>>;
    async fn increment_hit_count(&self, key: &str) -> Result<()>;
    async fn insert_outbox_event(&self, event: OutboxRecord) -> Result<()>;
    async fn fetch_pending_outbox_events(&self, limit: usize) -> Result<Vec<OutboxRecord>>;
    async fn mark_outbox_processed(&self, id: i64) -> Result<()>;
    async fn record_audit_event(&self, audit: AuditRecord) -> Result<()>;
    async fn scan_cache_entries(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<DurableCacheRecord>>;
    async fn count_cache_entries(&self) -> Result<usize>;
    async fn delete_tenant_data(&self, tenant_id: &str) -> Result<usize>;
}

#[derive(Clone, Default)]
pub struct MemoryDatabaseStore {
    tenants: Arc<RwLock<HashMap<String, TenantRecord>>>,
    cache_entries: Arc<RwLock<HashMap<String, DurableCacheRecord>>>,
    outbox: Arc<RwLock<Vec<OutboxRecord>>>,
    audits: Arc<RwLock<Vec<AuditRecord>>>,
}

impl MemoryDatabaseStore {
    pub fn new() -> Self {
        Self {
            tenants: Arc::new(RwLock::new(HashMap::new())),
            cache_entries: Arc::new(RwLock::new(HashMap::new())),
            outbox: Arc::new(RwLock::new(Vec::new())),
            audits: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl DatabaseStore for MemoryDatabaseStore {
    async fn insert_tenant(&self, tenant: TenantRecord) -> Result<()> {
        let mut tenants = self.tenants.write().await;
        tenants.insert(tenant.id.clone(), tenant);
        Ok(())
    }

    async fn get_tenant(&self, id: &str) -> Result<Option<TenantRecord>> {
        let tenants = self.tenants.read().await;
        Ok(tenants.get(id).cloned())
    }

    async fn insert_cache_entry(&self, entry: DurableCacheRecord) -> Result<()> {
        let mut cache = self.cache_entries.write().await;
        cache.insert(entry.key.clone(), entry);
        Ok(())
    }

    async fn get_cache_entry(&self, key: &str) -> Result<Option<DurableCacheRecord>> {
        let cache = self.cache_entries.read().await;
        Ok(cache.get(key).cloned())
    }

    async fn increment_hit_count(&self, key: &str) -> Result<()> {
        let mut cache = self.cache_entries.write().await;
        if let Some(entry) = cache.get_mut(key) {
            entry.hit_count += 1;
            entry.last_accessed_at = Utc::now();
        }
        Ok(())
    }

    async fn insert_outbox_event(&self, event: OutboxRecord) -> Result<()> {
        let mut outbox = self.outbox.write().await;
        outbox.push(event);
        Ok(())
    }

    async fn fetch_pending_outbox_events(&self, limit: usize) -> Result<Vec<OutboxRecord>> {
        let outbox = self.outbox.read().await;
        let pending: Vec<OutboxRecord> = outbox
            .iter()
            .filter(|e| e.status == "pending")
            .take(limit)
            .cloned()
            .collect();
        Ok(pending)
    }

    async fn mark_outbox_processed(&self, id: i64) -> Result<()> {
        let mut outbox = self.outbox.write().await;
        if let Some(event) = outbox.iter_mut().find(|e| e.id == id) {
            event.status = "processed".to_string();
        }
        Ok(())
    }

    async fn record_audit_event(&self, audit: AuditRecord) -> Result<()> {
        let mut audits = self.audits.write().await;
        audits.push(audit);
        Ok(())
    }

    async fn scan_cache_entries(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<DurableCacheRecord>> {
        let cache = self.cache_entries.read().await;
        let mut entries: Vec<DurableCacheRecord> = cache.values().cloned().collect();
        entries.sort_by_key(|a| a.created_at);
        let paged = entries.into_iter().skip(offset).take(limit).collect();
        Ok(paged)
    }

    async fn count_cache_entries(&self) -> Result<usize> {
        let cache = self.cache_entries.read().await;
        Ok(cache.len())
    }

    async fn delete_tenant_data(&self, tenant_id: &str) -> Result<usize> {
        let mut tenants = self.tenants.write().await;
        tenants.remove(tenant_id);

        let mut cache = self.cache_entries.write().await;
        let before_cache = cache.len();
        cache.retain(|_, v| v.tenant_id != tenant_id);
        let deleted_cache = before_cache - cache.len();

        let mut outbox = self.outbox.write().await;
        outbox.retain(|e| e.tenant_id != tenant_id);

        let mut audits = self.audits.write().await;
        audits.push(AuditRecord {
            request_id: uuid::Uuid::new_v4(),
            tenant_id: tenant_id.to_string(),
            action: "TENANT_OFFBOARDING_ERASURE".to_string(),
            details: serde_json::json!({
                "deleted_cache_entries": deleted_cache,
                "timestamp": Utc::now()
            }),
            created_at: Utc::now(),
        });

        Ok(deleted_cache)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_db_store_lifecycle() {
        let db = MemoryDatabaseStore::new();

        // 1. Insert tenant
        db.insert_tenant(TenantRecord {
            id: "t-fintech".into(),
            name: "FinTech Enterprise".into(),
            status: "active".into(),
            rate_limit_rps: 150,
            created_at: Utc::now(),
        })
        .await
        .unwrap();

        let tenant = db.get_tenant("t-fintech").await.unwrap();
        assert!(tenant.is_some());
        assert_eq!(tenant.unwrap().rate_limit_rps, 150);

        // 2. Insert cache entry & increment hit
        db.insert_cache_entry(DurableCacheRecord {
            key: "key-123".into(),
            tenant_id: "t-fintech".into(),
            application_id: "app-1".into(),
            model: "gpt-4o".into(),
            policy_version: "v1".into(),
            hash: "abc".into(),
            prompt_canonical: "test prompt".into(),
            content: "test content".into(),
            hit_count: 0,
            created_at: Utc::now(),
            expires_at: None,
            last_accessed_at: Utc::now(),
        })
        .await
        .unwrap();

        db.increment_hit_count("key-123").await.unwrap();
        let entry = db.get_cache_entry("key-123").await.unwrap().unwrap();
        assert_eq!(entry.hit_count, 1);
    }
}
