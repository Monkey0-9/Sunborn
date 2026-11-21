use crate::models::{
    AuditRecord, DurableCacheRecord, FinOpsLedgerRecord, OutboxRecord, TenantErasureTaskRecord,
    TenantRecord,
};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use sunborn_core::Result;
use tokio::sync::RwLock;
use uuid::Uuid;

const OUTBOX_MAX_RETRIES: u32 = 5;
const OUTBOX_LEASE_SECS: u64 = 300;

#[async_trait]
pub trait DatabaseStore: Send + Sync {
    async fn insert_tenant(&self, tenant: TenantRecord) -> Result<()>;
    async fn get_tenant(&self, id: &str) -> Result<Option<TenantRecord>>;
    async fn insert_cache_entry(&self, entry: DurableCacheRecord) -> Result<()>;
    async fn get_cache_entry(&self, key: &str) -> Result<Option<DurableCacheRecord>>;
    async fn increment_hit_count(&self, key: &str) -> Result<()>;
    /// Insert a PENDING event and return its durable id. The caller must not
    /// report success until the event is processed or explicitly failed.
    async fn insert_outbox_event(&self, event: OutboxRecord) -> Result<i64>;
    async fn fetch_pending_outbox_events(&self, limit: usize) -> Result<Vec<OutboxRecord>>;
    /// Atomically claim up to `limit` PENDING (or lease-expired PROCESSING)
    /// events for a worker. Implementations must use a single-statement claim
    /// (PostgreSQL: FOR UPDATE SKIP LOCKED) so two replicas cannot process the
    /// same event concurrently.
    async fn claim_pending_outbox_events(
        &self,
        limit: usize,
        worker_id: &str,
    ) -> Result<Vec<OutboxRecord>>;
    async fn mark_outbox_processed(&self, id: i64) -> Result<()>;
    /// Record a processing failure. Retries with backoff up to the configured
    /// maximum, then moves the event to the persistent DLQ.
    async fn mark_outbox_failed(&self, id: i64, error: &str) -> Result<()>;
    /// Requeue DLQ events for another processing attempt. Returns the count.
    async fn replay_outbox_dlq(&self, limit: usize) -> Result<usize>;
    /// Number of outbox events currently parked in FAILED state, i.e. the
    /// effective DLQ depth. This is what `replay_outbox_dlq` can clear, so it is
    /// the value to alert on: the historical `outbox_dlq` audit rows are kept
    /// after a replay and would keep a naive `count(*)` alert firing forever.
    async fn failed_outbox_depth(&self) -> Result<usize>;
    /// Authoritative source epochs for recovery/rebuild verification.
    async fn load_all_epochs(&self) -> Result<HashMap<Uuid, u64>>;
    async fn record_audit_event(&self, audit: AuditRecord) -> Result<()>;
    async fn record_finops_ledger(&self, record: FinOpsLedgerRecord) -> Result<()>;
    async fn scan_cache_entries(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<DurableCacheRecord>>;
    async fn count_cache_entries(&self) -> Result<usize>;
    async fn count_tenant_entries(&self, tenant_id: &str) -> Result<usize>;
    async fn delete_tenant_data(&self, tenant_id: &str) -> Result<usize>;
    async fn create_erasure_task(&self, tenant_id: &str) -> Result<TenantErasureTaskRecord>;
    async fn update_erasure_task(&self, task: &TenantErasureTaskRecord) -> Result<()>;
    async fn get_erasure_task(&self, task_id: Uuid) -> Result<Option<TenantErasureTaskRecord>>;
    async fn get_active_erasure_task(
        &self,
        tenant_id: &str,
    ) -> Result<Option<TenantErasureTaskRecord>>;
    async fn list_incomplete_erasure_tasks(&self) -> Result<Vec<TenantErasureTaskRecord>>;
}

#[derive(Clone, Default)]
pub struct MemoryDatabaseStore {
    tenants: Arc<RwLock<HashMap<String, TenantRecord>>>,
    cache_entries: Arc<RwLock<HashMap<String, DurableCacheRecord>>>,
    outbox: Arc<RwLock<Vec<OutboxRecord>>>,
    outbox_claims: Arc<RwLock<HashMap<i64, std::time::Instant>>>,
    outbox_retries: Arc<RwLock<HashMap<i64, u32>>>,
    outbox_dlq: Arc<RwLock<Vec<(OutboxRecord, String)>>>,
    outbox_next_id: Arc<AtomicI64>,
    epochs: Arc<RwLock<HashMap<Uuid, u64>>>,
    audits: Arc<RwLock<Vec<AuditRecord>>>,
    finops_ledger: Arc<RwLock<Vec<FinOpsLedgerRecord>>>,
    erasure_tasks: Arc<RwLock<HashMap<Uuid, TenantErasureTaskRecord>>>,
}

impl MemoryDatabaseStore {
    pub fn new() -> Self {
        Self {
            tenants: Arc::new(RwLock::new(HashMap::new())),
            cache_entries: Arc::new(RwLock::new(HashMap::new())),
            outbox: Arc::new(RwLock::new(Vec::new())),
            outbox_claims: Arc::new(RwLock::new(HashMap::new())),
            outbox_retries: Arc::new(RwLock::new(HashMap::new())),
            outbox_dlq: Arc::new(RwLock::new(Vec::new())),
            outbox_next_id: Arc::new(AtomicI64::new(1)),
            epochs: Arc::new(RwLock::new(HashMap::new())),
            audits: Arc::new(RwLock::new(Vec::new())),
            finops_ledger: Arc::new(RwLock::new(Vec::new())),
            erasure_tasks: Arc::new(RwLock::new(HashMap::new())),
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

    async fn insert_outbox_event(&self, mut event: OutboxRecord) -> Result<i64> {
        if event.id == 0 {
            event.id = self.outbox_next_id.fetch_add(1, Ordering::SeqCst);
        }
        let id = event.id;
        self.outbox.write().await.push(event);
        Ok(id)
    }

    async fn fetch_pending_outbox_events(&self, limit: usize) -> Result<Vec<OutboxRecord>> {
        let outbox = self.outbox.read().await;
        let pending: Vec<OutboxRecord> = outbox
            .iter()
            .filter(|e| e.status == "PENDING")
            .take(limit)
            .cloned()
            .collect();
        Ok(pending)
    }

    async fn claim_pending_outbox_events(
        &self,
        limit: usize,
        _worker_id: &str,
    ) -> Result<Vec<OutboxRecord>> {
        let now = std::time::Instant::now();
        let mut claims = self.outbox_claims.write().await;
        let mut outbox = self.outbox.write().await;
        let mut claimed = Vec::new();
        for event in outbox.iter_mut() {
            if claimed.len() >= limit {
                break;
            }
            let expired_lease = event.status == "PROCESSING"
                && claims
                    .get(&event.id)
                    .map(|at| now.duration_since(*at).as_secs() >= OUTBOX_LEASE_SECS)
                    .unwrap_or(true);
            if event.status == "PENDING" || expired_lease {
                event.status = "PROCESSING".to_string();
                claims.insert(event.id, now);
                claimed.push(event.clone());
            }
        }
        Ok(claimed)
    }

    async fn mark_outbox_processed(&self, id: i64) -> Result<()> {
        let mut outbox = self.outbox.write().await;
        if let Some(event) = outbox.iter_mut().find(|e| e.id == id) {
            event.status = "PROCESSED".to_string();
        }
        self.outbox_claims.write().await.remove(&id);
        Ok(())
    }

    async fn mark_outbox_failed(&self, id: i64, error: &str) -> Result<()> {
        let mut outbox = self.outbox.write().await;
        let mut retries = self.outbox_retries.write().await;
        if let Some(event) = outbox.iter_mut().find(|e| e.id == id) {
            let attempt = retries.entry(id).or_insert(0);
            *attempt += 1;
            if *attempt >= OUTBOX_MAX_RETRIES {
                event.status = "FAILED".to_string();
                let dead = (event.clone(), error.to_string());
                self.outbox_dlq.write().await.push(dead);
            } else {
                event.status = "PENDING".to_string();
            }
        }
        self.outbox_claims.write().await.remove(&id);
        Ok(())
    }

    async fn replay_outbox_dlq(&self, limit: usize) -> Result<usize> {
        let mut outbox = self.outbox.write().await;
        let mut replayed = 0;
        for event in outbox.iter_mut() {
            if replayed >= limit {
                break;
            }
            if event.status == "FAILED" {
                event.status = "PENDING".to_string();
                replayed += 1;
            }
        }
        Ok(replayed)
    }

    async fn failed_outbox_depth(&self) -> Result<usize> {
        let outbox = self.outbox.read().await;
        Ok(outbox.iter().filter(|e| e.status == "FAILED").count())
    }

    async fn load_all_epochs(&self) -> Result<HashMap<Uuid, u64>> {
        Ok(self.epochs.read().await.clone())
    }

    async fn record_audit_event(&self, audit: AuditRecord) -> Result<()> {
        let mut audits = self.audits.write().await;
        audits.push(audit);
        Ok(())
    }

    async fn record_finops_ledger(&self, record: FinOpsLedgerRecord) -> Result<()> {
        let mut ledger = self.finops_ledger.write().await;
        ledger.push(record);
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

        let mut finops = self.finops_ledger.write().await;
        finops.retain(|r| r.tenant_id != tenant_id);

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

    async fn count_tenant_entries(&self, tenant_id: &str) -> Result<usize> {
        let mut count = 0;
        if self.tenants.read().await.contains_key(tenant_id) {
            count += 1;
        }
        let cache = self.cache_entries.read().await;
        count += cache.values().filter(|v| v.tenant_id == tenant_id).count();
        Ok(count)
    }

    async fn create_erasure_task(&self, tenant_id: &str) -> Result<TenantErasureTaskRecord> {
        let task = TenantErasureTaskRecord {
            task_id: Uuid::new_v4(),
            tenant_id: tenant_id.to_string(),
            current_step: "INITIATED".to_string(),
            status: "IN_PROGRESS".to_string(),
            last_error: None,
            shred_duration_ns: None,
            deleted_db_entries: 0,
            deleted_exact_cache: 0,
            deleted_vectors: 0,
            deleted_execution_tools: 0,
            deleted_learning_events: 0,
            verified_integrity: false,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let mut tasks = self.erasure_tasks.write().await;
        tasks.insert(task.task_id, task.clone());
        Ok(task)
    }

    async fn update_erasure_task(&self, task: &TenantErasureTaskRecord) -> Result<()> {
        let mut tasks = self.erasure_tasks.write().await;
        let mut updated = task.clone();
        updated.updated_at = Utc::now();
        tasks.insert(updated.task_id, updated);
        Ok(())
    }

    async fn get_erasure_task(&self, task_id: Uuid) -> Result<Option<TenantErasureTaskRecord>> {
        let tasks = self.erasure_tasks.read().await;
        Ok(tasks.get(&task_id).cloned())
    }

    async fn get_active_erasure_task(
        &self,
        tenant_id: &str,
    ) -> Result<Option<TenantErasureTaskRecord>> {
        let tasks = self.erasure_tasks.read().await;
        let active = tasks
            .values()
            .filter(|t| t.tenant_id == tenant_id && t.status == "IN_PROGRESS")
            .max_by_key(|t| t.created_at)
            .cloned();
        Ok(active)
    }

    async fn list_incomplete_erasure_tasks(&self) -> Result<Vec<TenantErasureTaskRecord>> {
        let tasks = self.erasure_tasks.read().await;
        let mut list: Vec<_> = tasks
            .values()
            .filter(|t| t.status == "IN_PROGRESS" || t.status == "FAILED")
            .cloned()
            .collect();
        list.sort_by_key(|t| t.created_at);
        Ok(list)
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
            certificate_json: None,
        })
        .await
        .unwrap();

        db.increment_hit_count("key-123").await.unwrap();
        let entry = db.get_cache_entry("key-123").await.unwrap().unwrap();
        assert_eq!(entry.hit_count, 1);
    }

    fn outbox_event() -> OutboxRecord {
        OutboxRecord {
            id: 0,
            tenant_id: "t1".into(),
            entity_type: "knowledge_source".into(),
            entity_id: "00000000-0000-0000-0000-000000000001".into(),
            action: "invalidate".into(),
            payload: serde_json::json!({"source_id": "x"}),
            status: "PENDING".into(),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_outbox_claim_is_exclusive_and_exhaustion_dead_letters() {
        let db = MemoryDatabaseStore::new();
        let id = db.insert_outbox_event(outbox_event()).await.unwrap();

        // Two workers must not claim the same PENDING event.
        let first = db
            .claim_pending_outbox_events(10, "worker-a")
            .await
            .unwrap();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].id, id);
        assert!(db
            .claim_pending_outbox_events(10, "worker-b")
            .await
            .unwrap()
            .is_empty());

        // Failure returns the event to PENDING until the retry budget is spent.
        for attempt in 1..=5 {
            db.mark_outbox_failed(id, "simulated store failure")
                .await
                .unwrap();
            let claimed = db
                .claim_pending_outbox_events(10, "worker-a")
                .await
                .unwrap();
            if attempt < 5 {
                assert_eq!(claimed.len(), 1);
            } else {
                assert!(claimed.is_empty());
            }
        }

        // The exhausted event can be replayed deliberately.
        assert_eq!(db.replay_outbox_dlq(10).await.unwrap(), 1);
        assert_eq!(
            db.claim_pending_outbox_events(10, "worker-c")
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn test_outbox_processed_requires_claim() {
        let db = MemoryDatabaseStore::new();
        let id = db.insert_outbox_event(outbox_event()).await.unwrap();
        db.mark_outbox_processed(id).await.unwrap();
        assert_eq!(db.fetch_pending_outbox_events(10).await.unwrap().len(), 0);
    }
}
