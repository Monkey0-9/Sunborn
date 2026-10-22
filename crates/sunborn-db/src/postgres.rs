use crate::models::{
    AuditRecord, DurableApiCredentialRecord, DurableCacheRecord, OutboxRecord, TenantRecord,
};
use crate::repository::{DatabaseStore, MemoryDatabaseStore};
use async_trait::async_trait;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::sync::Arc;
use std::time::Duration;
use sunborn_core::{Result, SunbornError};
use tokio::sync::RwLock;

/// Production PostgreSQL database store adapter implementing DatabaseStore
#[derive(Clone)]
pub struct PostgresDatabaseStore {
    pub connection_url: String,
    pool: Arc<RwLock<Option<PgPool>>>,
    fallback_store: Arc<MemoryDatabaseStore>,
    pub strict_durability: bool,
}

impl PostgresDatabaseStore {
    pub fn new(connection_url: impl Into<String>) -> Self {
        let url_str = connection_url.into();
        let strict =
            match sunborn_core::config::parse_bool_env("SUNBORN_DB_STRICT_DURABILITY", false) {
                Ok(v) => {
                    v || std::env::var("SUNBORN_ENV")
                        .map(|e| e.to_lowercase() == "production")
                        .unwrap_or(false)
                }
                Err(_) => true, // Invalid boolean value fails closed to strict durability
            };

        Self {
            connection_url: url_str,
            pool: Arc::new(RwLock::new(None)),
            fallback_store: Arc::new(MemoryDatabaseStore::new()),
            strict_durability: strict,
        }
    }

    pub fn new_strict(connection_url: impl Into<String>) -> Self {
        let url_str = connection_url.into();
        Self {
            connection_url: url_str,
            pool: Arc::new(RwLock::new(None)),
            fallback_store: Arc::new(MemoryDatabaseStore::new()),
            strict_durability: true,
        }
    }

