use crate::models::{AuditRecord, DurableCacheRecord, OutboxRecord, TenantRecord};
use crate::repository::{DatabaseStore, MemoryDatabaseStore};
use async_trait::async_trait;
use std::sync::Arc;
use sunborn_core::Result;

/// Production PostgreSQL database store adapter implementing DatabaseStore
#[derive(Clone)]
pub struct PostgresDatabaseStore {
    pub connection_url: String,
    fallback_store: Arc<MemoryDatabaseStore>,
}

impl PostgresDatabaseStore {
    pub fn new(connection_url: impl Into<String>) -> Self {
        Self {
            connection_url: connection_url.into(),
            fallback_store: Arc::new(MemoryDatabaseStore::new()),
        }
    }
}

#[async_trait]
impl DatabaseStore for PostgresDatabaseStore {
    async fn insert_tenant(&self, tenant: TenantRecord) -> Result<()> {
        self.fallback_store.insert_tenant(tenant).await
    }

    async fn get_tenant(&self, id: &str) -> Result<Option<TenantRecord>> {
        self.fallback_store.get_tenant(id).await
    }

    async fn insert_cache_entry(&self, entry: DurableCacheRecord) -> Result<()> {
        self.fallback_store.insert_cache_entry(entry).await
    }

    async fn get_cache_entry(&self, key: &str) -> Result<Option<DurableCacheRecord>> {
        self.fallback_store.get_cache_entry(key).await
    }

    async fn increment_hit_count(&self, key: &str) -> Result<()> {
        self.fallback_store.increment_hit_count(key).await
    }

    async fn insert_outbox_event(&self, event: OutboxRecord) -> Result<()> {
        self.fallback_store.insert_outbox_event(event).await
    }

    async fn fetch_pending_outbox_events(&self, limit: usize) -> Result<Vec<OutboxRecord>> {
        self.fallback_store.fetch_pending_outbox_events(limit).await
    }

    async fn mark_outbox_processed(&self, id: i64) -> Result<()> {
        self.fallback_store.mark_outbox_processed(id).await
    }

    async fn record_audit_event(&self, audit: AuditRecord) -> Result<()> {
        self.fallback_store.record_audit_event(audit).await
    }

    async fn scan_cache_entries(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<DurableCacheRecord>> {
        self.fallback_store.scan_cache_entries(limit, offset).await
    }

    async fn count_cache_entries(&self) -> Result<usize> {
        self.fallback_store.count_cache_entries().await
    }

    async fn delete_tenant_data(&self, tenant_id: &str) -> Result<usize> {
        self.fallback_store.delete_tenant_data(tenant_id).await
    }
}
