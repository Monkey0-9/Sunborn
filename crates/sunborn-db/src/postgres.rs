use crate::models::{
    AuditRecord, DurableApiCredentialRecord, DurableCacheRecord, FinOpsLedgerRecord,
    NewApiCredentialRecord, OutboxRecord, TenantRecord,
};
use crate::repository::{DatabaseStore, MemoryDatabaseStore};
use async_trait::async_trait;
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sunborn_core::{Result, SunbornError};
use tokio::sync::RwLock;
use uuid::Uuid;

pub fn redact_connection_url(url: &str) -> String {
    if let Some(at_idx) = url.find('@') {
        if let Some(proto_idx) = url.find("://") {
            let userinfo_start = proto_idx + 3;
            if userinfo_start < at_idx {
                return format!("{}***:***@{}", &url[..userinfo_start], &url[at_idx + 1..]);
            }
        }
    }
    url.to_string()
}

type FallbackTdekMap = Arc<RwLock<std::collections::HashMap<String, (Vec<u8>, String)>>>;

const MIGRATION_0001: &str = include_str!("../../../migrations/0001_initial_schema.sql");
const MIGRATION_0002: &str = include_str!("../../../migrations/0002_durable_identity_and_keys.sql");
const MIGRATION_0003: &str = include_str!("../../../migrations/0003_finops_and_security.sql");
const MIGRATION_0004: &str = include_str!("../../../migrations/0004_durable_cache_certificate.sql");
const MIGRATION_0005: &str = include_str!("../../../migrations/0005_persistent_epochs_and_dlq.sql");
const MIGRATION_0006: &str = include_str!("../../../migrations/0006_durable_outbox_queue.sql");
const MIGRATION_0007: &str = include_str!("../../../migrations/0007_distributed_spend_ledger.sql");

/// Outcome of a cluster-wide spend reservation attempt.
#[derive(Debug, Clone, PartialEq)]
pub enum SpendReservationOutcome {
    /// The reservation is durable and visible to every replica.
    Granted(Uuid),
    /// The tenant's rolling-window commitment would exceed its ceiling.
    Denied { committed: f64, ceiling: f64 },
    /// The authoritative store could not be reached. Callers must fail closed.
    StoreUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DbCircuitState {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug, Clone)]
pub struct DbCircuitBreaker {
    consecutive_failures: u32,
    failure_threshold: u32,
    cooldown_secs: u64,
    last_failure_at: Option<Instant>,
}

impl DbCircuitBreaker {
    pub fn new(failure_threshold: u32, cooldown_secs: u64) -> Self {
        Self {
            consecutive_failures: 0,
            failure_threshold,
            cooldown_secs,
            last_failure_at: None,
        }
    }

    pub fn state(&self) -> DbCircuitState {
        if self.consecutive_failures >= self.failure_threshold {
            if let Some(last) = self.last_failure_at {
                if last.elapsed() > Duration::from_secs(self.cooldown_secs) {
                    return DbCircuitState::HalfOpen;
                }
            }
            DbCircuitState::Open
        } else {
            DbCircuitState::Closed
        }
    }

    pub fn record_success(&mut self) {
        self.consecutive_failures = 0;
        self.last_failure_at = None;
    }

    pub fn record_failure(&mut self) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.last_failure_at = Some(Instant::now());
    }

    pub fn reset(&mut self) {
        self.consecutive_failures = 0;
        self.last_failure_at = None;
    }
}