    /// Actively validates PostgreSQL connection pool and executes a health query (SELECT 1).
    pub async fn verify_connection(&self) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            match sqlx::query("SELECT 1").execute(&pool).await {
                Ok(_) => Ok(()),
                Err(err) => Err(SunbornError::database(format!(
                    "PostgreSQL query check failed at {}: {err}",
                    self.connection_url
                ))),
            }
        } else {
            Err(SunbornError::database(format!(
                "Unable to establish connection pool to PostgreSQL at {}",
                self.connection_url
            )))
        }
    }

    async fn get_pool(&self) -> Option<PgPool> {
        {
            let guard = self.pool.read().await;
            if let Some(pool) = guard.as_ref() {
                return Some(pool.clone());
            }
        }

        if let Ok(pool) = PgPoolOptions::new()
            .max_connections(20)
            .acquire_timeout(Duration::from_secs(2))
            .connect(&self.connection_url)
            .await
        {
            if let Err(err) = self.migrate(&pool).await {
                tracing::error!(error = %err, "PostgreSQL schema auto-migration failed");
                if self.strict_durability {
                    return None;
                }
            }
            let mut guard = self.pool.write().await;
            *guard = Some(pool.clone());
            return Some(pool);
        }

        None
    }

    async fn migrate(&self, pool: &PgPool) -> std::result::Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tenants (
                id VARCHAR(128) PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                status VARCHAR(64) NOT NULL,
                rate_limit_rps INT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL
            );

            CREATE TABLE IF NOT EXISTS durable_cache_entries (
                key VARCHAR(512) PRIMARY KEY,
                tenant_id VARCHAR(128) NOT NULL,
                application_id VARCHAR(128) NOT NULL,
                model VARCHAR(128) NOT NULL,
                policy_version VARCHAR(64) NOT NULL,
                hash VARCHAR(128) NOT NULL,
                prompt_canonical TEXT NOT NULL,
                content TEXT NOT NULL,
                hit_count BIGINT NOT NULL DEFAULT 0,
                created_at TIMESTAMPTZ NOT NULL,
                expires_at TIMESTAMPTZ,
                last_accessed_at TIMESTAMPTZ NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_cache_tenant ON durable_cache_entries(tenant_id);

            CREATE TABLE IF NOT EXISTS outbox_events (
                id BIGSERIAL PRIMARY KEY,
                tenant_id VARCHAR(128) NOT NULL,
                entity_type VARCHAR(64) NOT NULL,
                entity_id VARCHAR(255) NOT NULL,
                action VARCHAR(64) NOT NULL,
                payload JSONB NOT NULL,
                status VARCHAR(64) NOT NULL,
                created_at TIMESTAMPTZ NOT NULL
            );

            CREATE TABLE IF NOT EXISTS audit_events (
                request_id UUID PRIMARY KEY,
                tenant_id VARCHAR(128) NOT NULL,
                action VARCHAR(128) NOT NULL,
                details JSONB NOT NULL,
                created_at TIMESTAMPTZ NOT NULL
            );

            CREATE TABLE IF NOT EXISTS tenant_wrapped_keys (
                tenant_id VARCHAR(128) PRIMARY KEY,
                key_version INT NOT NULL DEFAULT 1,
                wrapped_tdek BYTEA NOT NULL,
                kms_provider VARCHAR(64) NOT NULL DEFAULT 'local-aead',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );

            CREATE TABLE IF NOT EXISTS api_credentials (
                key_hash VARCHAR(128) PRIMARY KEY,
                tenant_id VARCHAR(128) NOT NULL,
                application_id VARCHAR(128) NOT NULL,
                rate_limit_rps INT NOT NULL DEFAULT 100,
                allowed_models JSONB NOT NULL DEFAULT '["*"]'::jsonb,
                is_admin BOOLEAN NOT NULL DEFAULT FALSE,
                status VARCHAR(32) NOT NULL DEFAULT 'active',
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
        )
        .execute(pool)
        .await?;

        Ok(())
    }

    /// Persists an enveloped Tenant Data Encryption Key (TDEK) wrapped under the master KEK into PostgreSQL.
    pub async fn persist_wrapped_tdek(
        &self,
        tenant_id: &str,
        wrapped_tdek: &[u8],
        kms_provider: &str,
    ) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let res = sqlx::query(
                "INSERT INTO tenant_wrapped_keys (tenant_id, wrapped_tdek, kms_provider, updated_at)
                 VALUES ($1, $2, $3, NOW())
                 ON CONFLICT (tenant_id) DO UPDATE SET wrapped_tdek = $2, kms_provider = $3, updated_at = NOW()",
            )
            .bind(tenant_id)
            .bind(wrapped_tdek)
            .bind(kms_provider)
            .execute(&pool)
            .await;

            if let Err(e) = res {
                tracing::error!(error = %e, "Failed to persist wrapped TDEK to PostgreSQL");
                if self.strict_durability {
                    return Err(sunborn_core::SunbornError::database(format!(
                        "Failed to persist wrapped TDEK: {e}"
                    )));
                }
            }
        }
        Ok(())
    }

    /// Fetches all persisted wrapped TDEKs for startup key hydration into memory.
    pub async fn fetch_all_wrapped_tdeks(&self) -> Result<Vec<(String, Vec<u8>, String)>> {
        if let Some(pool) = self.get_pool().await {
            let rows: std::result::Result<Vec<(String, Vec<u8>, String)>, sqlx::Error> =
                sqlx::query_as(
                    "SELECT tenant_id, wrapped_tdek, kms_provider FROM tenant_wrapped_keys",
                )
                .fetch_all(&pool)
                .await;

            match rows {
                Ok(records) => return Ok(records),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to query tenant_wrapped_keys from PostgreSQL");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "Failed to query tenant_wrapped_keys: {e}"
                        )));
                    }
                }
            }
        }
        Ok(Vec::new())
    }

    /// Persists an API credential record into the durable PostgreSQL `api_credentials` table.
    pub async fn persist_api_credential(
        &self,
        key_hash: &str,
        tenant_id: &str,
        application_id: &str,
        rate_limit_rps: i32,
        allowed_models: &[String],
        is_admin: bool,
    ) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let models_json =
                serde_json::to_value(allowed_models).unwrap_or_else(|_| serde_json::json!(["*"]));
            let res = sqlx::query(
                "INSERT INTO api_credentials (key_hash, tenant_id, application_id, rate_limit_rps, allowed_models, is_admin, status, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, 'active', NOW())
                 ON CONFLICT (key_hash) DO UPDATE SET
                    tenant_id = $2,
                    application_id = $3,
                    rate_limit_rps = $4,
                    allowed_models = $5,
                    is_admin = $6,
                    status = 'active'",
            )
            .bind(key_hash)
            .bind(tenant_id)
            .bind(application_id)
            .bind(rate_limit_rps)
            .bind(models_json)
            .bind(is_admin)
            .execute(&pool)
            .await;

            if let Err(e) = res {
                tracing::error!(error = %e, "Failed to persist API credential to PostgreSQL");
                if self.strict_durability {
                    return Err(sunborn_core::SunbornError::database(format!(
                        "Failed to persist API credential: {e}"
                    )));
                }
            }
        }
        Ok(())
    }

    /// Fetches all active durable API credentials from PostgreSQL for registry hydration.
    pub async fn fetch_all_api_credentials(&self) -> Result<Vec<DurableApiCredentialRecord>> {
        if let Some(pool) = self.get_pool().await {
            let query = "SELECT key_hash, tenant_id, application_id, rate_limit_rps, allowed_models, is_admin, status, created_at FROM api_credentials WHERE status = 'active'";
            let rows = sqlx::query(query).fetch_all(&pool).await;

            match rows {
                Ok(records) => {
                    let mut out = Vec::with_capacity(records.len());
                    for row in records {
                        let models_val: serde_json::Value = row.get("allowed_models");
                        let allowed_models: Vec<String> = serde_json::from_value(models_val)
                            .unwrap_or_else(|_| vec!["*".to_string()]);
                        let rps: i32 = row.get("rate_limit_rps");
                        out.push(DurableApiCredentialRecord {
                            key_hash: row.get("key_hash"),
                            tenant_id: row.get("tenant_id"),
                            application_id: row.get("application_id"),
                            rate_limit_rps: rps.max(1) as u32,
                            allowed_models,
                            is_admin: row.get("is_admin"),
                            status: row.get("status"),
                            created_at: row.get("created_at"),
                        });
                    }
                    return Ok(out);
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to fetch api_credentials from PostgreSQL");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "Failed to fetch api_credentials: {e}"
                        )));
                    }
                }
            }
        }
        Ok(Vec::new())
    }

    /// Revokes an API credential in the durable PostgreSQL store by marking status='revoked'.
    pub async fn revoke_api_credential(&self, key_hash: &str) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let res =
                sqlx::query("UPDATE api_credentials SET status = 'revoked' WHERE key_hash = $1")
                    .bind(key_hash)
                    .execute(&pool)
                    .await;

            if let Err(e) = res {
                tracing::error!(error = %e, "Failed to revoke API credential in PostgreSQL");
                if self.strict_durability {
                    return Err(sunborn_core::SunbornError::database(format!(
                        "Failed to revoke API credential: {e}"
                    )));
                }
            }
        }
        Ok(())
    }
}

#[async_trait]
impl DatabaseStore for PostgresDatabaseStore {
    async fn insert_tenant(&self, tenant: TenantRecord) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let res = sqlx::query(
                "INSERT INTO tenants (id, name, status, rate_limit_rps, created_at)
                 VALUES ($1, $2, $3, $4, $5)
                 ON CONFLICT (id) DO UPDATE SET name = $2, status = $3, rate_limit_rps = $4",
            )
            .bind(&tenant.id)
            .bind(&tenant.name)
            .bind(&tenant.status)
            .bind(tenant.rate_limit_rps as i32)
            .bind(tenant.created_at)
            .execute(&pool)
            .await;

            match res {
                Ok(_) => {
                    let _ = self.fallback_store.insert_tenant(tenant).await;
                    return Ok(());
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL insert_tenant failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL insert_tenant failed: {}",
                            err
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }

        self.fallback_store.insert_tenant(tenant).await
    }

    async fn get_tenant(&self, id: &str) -> Result<Option<TenantRecord>> {
        if let Some(pool) = self.get_pool().await {
            match sqlx::query(
                "SELECT id, name, status, rate_limit_rps, created_at FROM tenants WHERE id = $1",
            )
            .bind(id)
            .fetch_optional(&pool)
            .await
            {
                Ok(Some(row)) => {
                    let rps: i32 = row.get("rate_limit_rps");
                    return Ok(Some(TenantRecord {
                        id: row.get("id"),
                        name: row.get("name"),
                        status: row.get("status"),
                        rate_limit_rps: rps as u32,
                        created_at: row.get("created_at"),
                    }));
                }
                Ok(None) => {
                    if self.strict_durability {
                        return Ok(None);
                    }
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL get_tenant failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL get_tenant failed: {err}"
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        self.fallback_store.get_tenant(id).await
    }

    async fn insert_cache_entry(&self, entry: DurableCacheRecord) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let res = sqlx::query(
                "INSERT INTO durable_cache_entries 
                 (key, tenant_id, application_id, model, policy_version, hash, prompt_canonical, content, hit_count, created_at, expires_at, last_accessed_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                 ON CONFLICT (key) DO UPDATE SET hit_count = durable_cache_entries.hit_count + 1, last_accessed_at = $12"
            )
            .bind(&entry.key)
            .bind(&entry.tenant_id)
            .bind(&entry.application_id)
            .bind(&entry.model)
            .bind(&entry.policy_version)
            .bind(&entry.hash)
            .bind(&entry.prompt_canonical)
            .bind(&entry.content)
            .bind(entry.hit_count as i64)
            .bind(entry.created_at)
            .bind(entry.expires_at)
            .bind(entry.last_accessed_at)
            .execute(&pool)
            .await;

            match res {
                Ok(_) => {
                    let _ = self.fallback_store.insert_cache_entry(entry).await;
                    return Ok(());
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL insert_cache_entry failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL insert_cache_entry failed: {}",
                            err
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }

        self.fallback_store.insert_cache_entry(entry).await
    }

    async fn get_cache_entry(&self, key: &str) -> Result<Option<DurableCacheRecord>> {
        if let Some(pool) = self.get_pool().await {
            match sqlx::query(
                "SELECT key, tenant_id, application_id, model, policy_version, hash, prompt_canonical, content, hit_count, created_at, expires_at, last_accessed_at
                 FROM durable_cache_entries WHERE key = $1"
            )
            .bind(key)
            .fetch_optional(&pool)
            .await
            {
                Ok(Some(row)) => {
                    let hit_count: i64 = row.get("hit_count");
                    return Ok(Some(DurableCacheRecord {
                        key: row.get("key"),
                        tenant_id: row.get("tenant_id"),
                        application_id: row.get("application_id"),
                        model: row.get("model"),
                        policy_version: row.get("policy_version"),
                        hash: row.get("hash"),
                        prompt_canonical: row.get("prompt_canonical"),
                        content: row.get("content"),
                        hit_count: hit_count as u64,
                        created_at: row.get("created_at"),
                        expires_at: row.get("expires_at"),
                        last_accessed_at: row.get("last_accessed_at"),
                    }));
                }
                Ok(None) => {
                    if self.strict_durability {
                        return Ok(None);
                    }
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL get_cache_entry failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL get_cache_entry failed: {err}"
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        self.fallback_store.get_cache_entry(key).await
    }

    async fn increment_hit_count(&self, key: &str) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let _ = sqlx::query(
                "UPDATE durable_cache_entries SET hit_count = hit_count + 1, last_accessed_at = NOW() WHERE key = $1"
            )
            .bind(key)
            .execute(&pool)
            .await;
        }
        self.fallback_store.increment_hit_count(key).await
    }

    async fn insert_outbox_event(&self, event: OutboxRecord) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let res = sqlx::query(
                "INSERT INTO outbox_events (tenant_id, entity_type, entity_id, action, payload, status, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7)"
            )
            .bind(&event.tenant_id)
            .bind(&event.entity_type)
            .bind(&event.entity_id)
            .bind(&event.action)
            .bind(&event.payload)
            .bind(&event.status)
            .bind(event.created_at)
            .execute(&pool)
            .await;

            match res {
                Ok(_) => {
                    let _ = self.fallback_store.insert_outbox_event(event).await;
                    return Ok(());
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL insert_outbox_event failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL insert_outbox_event failed: {}",
                            err
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }

        self.fallback_store.insert_outbox_event(event).await
    }

    async fn fetch_pending_outbox_events(&self, limit: usize) -> Result<Vec<OutboxRecord>> {
        if let Some(pool) = self.get_pool().await {
            match sqlx::query(
                "SELECT id, tenant_id, entity_type, entity_id, action, payload, status, created_at
                 FROM outbox_events WHERE status = 'PENDING' ORDER BY id ASC LIMIT $1",
            )
            .bind(limit as i64)
            .fetch_all(&pool)
            .await
            {
                Ok(rows) => {
                    let events: Vec<OutboxRecord> = rows
                        .into_iter()
                        .map(|r| OutboxRecord {
                            id: r.get("id"),
                            tenant_id: r.get("tenant_id"),
                            entity_type: r.get("entity_type"),
                            entity_id: r.get("entity_id"),
                            action: r.get("action"),
                            payload: r.get("payload"),
                            status: r.get("status"),
                            created_at: r.get("created_at"),
                        })
                        .collect();
                    return Ok(events);
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL fetch_pending_outbox_events failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL fetch_pending_outbox_events failed: {err}"
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        self.fallback_store.fetch_pending_outbox_events(limit).await
    }

    async fn mark_outbox_processed(&self, id: i64) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let _ = sqlx::query("UPDATE outbox_events SET status = 'PROCESSED' WHERE id = $1")
                .bind(id)
                .execute(&pool)
                .await;
        }
        self.fallback_store.mark_outbox_processed(id).await
    }

    async fn record_audit_event(&self, audit: AuditRecord) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let res = sqlx::query(
                "INSERT INTO audit_events (request_id, tenant_id, action, details, created_at)
                 VALUES ($1, $2, $3, $4, $5)",
            )
            .bind(audit.request_id)
            .bind(&audit.tenant_id)
            .bind(&audit.action)
            .bind(&audit.details)
            .bind(audit.created_at)
            .execute(&pool)
            .await;

            match res {
                Ok(_) => {
                    let _ = self.fallback_store.record_audit_event(audit).await;
                    return Ok(());
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL record_audit_event failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL record_audit_event failed: {}",
                            err
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }

        self.fallback_store.record_audit_event(audit).await
    }

    async fn scan_cache_entries(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<DurableCacheRecord>> {
        if let Some(pool) = self.get_pool().await {
            match sqlx::query(
                "SELECT key, tenant_id, application_id, model, policy_version, hash, prompt_canonical, content, hit_count, created_at, expires_at, last_accessed_at
                 FROM durable_cache_entries ORDER BY created_at DESC LIMIT $1 OFFSET $2"
            )
            .bind(limit as i64)
            .bind(offset as i64)
            .fetch_all(&pool)
            .await
            {
                Ok(rows) => {
                    let entries: Vec<DurableCacheRecord> = rows
                        .into_iter()
                        .map(|row| {
                            let hit_count: i64 = row.get("hit_count");
                            DurableCacheRecord {
                                key: row.get("key"),
                                tenant_id: row.get("tenant_id"),
                                application_id: row.get("application_id"),
                                model: row.get("model"),
                                policy_version: row.get("policy_version"),
                                hash: row.get("hash"),
                                prompt_canonical: row.get("prompt_canonical"),
                                content: row.get("content"),
                                hit_count: hit_count as u64,
                                created_at: row.get("created_at"),
                                expires_at: row.get("expires_at"),
                                last_accessed_at: row.get("last_accessed_at"),
                            }
                        })
                        .collect();
                    return Ok(entries);
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL scan_cache_entries failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL scan_cache_entries failed: {err}"
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        self.fallback_store.scan_cache_entries(limit, offset).await
    }

    async fn count_cache_entries(&self) -> Result<usize> {
        if let Some(pool) = self.get_pool().await {
            match sqlx::query("SELECT COUNT(*) as count FROM durable_cache_entries")
                .fetch_one(&pool)
                .await
            {
                Ok(row) => {
                    let count: i64 = row.get("count");
                    return Ok(count as usize);
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL count_cache_entries failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL count_cache_entries failed: {err}"
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        self.fallback_store.count_cache_entries().await
    }

    async fn delete_tenant_data(&self, tenant_id: &str) -> Result<usize> {
        let mut pg_deleted = 0;
        if let Some(pool) = self.get_pool().await {
            match sqlx::query("DELETE FROM durable_cache_entries WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await
            {
                Ok(res) => {
                    pg_deleted = res.rows_affected() as usize;
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL delete_tenant_data failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL delete_tenant_data failed: {}",
                            err
                        )));
                    }
                }
            }
            let _ = sqlx::query("DELETE FROM tenants WHERE id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await;
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        let mem_deleted = self.fallback_store.delete_tenant_data(tenant_id).await?;
        Ok(pg_deleted.max(mem_deleted))
    }

    async fn count_tenant_entries(&self, tenant_id: &str) -> Result<usize> {
        let mut pg_count = 0;
        if let Some(pool) = self.get_pool().await {
            let cache_res: std::result::Result<(i64,), sqlx::Error> =
                sqlx::query_as("SELECT COUNT(*) FROM durable_cache_entries WHERE tenant_id = $1")
                    .bind(tenant_id)
                    .fetch_one(&pool)
                    .await;
            if let Ok(cache_rows) = cache_res {
                pg_count += cache_rows.0 as usize;
            }
            let tenant_res: std::result::Result<(i64,), sqlx::Error> =
                sqlx::query_as("SELECT COUNT(*) FROM tenants WHERE id = $1")
                    .bind(tenant_id)
                    .fetch_one(&pool)
                    .await;
            if let Ok(tenant_rows) = tenant_res {
                pg_count += tenant_rows.0 as usize;
            }
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        let mem_count = self.fallback_store.count_tenant_entries(tenant_id).await?;
        Ok(pg_count.max(mem_count))
    }
}