/// Production PostgreSQL database store adapter implementing DatabaseStore
#[derive(Clone)]
pub struct PostgresDatabaseStore {
    pub connection_url: String,
    pool: Arc<RwLock<Option<PgPool>>>,
    pool_init_lock: Arc<tokio::sync::Mutex<()>>,
    fallback_store: Arc<MemoryDatabaseStore>,
    fallback_tdeks: FallbackTdekMap,
    pub strict_durability: bool,
    circuit_breaker: Arc<RwLock<DbCircuitBreaker>>,
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
            pool_init_lock: Arc::new(tokio::sync::Mutex::new(())),
            fallback_store: Arc::new(MemoryDatabaseStore::new()),
            fallback_tdeks: Arc::new(RwLock::new(std::collections::HashMap::new())),
            strict_durability: strict,
            circuit_breaker: Arc::new(RwLock::new(DbCircuitBreaker::new(3, 5))),
        }
    }

    pub fn new_strict(connection_url: impl Into<String>) -> Self {
        let url_str = connection_url.into();
        Self {
            connection_url: url_str,
            pool: Arc::new(RwLock::new(None)),
            pool_init_lock: Arc::new(tokio::sync::Mutex::new(())),
            fallback_store: Arc::new(MemoryDatabaseStore::new()),
            fallback_tdeks: Arc::new(RwLock::new(std::collections::HashMap::new())),
            strict_durability: true,
            circuit_breaker: Arc::new(RwLock::new(DbCircuitBreaker::new(3, 5))),
        }
    }

    /// Returns the current state of the database connection circuit breaker.
    pub async fn circuit_state(&self) -> DbCircuitState {
        let cb = self.circuit_breaker.read().await;
        cb.state()
    }

    /// Explicitly resets the circuit breaker to closed state.
    pub async fn reset_circuit_breaker(&self) {
        let mut cb = self.circuit_breaker.write().await;
        cb.reset();
    }

    /// Actively validates PostgreSQL connection pool and executes a health query (SELECT 1).
    pub async fn verify_connection(&self) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            match sqlx::query("SELECT 1").execute(&pool).await {
                Ok(_) => Ok(()),
                Err(err) => Err(SunbornError::database(format!(
                    "PostgreSQL query check failed at {}: {err}",
                    redact_connection_url(&self.connection_url)
                ))),
            }
        } else {
            Err(SunbornError::database(format!(
                "Unable to establish connection pool to PostgreSQL at {}",
                redact_connection_url(&self.connection_url)
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

        // Fast-fail if database circuit breaker is open
        {
            let cb = self.circuit_breaker.read().await;
            if cb.state() == DbCircuitState::Open {
                tracing::debug!(
                    "PostgreSQL connection circuit breaker OPEN; failing fast without acquire"
                );
                return None;
            }
        }

        let _init_guard = self.pool_init_lock.lock().await;
        {
            let guard = self.pool.read().await;
            if let Some(pool) = guard.as_ref() {
                return Some(pool.clone());
            }
        }

        match PgPoolOptions::new()
            .max_connections(20)
            .acquire_timeout(Duration::from_secs(2))
            .connect(&self.connection_url)
            .await
        {
            Ok(pool) => {
                if let Err(err) = self.migrate(&pool).await {
                    tracing::error!(error = %err, "PostgreSQL schema auto-migration failed");
                    if self.strict_durability {
                        let mut cb = self.circuit_breaker.write().await;
                        cb.record_failure();
                        return None;
                    }
                }
                {
                    let mut cb = self.circuit_breaker.write().await;
                    cb.record_success();
                }
                let mut guard = self.pool.write().await;
                *guard = Some(pool.clone());
                Some(pool)
            }
            Err(err) => {
                tracing::warn!(error = %err, "Failed to connect to PostgreSQL; recording failure in circuit breaker");
                let mut cb = self.circuit_breaker.write().await;
                cb.record_failure();
                None
            }
        }
    }

    async fn migrate(&self, pool: &PgPool) -> std::result::Result<(), sqlx::Error> {
        // Multiple replicas may start simultaneously. Serialize the migration
        // bookkeeping so two processes cannot both insert the same version.
        // Acquire an exclusive connection from the pool so that pg_advisory_lock
        // and pg_advisory_unlock are executed on the exact same backend session,
        // preventing leaked session advisory locks and mismatched unlock warnings.
        const MIGRATION_LOCK_ID: i64 = 0x0053_554E_424F_524E; // "SUNBORN"
        let mut lock_conn = pool.acquire().await?;
        sqlx::query("SELECT pg_advisory_lock($1)")
            .bind(MIGRATION_LOCK_ID)
            .execute(&mut *lock_conn)
            .await?;
        let result = self.migrate_locked(pool).await;
        let _ = sqlx::query("SELECT pg_advisory_unlock($1)")
            .bind(MIGRATION_LOCK_ID)
            .execute(&mut *lock_conn)
            .await;
        drop(lock_conn);
        result
    }

    async fn migrate_locked(&self, pool: &PgPool) -> std::result::Result<(), sqlx::Error> {
        // Authoritative Singular Database Migration Runner (SSOT)
        sqlx::raw_sql(
            r#"
            CREATE TABLE IF NOT EXISTS _sunborn_migrations (
                version INT PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                checksum VARCHAR(64) NOT NULL,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
        )
        .execute(pool)
        .await?;

        let migrations: &[(i32, &str, &str)] = &[
            (1, "0001_initial_schema", MIGRATION_0001),
            (2, "0002_durable_identity_and_keys", MIGRATION_0002),
            (3, "0003_finops_and_security", MIGRATION_0003),
            (4, "0004_durable_cache_certificate", MIGRATION_0004),
            (5, "0005_persistent_epochs_and_dlq", MIGRATION_0005),
            (6, "0006_durable_outbox_queue", MIGRATION_0006),
            (7, "0007_distributed_spend_ledger", MIGRATION_0007),
        ];

        for (version, name, sql) in migrations {
            let row = sqlx::query("SELECT checksum FROM _sunborn_migrations WHERE version = $1")
                .bind(version)
                .fetch_optional(pool)
                .await?;

            if row.is_none() {
                tracing::info!(
                    version = *version,
                    name = *name,
                    "Applying authoritative database migration"
                );
                sqlx::raw_sql(sql).execute(pool).await?;
                use sha2::Digest;
                let checksum = hex::encode(sha2::Sha256::digest(sql.as_bytes()));
                sqlx::query(
                    "INSERT INTO _sunborn_migrations (version, name, checksum) VALUES ($1, $2, $3)",
                )
                .bind(version)
                .bind(name)
                .bind(&checksum)
                .execute(pool)
                .await?;
            }
        }

        Ok(())
    }

    /// Sets a durable per-tenant spend ceiling shared by every replica.
    pub async fn set_spend_ceiling(&self, tenant_id: &str, ceiling_usd: f64) -> Result<()> {
        let pool = self.get_pool().await.ok_or_else(|| {
            SunbornError::database("PostgreSQL connection pool unavailable for spend ceiling")
        })?;
        sqlx::query(
            "INSERT INTO spend_tenant_ceilings (tenant_id, ceiling_usd, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (tenant_id) DO UPDATE
             SET ceiling_usd = EXCLUDED.ceiling_usd, updated_at = NOW()",
        )
        .bind(tenant_id)
        .bind(ceiling_usd)
        .execute(&pool)
        .await
        .map_err(|e| SunbornError::database(e.to_string()))?;
        Ok(())
    }

    /// Atomically reserves budget against the tenant's cluster-wide rolling window.
    ///
    /// The transaction takes a per-tenant advisory lock, so concurrent replicas
    /// cannot both observe an under-ceiling commitment and both spend into it.
    pub async fn reserve_spend(
        &self,
        tenant_id: &str,
        estimated_cost: f64,
        default_ceiling: f64,
        window: Duration,
    ) -> SpendReservationOutcome {
        if !estimated_cost.is_finite() || !(0.0..=100_000.0).contains(&estimated_cost) {
            return SpendReservationOutcome::Denied {
                committed: 0.0,
                ceiling: default_ceiling,
            };
        }
        let Some(pool) = self.get_pool().await else {
            return SpendReservationOutcome::StoreUnavailable;
        };
        let window_secs = window.as_secs_f64().max(1.0);

        let mut tx = match pool.begin().await {
            Ok(tx) => tx,
            Err(err) => {
                tracing::error!(error = %err, "Failed to begin spend reservation transaction");
                return SpendReservationOutcome::StoreUnavailable;
            }
        };

        // Serialize reservation attempts per tenant across all replicas.
        if let Err(err) = sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(tenant_id)
            .execute(&mut *tx)
            .await
        {
            tracing::error!(error = %err, "Failed to acquire spend ledger tenant lock");
            return SpendReservationOutcome::StoreUnavailable;
        }

        let ceiling: f64 = match sqlx::query_scalar::<_, Option<f64>>(
            "SELECT ceiling_usd FROM spend_tenant_ceilings WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_one(&mut *tx)
        .await
        {
            Ok(Some(value)) => value,
            Ok(None) => default_ceiling,
            Err(err) => {
                tracing::error!(error = %err, "Failed to read durable spend ceiling");
                return SpendReservationOutcome::StoreUnavailable;
            }
        };

        let committed: f64 = match sqlx::query_scalar::<_, f64>(
            "SELECT
                 COALESCE((SELECT SUM(amount_usd) FROM spend_ledger_entries
                           WHERE tenant_id = $1 AND recorded_at > NOW() - ($2 * INTERVAL '1 second')), 0)
               + COALESCE((SELECT SUM(amount_usd) FROM spend_reservations
                           WHERE tenant_id = $1 AND reserved_at > NOW() - ($2 * INTERVAL '1 second')), 0)",
        )
        .bind(tenant_id)
        .bind(window_secs)
        .fetch_one(&mut *tx)
        .await
        {
            Ok(value) => value,
            Err(err) => {
                tracing::error!(error = %err, "Failed to read spend ledger commitment");
                return SpendReservationOutcome::StoreUnavailable;
            }
        };

        let would_commit = committed + estimated_cost;
        if committed >= ceiling || (committed > 0.0 && would_commit > ceiling) {
            // Rolling back also releases the advisory lock.
            return SpendReservationOutcome::Denied {
                committed: would_commit,
                ceiling,
            };
        }

        let reservation_id = Uuid::new_v4();
        let insert = sqlx::query(
            "INSERT INTO spend_reservations (reservation_id, tenant_id, amount_usd, reserved_at)
             VALUES ($1, $2, $3, NOW())",
        )
        .bind(reservation_id)
        .bind(tenant_id)
        .bind(estimated_cost)
        .execute(&mut *tx)
        .await;

        if let Err(err) = insert {
            tracing::error!(error = %err, "Failed to persist spend reservation");
            return SpendReservationOutcome::StoreUnavailable;
        }
        if let Err(err) = tx.commit().await {
            tracing::error!(error = %err, "Failed to commit spend reservation");
            return SpendReservationOutcome::StoreUnavailable;
        }

        SpendReservationOutcome::Granted(reservation_id)
    }

    /// Settles a durable reservation with the actual cost and returns the tenant's
    /// rolling-window spend. Returns `None` when the store is unavailable.
    pub async fn settle_spend(
        &self,
        tenant_id: &str,
        reservation_id: Uuid,
        actual_cost: f64,
        window: Duration,
    ) -> Option<f64> {
        let valid_cost = if actual_cost.is_finite() && actual_cost >= 0.0 {
            actual_cost.min(100_000.0)
        } else {
            0.0
        };
        let pool = self.get_pool().await?;
        let window_secs = window.as_secs_f64().max(1.0);

        let mut tx = pool.begin().await.ok()?;
        if sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(tenant_id)
            .execute(&mut *tx)
            .await
            .is_err()
        {
            return None;
        }
        // Idempotent: a duplicate settlement for the same reservation is ignored.
        sqlx::query("DELETE FROM spend_reservations WHERE reservation_id = $1 AND tenant_id = $2")
            .bind(reservation_id)
            .bind(tenant_id)
            .execute(&mut *tx)
            .await
            .ok()?;
        sqlx::query(
            "INSERT INTO spend_ledger_entries (tenant_id, reservation_id, amount_usd, recorded_at)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (reservation_id) DO NOTHING",
        )
        .bind(tenant_id)
        .bind(reservation_id)
        .bind(valid_cost)
        .execute(&mut *tx)
        .await
        .ok()?;
        // Bound table growth without a background job: drop entries that cannot
        // affect any supported window (clamped to one hour).
        sqlx::query(
            "DELETE FROM spend_ledger_entries
             WHERE tenant_id = $1 AND recorded_at < NOW() - INTERVAL '1 hour'",
        )
        .bind(tenant_id)
        .execute(&mut *tx)
        .await
        .ok()?;
        let total: f64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_usd), 0) FROM spend_ledger_entries
             WHERE tenant_id = $1 AND recorded_at > NOW() - ($2 * INTERVAL '1 second')",
        )
        .bind(tenant_id)
        .bind(window_secs)
        .fetch_one(&mut *tx)
        .await
        .ok()?;
        tx.commit().await.ok()?;
        Some(total)
    }

    /// Releases an unused durable reservation (cache hit or aborted request).
    pub async fn release_spend(&self, tenant_id: &str, reservation_id: Uuid) -> bool {
        let Some(pool) = self.get_pool().await else {
            return false;
        };
        sqlx::query("DELETE FROM spend_reservations WHERE reservation_id = $1 AND tenant_id = $2")
            .bind(reservation_id)
            .bind(tenant_id)
            .execute(&pool)
            .await
            .is_ok()
    }

    /// Reads the tenant's cluster-wide rolling-window spend.
    pub async fn spend_window_total(&self, tenant_id: &str, window: Duration) -> Option<f64> {
        let pool = self.get_pool().await?;
        sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_usd), 0) FROM spend_ledger_entries
             WHERE tenant_id = $1 AND recorded_at > NOW() - ($2 * INTERVAL '1 second')",
        )
        .bind(tenant_id)
        .bind(window.as_secs_f64().max(1.0))
        .fetch_one(&pool)
        .await
        .ok()
    }

    /// Removes all durable spend state for a tenant. Intended for operator resets.
    pub async fn reset_spend(&self, tenant_id: &str) -> bool {
        let Some(pool) = self.get_pool().await else {
            return false;
        };
        let mut tx = match pool.begin().await {
            Ok(tx) => tx,
            Err(_) => return false,
        };
        let ok = sqlx::query("DELETE FROM spend_reservations WHERE tenant_id = $1")
            .bind(tenant_id)
            .execute(&mut *tx)
            .await
            .is_ok()
            && sqlx::query("DELETE FROM spend_ledger_entries WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&mut *tx)
                .await
                .is_ok();
        ok && tx.commit().await.is_ok()
    }

    /// Loads all persistent knowledge source epochs from PostgreSQL.
    pub async fn load_all_epochs(&self) -> Result<std::collections::HashMap<Uuid, u64>> {
        if let Some(pool) = self.get_pool().await {
            let rows = sqlx::query_as::<_, (Uuid, i64)>(
                "SELECT source_id, current_epoch FROM tenant_source_epochs",
            )
            .fetch_all(&pool)
            .await
            .map_err(|e| SunbornError::database(e.to_string()))?;

            let mut map = std::collections::HashMap::new();
            for (src, ep) in rows {
                map.insert(src, ep as u64);
            }
            Ok(map)
        } else {
            Err(SunbornError::database(
                "PostgreSQL epoch authority unavailable; cannot hydrate epochs",
            ))
        }
    }

    /// Atomically advances the monotonic epoch for a knowledge source in PostgreSQL.
    ///
    /// `GREATEST(current_epoch + 1, proposed)` makes concurrent writers safe: a
    /// stale replica can never overwrite a newer durable epoch, and every caller
    /// receives the authoritative post-write value. Returns an error when the
    /// authoritative store is unavailable rather than silently advancing only
    /// in process memory.
    pub async fn advance_epoch(&self, source_id: Uuid, proposed: u64) -> Result<u64> {
        let pool = self.get_pool().await.ok_or_else(|| {
            SunbornError::database(
                "PostgreSQL epoch authority unavailable; refusing non-durable epoch advance",
            )
        })?;
        let row: (i64,) = sqlx::query_as(
            "INSERT INTO tenant_source_epochs (source_id, current_epoch, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (source_id) DO UPDATE SET
                 current_epoch = GREATEST(tenant_source_epochs.current_epoch + 1, EXCLUDED.current_epoch),
                 updated_at = NOW()
             RETURNING current_epoch",
        )
        .bind(source_id)
        .bind(proposed as i64)
        .fetch_one(&pool)
        .await
        .map_err(|e| SunbornError::database(format!("PostgreSQL epoch advance failed: {e}")))?;
        Ok(row.0 as u64)
    }

    /// Persists an enveloped Tenant Data Encryption Key (TDEK) wrapped under the master KEK into PostgreSQL.
    pub async fn persist_wrapped_tdek(
        &self,
        tenant_id: &str,
        wrapped_tdek: &[u8],
        kms_provider: &str,
    ) -> Result<()> {
        {
            let mut fb = self.fallback_tdeks.write().await;
            fb.insert(
                tenant_id.to_string(),
                (wrapped_tdek.to_vec(), kms_provider.to_string()),
            );
        }

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

    /// Fetches one persisted wrapped TDEK, if the tenant has already been provisioned.
    pub async fn fetch_wrapped_tdek(&self, tenant_id: &str) -> Result<Option<(Vec<u8>, String)>> {
        if let Some(pool) = self.get_pool().await {
            let row: std::result::Result<Option<(Vec<u8>, String)>, sqlx::Error> = sqlx::query_as(
                "SELECT wrapped_tdek, kms_provider FROM tenant_wrapped_keys WHERE tenant_id = $1",
            )
            .bind(tenant_id)
            .fetch_optional(&pool)
            .await;
            match row {
                Ok(record) => return Ok(record),
                Err(e) => {
                    tracing::warn!(error = %e, "Failed to query wrapped TDEK from PostgreSQL");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "Failed to query wrapped TDEK: {e}"
                        )));
                    }
                }
            }
        }
        Ok(self.fallback_tdeks.read().await.get(tenant_id).cloned())
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
        let fb = self.fallback_tdeks.read().await;
        Ok(fb
            .iter()
            .map(|(t, (w, k))| (t.clone(), w.clone(), k.clone()))
            .collect())
    }

    /// Persists an API credential record into the durable PostgreSQL `api_credentials` table.
    pub async fn persist_api_credential(&self, credential: &NewApiCredentialRecord) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let models_json = serde_json::json!(credential.allowed_models);
            let res = sqlx::query(
                "INSERT INTO api_credentials (key_hash, tenant_id, application_id, rate_limit_rps, allowed_models, is_admin, status, created_at, expires_at, revoked_at)
                 VALUES ($1, $2, $3, $4, $5, $6, 'active', NOW(), $7, NULL)
                 ON CONFLICT (key_hash) DO UPDATE SET
                    tenant_id = $2,
                    application_id = $3,
                    rate_limit_rps = $4,
                    allowed_models = $5,
                    is_admin = $6,
                    status = 'active',
                    expires_at = $7,
                    revoked_at = NULL",
            )
            .bind(&credential.key_hash)
            .bind(&credential.tenant_id)
            .bind(&credential.application_id)
            .bind(credential.rate_limit_rps)
            .bind(models_json)
            .bind(credential.is_admin)
            .bind(credential.expires_at)
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
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "Cannot persist API credential: PostgreSQL is unavailable",
            ));
        }
        Ok(())
    }

    /// Fetches all active durable API credentials from PostgreSQL for registry hydration.
    pub async fn fetch_all_api_credentials(&self) -> Result<Vec<DurableApiCredentialRecord>> {
        if let Some(pool) = self.get_pool().await {
            let query = "SELECT key_hash, tenant_id, application_id, rate_limit_rps, allowed_models, is_admin, status, created_at, expires_at FROM api_credentials WHERE status = 'active' AND (expires_at IS NULL OR expires_at > NOW())";
            let rows = sqlx::query(query).fetch_all(&pool).await;

            match rows {
                Ok(records) => {
                    let mut out = Vec::with_capacity(records.len());
                    for row in records {
                        let models_val: serde_json::Value = row.get("allowed_models");
                        let allowed_models: Vec<String> =
                            serde_json::from_value(models_val).unwrap_or_default();
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
                            expires_at: row.get("expires_at"),
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
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "Cannot fetch API credentials: PostgreSQL is unavailable",
            ));
        }
        Ok(Vec::new())
    }

    /// Revokes an API credential in the durable PostgreSQL store by marking status='revoked'.
    pub async fn revoke_api_credential(&self, key_hash: &str) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let res =
                sqlx::query("UPDATE api_credentials SET status = 'revoked', revoked_at = NOW() WHERE key_hash = $1")
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
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "Cannot revoke API credential: PostgreSQL is unavailable",
            ));
        }
        Ok(())
    }

    /// Revokes all active API credentials for a specific tenant in PostgreSQL.
    pub async fn revoke_tenant_credentials(&self, tenant_id: &str) -> Result<u64> {
        if let Some(pool) = self.get_pool().await {
            let res = sqlx::query(
                "UPDATE api_credentials SET status = 'revoked', revoked_at = NOW() WHERE tenant_id = $1 AND status != 'revoked'",
            )
            .bind(tenant_id)
            .execute(&pool)
            .await;

            match res {
                Ok(result) => Ok(result.rows_affected()),
                Err(e) => {
                    tracing::error!(error = %e, "Failed to revoke tenant API credentials in PostgreSQL");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "Failed to revoke tenant API credentials: {e}"
                        )));
                    }
                    Ok(0)
                }
            }
        } else if self.strict_durability {
            Err(sunborn_core::SunbornError::database(
                "Cannot revoke tenant API credentials: PostgreSQL is unavailable",
            ))
        } else {
            Ok(0)
        }
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
                 (key, tenant_id, application_id, model, policy_version, hash, prompt_canonical, content, hit_count, created_at, expires_at, last_accessed_at, certificate_json)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
                 ON CONFLICT (key) DO UPDATE SET hit_count = durable_cache_entries.hit_count + 1, last_accessed_at = $12, certificate_json = COALESCE($13, durable_cache_entries.certificate_json)"
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
            .bind(&entry.certificate_json)
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
                "SELECT key, tenant_id, application_id, model, policy_version, hash, prompt_canonical, content, hit_count, created_at, expires_at, last_accessed_at, certificate_json
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
                        certificate_json: row.get("certificate_json"),
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

    async fn insert_outbox_event(&self, event: OutboxRecord) -> Result<i64> {
        if let Some(pool) = self.get_pool().await {
            let res = sqlx::query_as::<_, (i64,)>(
                "INSERT INTO outbox_events (tenant_id, entity_type, entity_id, action, payload, status, created_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7) RETURNING id"
            )
            .bind(&event.tenant_id)
            .bind(&event.entity_type)
            .bind(&event.entity_id)
            .bind(&event.action)
            .bind(&event.payload)
            .bind(&event.status)
            .bind(event.created_at)
            .fetch_one(&pool)
            .await;

            match res {
                Ok((id,)) => {
                    let _ = self.fallback_store.insert_outbox_event(event).await;
                    return Ok(id);
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

    async fn claim_pending_outbox_events(
        &self,
        limit: usize,
        worker_id: &str,
    ) -> Result<Vec<OutboxRecord>> {
        if let Some(pool) = self.get_pool().await {
            let res = sqlx::query(
                "WITH claimed AS (
                     SELECT id FROM outbox_events
                     WHERE status = 'PENDING'
                        OR (status = 'PROCESSING' AND claimed_at < NOW() - INTERVAL '5 minutes')
                     ORDER BY id
                     LIMIT $1
                     FOR UPDATE SKIP LOCKED
                 )
                 UPDATE outbox_events o
                 SET status = 'PROCESSING', claimed_by = $2, claimed_at = NOW()
                 FROM claimed
                 WHERE o.id = claimed.id
                 RETURNING o.id, o.tenant_id, o.entity_type, o.entity_id, o.action, o.payload, o.status, o.created_at",
            )
            .bind(limit as i64)
            .bind(worker_id)
            .fetch_all(&pool)
            .await;

            match res {
                Ok(rows) => {
                    return Ok(rows
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
                        .collect());
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL claim_pending_outbox_events failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL claim_pending_outbox_events failed: {err}"
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        self.fallback_store
            .claim_pending_outbox_events(limit, worker_id)
            .await
    }

    async fn mark_outbox_processed(&self, id: i64) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let res = sqlx::query(
                "UPDATE outbox_events SET status = 'PROCESSED', processed_at = NOW()
                 WHERE id = $1 AND status = 'PROCESSING'",
            )
            .bind(id)
            .execute(&pool)
            .await;
            if let Err(err) = res {
                tracing::error!(error = %err, "PostgreSQL mark_outbox_processed failed");
                if self.strict_durability {
                    return Err(sunborn_core::SunbornError::database(format!(
                        "PostgreSQL mark_outbox_processed failed: {err}"
                    )));
                }
            }
        }
        self.fallback_store.mark_outbox_processed(id).await
    }

    async fn mark_outbox_failed(&self, id: i64, error: &str) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let mut tx = pool
                .begin()
                .await
                .map_err(|e| SunbornError::database(e.to_string()))?;
            let row = sqlx::query(
                "UPDATE outbox_events
                 SET retry_count = retry_count + 1,
                     status = CASE WHEN retry_count + 1 >= max_retries THEN 'FAILED' ELSE 'PENDING' END,
                     claimed_by = NULL,
                     claimed_at = NULL,
                     last_error = $2
                 WHERE id = $1
                 RETURNING retry_count, status, tenant_id, entity_type, entity_id, payload",
            )
            .bind(id)
            .bind(error)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| SunbornError::database(e.to_string()))?;

            if let Some(r) = row {
                let status: String = r.get("status");
                if status == "FAILED" {
                    sqlx::query(
                        "INSERT INTO outbox_dlq (outbox_id, tenant_id, entity_type, entity_id, payload, error_message, retry_count)
                         VALUES ($1, $2, $3, $4, $5, $6, $7)",
                    )
                    .bind(id)
                    .bind(r.get::<String, _>("tenant_id"))
                    .bind(r.get::<String, _>("entity_type"))
                    .bind(r.get::<String, _>("entity_id"))
                    .bind(r.get::<serde_json::Value, _>("payload"))
                    .bind(error)
                    .bind(r.get::<i32, _>("retry_count"))
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| SunbornError::database(e.to_string()))?;
                }
            }
            tx.commit()
                .await
                .map_err(|e| SunbornError::database(e.to_string()))?;
        }
        self.fallback_store.mark_outbox_failed(id, error).await
    }

    async fn replay_outbox_dlq(&self, limit: usize) -> Result<usize> {
        if let Some(pool) = self.get_pool().await {
            let rows = sqlx::query(
                "UPDATE outbox_events
                 SET status = 'PENDING', retry_count = 0, claimed_by = NULL,
                     claimed_at = NULL, last_error = NULL
                 WHERE id IN (
                     SELECT id FROM outbox_events WHERE status = 'FAILED'
                     ORDER BY id LIMIT $1
                 )
                 RETURNING id",
            )
            .bind(limit as i64)
            .fetch_all(&pool)
            .await
            .map_err(|e| SunbornError::database(e.to_string()))?;
            return Ok(rows.len());
        }
        self.fallback_store.replay_outbox_dlq(limit).await
    }

    async fn load_all_epochs(&self) -> Result<std::collections::HashMap<Uuid, u64>> {
        let pool = self.get_pool().await.ok_or_else(|| {
            SunbornError::database("PostgreSQL epoch authority unavailable; cannot load epochs")
        })?;
        let rows = sqlx::query_as::<_, (Uuid, i64)>(
            "SELECT source_id, current_epoch FROM tenant_source_epochs",
        )
        .fetch_all(&pool)
        .await
        .map_err(|e| SunbornError::database(e.to_string()))?;
        Ok(rows.into_iter().map(|(src, ep)| (src, ep as u64)).collect())
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

    async fn record_finops_ledger(&self, record: FinOpsLedgerRecord) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let res = sqlx::query(
                "INSERT INTO finops_ledger 
                 (tenant_id, application_id, model, cache_tier, prompt_tokens_avoided, completion_tokens_avoided, cost_avoided_usd, storage_cost_usd, net_savings_usd, recorded_at)
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)",
            )
            .bind(&record.tenant_id)
            .bind(&record.application_id)
            .bind(&record.model)
            .bind(&record.cache_tier)
            .bind(record.prompt_tokens_avoided)
            .bind(record.completion_tokens_avoided)
            .bind(record.cost_avoided_usd)
            .bind(record.storage_cost_usd)
            .bind(record.net_savings_usd)
            .bind(record.recorded_at)
            .execute(&pool)
            .await;

            match res {
                Ok(_) => {
                    let _ = self.fallback_store.record_finops_ledger(record).await;
                    return Ok(());
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL record_finops_ledger failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL record_finops_ledger failed: {err}"
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }

        self.fallback_store.record_finops_ledger(record).await
    }

    async fn scan_cache_entries(
        &self,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<DurableCacheRecord>> {
        if let Some(pool) = self.get_pool().await {
            match sqlx::query(
                "SELECT key, tenant_id, application_id, model, policy_version, hash, prompt_canonical, content, hit_count, created_at, expires_at, last_accessed_at, certificate_json
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
                                certificate_json: row.get("certificate_json"),
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
            // 1. Delete durable cache entries
            let cache_res = sqlx::query("DELETE FROM durable_cache_entries WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await
                .map_err(|err| {
                    tracing::error!(error = %err, "PostgreSQL delete durable_cache_entries failed");
                    SunbornError::database(format!(
                        "PostgreSQL delete durable_cache_entries failed: {err}"
                    ))
                })?;
            pg_deleted += cache_res.rows_affected() as usize;

            // 2. Delete tenant wrapped keys (cryptographic shredding at rest)
            let keys_res = sqlx::query("DELETE FROM tenant_wrapped_keys WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await
                .map_err(|err| {
                    tracing::error!(error = %err, "PostgreSQL delete tenant_wrapped_keys failed");
                    SunbornError::database(format!(
                        "PostgreSQL delete tenant_wrapped_keys failed: {err}"
                    ))
                })?;
            pg_deleted += keys_res.rows_affected() as usize;

            // 3. Delete tenant API credentials
            let creds_res = sqlx::query("DELETE FROM api_credentials WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await
                .map_err(|err| {
                    tracing::error!(error = %err, "PostgreSQL delete api_credentials failed");
                    SunbornError::database(format!(
                        "PostgreSQL delete api_credentials failed: {err}"
                    ))
                })?;
            pg_deleted += creds_res.rows_affected() as usize;

            // 4. Delete tenant root record
            let tenant_res = sqlx::query("DELETE FROM tenants WHERE id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await
                .map_err(|err| {
                    tracing::error!(error = %err, "PostgreSQL delete tenants failed");
                    SunbornError::database(format!("PostgreSQL delete tenants failed: {err}"))
                })?;
            pg_deleted += tenant_res.rows_affected() as usize;

            // 5. Delete tenant FinOps ledger records
            let _ = sqlx::query("DELETE FROM finops_ledger WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await;

            // 6. Delete tenant outbox events
            let _ = sqlx::query("DELETE FROM outbox_events WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await;

            // 7. Delete tenant decision records
            let _ = sqlx::query("DELETE FROM decision_records WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await;

            // 8. Delete tenant knowledge sources (cascades to document_versions and dependency_edges)
            let _ = sqlx::query("DELETE FROM knowledge_sources WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await;

            // 9. Delete tenant applications
            let _ = sqlx::query("DELETE FROM applications WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await;

            // 10. Delete tenant invalidation outbox
            let _ = sqlx::query("DELETE FROM invalidation_outbox WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await;
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        {
            let mut fb = self.fallback_tdeks.write().await;
            fb.remove(tenant_id);
        }
        let mem_deleted = self.fallback_store.delete_tenant_data(tenant_id).await?;
        Ok(pg_deleted.max(mem_deleted))
    }

    async fn count_tenant_entries(&self, tenant_id: &str) -> Result<usize> {
        let mut pg_count = 0;
        if let Some(pool) = self.get_pool().await {
            // Count cache entries
            let cache_row: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM durable_cache_entries WHERE tenant_id = $1")
                    .bind(tenant_id)
                    .fetch_one(&pool)
                    .await
                    .map_err(|err| {
                        SunbornError::database(format!(
                            "PostgreSQL count durable_cache_entries failed: {err}"
                        ))
                    })?;
            pg_count += cache_row.0 as usize;

            // Count tenant records
            let tenant_row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM tenants WHERE id = $1")
                .bind(tenant_id)
                .fetch_one(&pool)
                .await
                .map_err(|err| {
                    SunbornError::database(format!("PostgreSQL count tenants failed: {err}"))
                })?;
            pg_count += tenant_row.0 as usize;

            // Count wrapped keys
            let keys_row: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM tenant_wrapped_keys WHERE tenant_id = $1")
                    .bind(tenant_id)
                    .fetch_one(&pool)
                    .await
                    .map_err(|err| {
                        SunbornError::database(format!(
                            "PostgreSQL count tenant_wrapped_keys failed: {err}"
                        ))
                    })?;
            pg_count += keys_row.0 as usize;

            // Count active credentials
            let creds_row: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM api_credentials WHERE tenant_id = $1")
                    .bind(tenant_id)
                    .fetch_one(&pool)
                    .await
                    .map_err(|err| {
                        SunbornError::database(format!(
                            "PostgreSQL count api_credentials failed: {err}"
                        ))
                    })?;
            pg_count += creds_row.0 as usize;
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        let mem_count = self.fallback_store.count_tenant_entries(tenant_id).await?;
        Ok(pg_count.max(mem_count))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_circuit_breaker_transitions() {
        let mut cb = DbCircuitBreaker::new(3, 1);
        assert_eq!(cb.state(), DbCircuitState::Closed);

        cb.record_failure();
        cb.record_failure();
        assert_eq!(cb.state(), DbCircuitState::Closed);

        // 3rd failure trips the breaker to Open
        cb.record_failure();
        assert_eq!(cb.state(), DbCircuitState::Open);

        // Reset restores Closed
        cb.reset();
        assert_eq!(cb.state(), DbCircuitState::Closed);
    }

    #[tokio::test]
    async fn test_postgres_store_circuit_breaker_fast_fail() {
        // Points to unreachable port so connection fails immediately
        let store = PostgresDatabaseStore::new(
            "postgres://invalid_user:invalid_pass@127.0.0.1:54321/nonexistent",
        );

        assert_eq!(store.circuit_state().await, DbCircuitState::Closed);

        // Trigger failures until circuit trips
        for _ in 0..3 {
            let _ = store.get_pool().await;
        }

        assert_eq!(store.circuit_state().await, DbCircuitState::Open);

        // Next call must fast-fail without waiting
        let start = std::time::Instant::now();
        let pool = store.get_pool().await;
        let elapsed = start.elapsed();

        assert!(pool.is_none());
        assert!(
            elapsed < Duration::from_millis(50),
            "Circuit breaker must fail fast under 50ms"
        );
    }
}
