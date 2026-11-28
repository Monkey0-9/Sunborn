use crate::models::{
    AuditRecord, DurableApiCredentialRecord, DurableCacheRecord, FinOpsLedgerRecord,
    FreePoolLimits, NewApiCredentialRecord, OutboxRecord, TenantErasureTaskRecord,
    TenantQuotaLimits, TenantRecord, TenantTierAssignment,
};
use crate::repository::{DatabaseStore, MemoryDatabaseStore};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
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

/// Converts a USD amount into authoritative integer micro-units.
///
/// Money must never accumulate through binary floating point: `0.1 + 0.2` is not
/// `0.3` in IEEE-754, and a ledger that sums floats drifts by real, billable
/// amounts. The conversion happens exactly once, at the boundary where a price
/// estimate enters the system, and every authoritative comparison and
/// aggregation afterwards is `BIGINT` arithmetic.
///
/// Returns `None` for non-finite, negative, or implausibly large amounts so the
/// caller can fail closed instead of recording a corrupt charge.
fn usd_to_micro_units(usd: f64) -> Option<i64> {
    const MAX_USD: f64 = 100_000.0;
    if !usd.is_finite() || !(0.0..=MAX_USD).contains(&usd) {
        return None;
    }
    Some((usd * 1_000_000.0).round() as i64)
}

/// Renders integer micro-units as USD for display and API responses only.
#[inline]
fn micro_units_to_usd(micro: i64) -> f64 {
    micro as f64 / 1_000_000.0
}

type FallbackTdekMap = Arc<RwLock<std::collections::HashMap<String, (Vec<u8>, String)>>>;

const MIGRATION_0001: &str = include_str!("../../../migrations/0001_initial_schema.sql");
const MIGRATION_0002: &str = include_str!("../../../migrations/0002_durable_identity_and_keys.sql");
const MIGRATION_0003: &str = include_str!("../../../migrations/0003_finops_and_security.sql");
const MIGRATION_0004: &str = include_str!("../../../migrations/0004_durable_cache_certificate.sql");
const MIGRATION_0005: &str = include_str!("../../../migrations/0005_persistent_epochs_and_dlq.sql");
const MIGRATION_0006: &str = include_str!("../../../migrations/0006_durable_outbox_queue.sql");
const MIGRATION_0007: &str = include_str!("../../../migrations/0007_distributed_spend_ledger.sql");
const MIGRATION_0008: &str = include_str!("../../../migrations/0008_free_tier_quotas.sql");
const MIGRATION_0009: &str = include_str!("../../../migrations/0009_cluster_rate_limits.sql");
const MIGRATION_0010: &str =
    include_str!("../../../migrations/0010_financial_precision_micro_units.sql");
const MIGRATION_0011: &str =
    include_str!("../../../migrations/0011_micro_units_non_negative_checks.sql");
const MIGRATION_0012: &str =
    include_str!("../../../migrations/0012_durable_erasure_and_learning.sql");
const MIGRATION_0013: &str = include_str!("../../../migrations/0013_tenant_tier_defaults.sql");

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

/// Outcome of a cluster-wide daily/monthly quota reservation attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaReservationOutcome {
    /// The request fits every enrolled limit and was counted.
    Granted { daily_used: u64, monthly_used: u64 },
    /// The tenant is not enrolled in quota enforcement; admission is unchanged.
    NotEnrolled,
    /// A tenant or free-pool limit would be exceeded.
    Denied {
        scope: String,
        window: String,
        limit: u64,
        used: u64,
        retry_after_secs: u64,
    },
    /// The authoritative store could not be reached. Callers must fail closed.
    StoreUnavailable,
}

/// Outcome of a cluster-wide per-tenant token-bucket reservation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitOutcome {
    /// The request consumed one token.
    Allowed {
        limit: u64,
        remaining: u64,
        reset_secs: u64,
    },
    /// The bucket is empty.
    Denied { limit: u64, retry_after_secs: u64 },
    /// The tenant has no quota row, so the RPS bucket does not apply.
    NotEnrolled,
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

type FallbackInvitationList = Arc<RwLock<Vec<(String, String, DateTime<Utc>)>>>;

/// Production PostgreSQL database store adapter implementing DatabaseStore
#[derive(Clone)]
pub struct PostgresDatabaseStore {
    pub connection_url: String,
    pool: Arc<RwLock<Option<PgPool>>>,
    pool_init_lock: Arc<tokio::sync::Mutex<()>>,
    fallback_store: Arc<MemoryDatabaseStore>,
    fallback_tdeks: FallbackTdekMap,
    /// Invitations recorded by the in-memory fallback as `(tenant_id, inviter, at)`.
    /// Used only when no PostgreSQL pool is present, so the invitation cap behaves
    /// on a single-replica development node the way it does durably. Keyed by tenant
    /// so re-provisioning cannot inflate the count; the durable path counts rows.
    fallback_invitations: FallbackInvitationList,
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
            fallback_invitations: Arc::new(RwLock::new(Vec::new())),
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
            fallback_invitations: Arc::new(RwLock::new(Vec::new())),
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

        let max_connections = std::env::var("DATABASE_MAX_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(20);
        let min_connections = std::env::var("DATABASE_MIN_CONNECTIONS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(5);
        let idle_timeout = std::env::var("DATABASE_IDLE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(600));
        let max_lifetime = std::env::var("DATABASE_MAX_LIFETIME_SECS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_secs)
            .unwrap_or(Duration::from_secs(1800));

        match PgPoolOptions::new()
            .max_connections(max_connections)
            .min_connections(min_connections)
            .idle_timeout(Some(idle_timeout))
            .max_lifetime(Some(max_lifetime))
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
            (8, "0008_free_tier_quotas", MIGRATION_0008),
            (9, "0009_cluster_rate_limits", MIGRATION_0009),
            (10, "0010_financial_precision_micro_units", MIGRATION_0010),
            (11, "0011_micro_units_non_negative_checks", MIGRATION_0011),
            (12, "0012_durable_erasure_and_learning", MIGRATION_0012),
            (13, "0013_tenant_tier_defaults", MIGRATION_0013),
        ];

        for (version, name, sql) in migrations {
            let row = sqlx::query("SELECT checksum FROM _sunborn_migrations WHERE version = $1")
                .bind(version)
                .fetch_optional(pool)
                .await?;

            if let Some(existing) = row {
                let recorded_checksum: String = existing.try_get("checksum")?;
                use sha2::Digest;
                let expected_checksum = hex::encode(sha2::Sha256::digest(sql.as_bytes()));
                if recorded_checksum != expected_checksum {
                    return Err(sqlx::Error::Configuration(format!(
                        "FATAL: Database migration checksum mismatch on version {version} ({name})! \
                         Stored checksum: {recorded_checksum}, Current migration source checksum: {expected_checksum}. \
                         Authoritative database schema drift detected; startup aborted."
                    ).into()));
                }
            } else {
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
        // The integer column is the source of truth; the USD column is a
        // derived display mirror so existing readers cannot diverge.
        let ceiling_micro = usd_to_micro_units(ceiling_usd).ok_or_else(|| {
            SunbornError::invalid_payload(format!(
                "invalid spend ceiling {ceiling_usd}: must be finite, non-negative and <= 100000 USD"
            ))
        })?;
        sqlx::query(
            "INSERT INTO spend_tenant_ceilings (tenant_id, ceiling_usd, ceiling_micro_units, updated_at)
             VALUES ($1, $2, $3, NOW())
             ON CONFLICT (tenant_id) DO UPDATE
             SET ceiling_usd = EXCLUDED.ceiling_usd, ceiling_micro_units = EXCLUDED.ceiling_micro_units, updated_at = NOW()",
        )
        .bind(tenant_id)
        .bind(micro_units_to_usd(ceiling_micro))
        .bind(ceiling_micro)
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
        // Single, validated conversion point for the incoming estimate.
        let Some(estimated_micro) = usd_to_micro_units(estimated_cost) else {
            return SpendReservationOutcome::Denied {
                committed: 0.0,
                ceiling: default_ceiling,
            };
        };
        let default_ceiling_micro = match usd_to_micro_units(default_ceiling) {
            Some(m) if m >= 0 => m,
            _ => {
                tracing::error!(
                    tenant = %tenant_id,
                    ceiling = %default_ceiling,
                    "Rejecting invalid default ceiling during spend reservation"
                );
                return SpendReservationOutcome::Denied {
                    committed: 0.0,
                    ceiling: 0.0,
                };
            }
        };
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

        let ceiling_micro: i64 = match sqlx::query_scalar::<_, i64>(
            "SELECT (CASE
                 WHEN ceiling_micro_units > 0 THEN ceiling_micro_units
                 WHEN ceiling_usd > 0 THEN ROUND(ceiling_usd * 1000000)::BIGINT
                 ELSE 0::BIGINT
             END)::BIGINT FROM spend_tenant_ceilings WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await
        {
            Ok(Some(value)) => value,
            Ok(None) => default_ceiling_micro,
            Err(err) => {
                tracing::error!(error = %err, "Failed to read durable spend ceiling");
                return SpendReservationOutcome::StoreUnavailable;
            }
        };

        let committed_micro: i64 = match sqlx::query_scalar::<_, i64>(
            "SELECT (
                 COALESCE((SELECT SUM(amount_micro_units) FROM spend_ledger_entries
                           WHERE tenant_id = $1 AND recorded_at > NOW() - ($2 * INTERVAL '1 second')), 0)
               + COALESCE((SELECT SUM(amount_micro_units) FROM spend_reservations
                           WHERE tenant_id = $1 AND reserved_at > NOW() - ($2 * INTERVAL '1 second')), 0)
             )::BIGINT",
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

        let would_commit_micro = committed_micro + estimated_micro;
        if committed_micro >= ceiling_micro
            || (committed_micro > 0 && would_commit_micro > ceiling_micro)
        {
            // Rolling back also releases the advisory lock.
            return SpendReservationOutcome::Denied {
                committed: (would_commit_micro as f64) / 1_000_000.0,
                ceiling: (ceiling_micro as f64) / 1_000_000.0,
            };
        }

        let reservation_id = Uuid::new_v4();
        let insert = sqlx::query(
            "INSERT INTO spend_reservations (reservation_id, tenant_id, amount_usd, amount_micro_units, reserved_at)
             VALUES ($1, $2, $3, $4, NOW())",
        )
        .bind(reservation_id)
        .bind(tenant_id)
        .bind(estimated_cost)
        .bind(estimated_micro)
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
        // Reject invalid financial input strictly (negative, non-finite, NaN, or > 100k USD)
        // rather than silently converting into zero spend.
        let valid_micro = match usd_to_micro_units(actual_cost) {
            Some(m) if m >= 0 => m,
            _ => {
                tracing::error!(
                    tenant = %tenant_id,
                    reservation = %reservation_id,
                    cost = %actual_cost,
                    "Rejecting invalid financial spend settlement: negative or non-finite amount"
                );
                return None;
            }
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
            "INSERT INTO spend_ledger_entries (tenant_id, reservation_id, amount_usd, amount_micro_units, recorded_at)
             VALUES ($1, $2, $3, $4, NOW())
             ON CONFLICT (reservation_id) DO NOTHING",
        )
        .bind(tenant_id)
        .bind(reservation_id)
        .bind(micro_units_to_usd(valid_micro))
        .bind(valid_micro)
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
        let total_micro: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_micro_units), 0)::BIGINT FROM spend_ledger_entries
             WHERE tenant_id = $1 AND recorded_at > NOW() - ($2 * INTERVAL '1 second')",
        )
        .bind(tenant_id)
        .bind(window_secs)
        .fetch_one(&mut *tx)
        .await
        .ok()?;
        tx.commit().await.ok()?;
        Some((total_micro as f64) / 1_000_000.0)
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
        let total_micro: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(amount_micro_units), 0)::BIGINT FROM spend_ledger_entries
             WHERE tenant_id = $1 AND recorded_at > NOW() - ($2 * INTERVAL '1 second')",
        )
        .bind(tenant_id)
        .bind(window.as_secs_f64().max(1.0))
        .fetch_one(&pool)
        .await
        .ok()?;
        Some((total_micro as f64) / 1_000_000.0)
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

    /// Enrolls a tenant in daily/monthly quota enforcement.
    ///
    /// Enrollment is explicit: tenants without a row here are not quota-limited,
    /// so existing deployments keep their current admission behavior.
    pub async fn set_tenant_quota(&self, limits: &TenantQuotaLimits) -> Result<()> {
        let pool = self.get_pool().await.ok_or_else(|| {
            SunbornError::database("PostgreSQL connection pool unavailable for tenant quota")
        })?;
        sqlx::query(
            "INSERT INTO tenant_quota_limits (tenant_id, tier, daily_requests, monthly_requests, updated_at)
             VALUES ($1, $2, $3, $4, NOW())
             ON CONFLICT (tenant_id) DO UPDATE
             SET tier = EXCLUDED.tier,
                 daily_requests = EXCLUDED.daily_requests,
                 monthly_requests = EXCLUDED.monthly_requests,
                 updated_at = NOW()",
        )
        .bind(&limits.tenant_id)
        .bind(&limits.tier)
        .bind(limits.daily_requests as i64)
        .bind(limits.monthly_requests as i64)
        .execute(&pool)
        .await
        .map_err(|e| SunbornError::database(e.to_string()))?;
        sqlx::query("UPDATE tenants SET tier = $2 WHERE id = $1")
            .bind(&limits.tenant_id)
            .bind(&limits.tier)
            .execute(&pool)
            .await
            .map_err(|e| SunbornError::database(e.to_string()))?;
        Ok(())
    }

    /// Persists the tier defaults that provisioning or a tier change applied.
    ///
    /// The upsert creates the tenant row if it does not exist yet, because
    /// provisioning writes the TDEK and the tier in separate statements and an
    /// older tenant may predate this migration.
    ///
    /// `identity_verified` and `invited_by` are monotonic: a later tier change
    /// never revokes a completed verification, and never rewrites the original
    /// inviter that the invitation cap is counted against.
    pub async fn apply_tenant_tier(&self, assignment: &TenantTierAssignment) -> Result<()> {
        let Some(pool) = self.get_pool().await else {
            if self.strict_durability {
                return Err(SunbornError::database(
                    "PostgreSQL connection pool unavailable for tenant tier assignment",
                ));
            }
            if let Some(inviter) = assignment.invited_by.as_deref() {
                let mut invitations = self.fallback_invitations.write().await;
                let already_recorded = invitations
                    .iter()
                    .any(|(tenant, _, _)| tenant == &assignment.tenant_id);
                if !already_recorded {
                    invitations.push((
                        assignment.tenant_id.clone(),
                        inviter.to_string(),
                        Utc::now(),
                    ));
                }
            }
            return Ok(());
        };
        let res = sqlx::query(
            "INSERT INTO tenants (id, name, status, rate_limit_rps, tier, retention_days,
                                  support_level, identity_verified, invited_by,
                                  default_allowed_models, created_at)
             VALUES ($1, $1, 'active', 100, $2, $3, $4, $5, $6, $7, NOW())
             ON CONFLICT (id) DO UPDATE
             SET tier = EXCLUDED.tier,
                 retention_days = EXCLUDED.retention_days,
                 support_level = EXCLUDED.support_level,
                 identity_verified = tenants.identity_verified OR EXCLUDED.identity_verified,
                 invited_by = COALESCE(tenants.invited_by, EXCLUDED.invited_by),
                 default_allowed_models = EXCLUDED.default_allowed_models,
                 updated_at = NOW()",
        )
        .bind(&assignment.tenant_id)
        .bind(&assignment.tier)
        .bind(assignment.retention_days as i32)
        .bind(&assignment.support_level)
        .bind(assignment.identity_verified)
        .bind(assignment.invited_by.as_deref())
        .bind(serde_json::json!(assignment.allowed_models))
        .execute(&pool)
        .await;

        match res {
            Ok(_) => Ok(()),
            Err(err) => {
                tracing::error!(error = %err, "PostgreSQL apply_tenant_tier failed");
                if self.strict_durability {
                    return Err(SunbornError::database(format!(
                        "Failed to persist tenant tier assignment: {err}"
                    )));
                }
                Ok(())
            }
        }
    }

    /// Counts tenants provisioned by `inviter` since `since`, for the per-inviter
    /// invitation cap.
    ///
    /// The cap cannot be counted across replicas from memory, so when the durable
    /// store is unavailable this returns `0` and logs. Production requires the
    /// durable store (strict durability refuses startup without it), so the
    /// unenforced path exists only for single-process development.
    pub async fn count_recent_invitations(
        &self,
        inviter: &str,
        since: DateTime<Utc>,
    ) -> Result<i64> {
        let Some(pool) = self.get_pool().await else {
            // Single-replica fallback: count what provisioning recorded in memory,
            // so the cap is exercised rather than silently disabled.
            let invitations = self.fallback_invitations.read().await;
            let count = invitations
                .iter()
                .filter(|(_, who, at)| who == inviter && *at >= since)
                .count();
            return Ok(count as i64);
        };
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tenants WHERE invited_by = $1 AND created_at >= $2",
        )
        .bind(inviter)
        .bind(since)
        .fetch_one(&pool)
        .await
        .map_err(|e| SunbornError::database(e.to_string()))?;
        Ok(count)
    }

    /// The tenant's applied tier and model allowlist, if the row exists.
    ///
    /// A missing row returns `None` so callers can keep pre-migration behaviour;
    /// a malformed allowlist is an error rather than a silent `*`, because
    /// defaulting a corrupt policy to unrestricted would fail open on cost.
    pub async fn get_tenant_tier_defaults(
        &self,
        tenant_id: &str,
    ) -> Result<Option<(String, Vec<String>)>> {
        let Some(pool) = self.get_pool().await else {
            return Ok(None);
        };
        let row = sqlx::query("SELECT tier, default_allowed_models FROM tenants WHERE id = $1")
            .bind(tenant_id)
            .fetch_optional(&pool)
            .await
            .map_err(|e| SunbornError::database(e.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let tier: String = row
            .try_get("tier")
            .map_err(|e| SunbornError::database(e.to_string()))?;
        let models_value: serde_json::Value = row
            .try_get("default_allowed_models")
            .map_err(|e| SunbornError::database(e.to_string()))?;
        let models: Vec<String> = serde_json::from_value(models_value).map_err(|e| {
            SunbornError::database(format!(
                "tenant {tenant_id} has a malformed model allowlist: {e}"
            ))
        })?;
        Ok(Some((tier, models)))
    }

    /// Sets the aggregate free-pool ceiling shared by every replica.
    pub async fn set_free_pool_limits(&self, limits: &FreePoolLimits) -> Result<()> {
        let pool = self.get_pool().await.ok_or_else(|| {
            SunbornError::database("PostgreSQL connection pool unavailable for free pool")
        })?;
        sqlx::query(
            "INSERT INTO free_pool_limits (pool_id, daily_requests, monthly_requests, updated_at)
             VALUES (1, $1, $2, NOW())
             ON CONFLICT (pool_id) DO UPDATE
             SET daily_requests = EXCLUDED.daily_requests,
                 monthly_requests = EXCLUDED.monthly_requests,
                 updated_at = NOW()",
        )
        .bind(limits.daily_requests as i64)
        .bind(limits.monthly_requests as i64)
        .execute(&pool)
        .await
        .map_err(|e| SunbornError::database(e.to_string()))?;
        Ok(())
    }

    /// Atomically counts one request against the tenant's day/month windows and
    /// the aggregate free pool.
    ///
    /// The transaction takes a per-tenant advisory lock followed by the free-pool
    /// lock, so concurrent replicas cannot both observe room under a limit and
    /// both consume it. Any denied dimension rolls back every increment in the
    /// transaction, including the tenant counters.
    pub async fn reserve_quota(&self, tenant_id: &str) -> QuotaReservationOutcome {
        let Some(pool) = self.get_pool().await else {
            return QuotaReservationOutcome::StoreUnavailable;
        };
        let mut tx = match pool.begin().await {
            Ok(tx) => tx,
            Err(err) => {
                tracing::error!(error = %err, "Failed to begin quota reservation transaction");
                return QuotaReservationOutcome::StoreUnavailable;
            }
        };

        let tenant_lock = format!("quota:{tenant_id}");
        if let Err(err) = sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(&tenant_lock)
            .execute(&mut *tx)
            .await
        {
            tracing::error!(error = %err, "Failed to acquire tenant quota lock");
            return QuotaReservationOutcome::StoreUnavailable;
        }

        let limits: Option<(i64, i64)> = match sqlx::query_as(
            "SELECT daily_requests, monthly_requests FROM tenant_quota_limits WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await
        {
            Ok(limits) => limits,
            Err(err) => {
                tracing::error!(error = %err, "Failed to read tenant quota limits");
                return QuotaReservationOutcome::StoreUnavailable;
            }
        };
        let Some((daily_limit, monthly_limit)) = limits else {
            let _ = tx.rollback().await;
            return QuotaReservationOutcome::NotEnrolled;
        };

        let daily_used: Option<i64> = match sqlx::query_scalar(
            "INSERT INTO tenant_quota_usage (tenant_id, window_kind, window_start, requests)
             VALUES ($1, 'day', date_trunc('day', NOW()), 1)
             ON CONFLICT (tenant_id, window_kind, window_start)
             DO UPDATE SET requests = tenant_quota_usage.requests + 1
             WHERE tenant_quota_usage.requests < $2
             RETURNING requests",
        )
        .bind(tenant_id)
        .bind(daily_limit)
        .fetch_optional(&mut *tx)
        .await
        {
            Ok(value) => value,
            Err(err) => {
                tracing::error!(error = %err, "Failed to reserve daily tenant quota");
                return QuotaReservationOutcome::StoreUnavailable;
            }
        };
        let Some(daily_used) = daily_used else {
            let used: i64 = sqlx::query_scalar(
                "SELECT COALESCE((SELECT requests FROM tenant_quota_usage
                   WHERE tenant_id = $1 AND window_kind = 'day'
                     AND window_start = date_trunc('day', NOW())), 0)",
            )
            .bind(tenant_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(daily_limit);
            let retry_after_secs: i64 = sqlx::query_scalar(
                "SELECT GREATEST(1, EXTRACT(EPOCH FROM
                   (date_trunc('day', NOW()) + INTERVAL '1 day' - NOW()))::bigint)",
            )
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(86_400);
            let _ = tx.rollback().await;
            return QuotaReservationOutcome::Denied {
                scope: "tenant".to_string(),
                window: "day".to_string(),
                limit: daily_limit.max(0) as u64,
                used: used.max(0) as u64,
                retry_after_secs: retry_after_secs.max(1) as u64,
            };
        };

        let monthly_used: Option<i64> = match sqlx::query_scalar(
            "INSERT INTO tenant_quota_usage (tenant_id, window_kind, window_start, requests)
             VALUES ($1, 'month', date_trunc('month', NOW()), 1)
             ON CONFLICT (tenant_id, window_kind, window_start)
             DO UPDATE SET requests = tenant_quota_usage.requests + 1
             WHERE tenant_quota_usage.requests < $2
             RETURNING requests",
        )
        .bind(tenant_id)
        .bind(monthly_limit)
        .fetch_optional(&mut *tx)
        .await
        {
            Ok(value) => value,
            Err(err) => {
                tracing::error!(error = %err, "Failed to reserve monthly tenant quota");
                return QuotaReservationOutcome::StoreUnavailable;
            }
        };
        let Some(monthly_used) = monthly_used else {
            let used: i64 = sqlx::query_scalar(
                "SELECT COALESCE((SELECT requests FROM tenant_quota_usage
                   WHERE tenant_id = $1 AND window_kind = 'month'
                     AND window_start = date_trunc('month', NOW())), 0)",
            )
            .bind(tenant_id)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(monthly_limit);
            let retry_after_secs: i64 = sqlx::query_scalar(
                "SELECT GREATEST(1, EXTRACT(EPOCH FROM
                   (date_trunc('month', NOW()) + INTERVAL '1 month' - NOW()))::bigint)",
            )
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(2_592_000);
            let _ = tx.rollback().await;
            return QuotaReservationOutcome::Denied {
                scope: "tenant".to_string(),
                window: "month".to_string(),
                limit: monthly_limit.max(0) as u64,
                used: used.max(0) as u64,
                retry_after_secs: retry_after_secs.max(1) as u64,
            };
        };

        let pool_limits: Option<(i64, i64)> = match sqlx::query_as(
            "SELECT daily_requests, monthly_requests FROM free_pool_limits WHERE pool_id = 1",
        )
        .fetch_optional(&mut *tx)
        .await
        {
            Ok(limits) => limits,
            Err(err) => {
                tracing::error!(error = %err, "Failed to read free-pool limits");
                return QuotaReservationOutcome::StoreUnavailable;
            }
        };

        if let Some((pool_daily_limit, pool_monthly_limit)) = pool_limits {
            if let Err(err) = sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
                .bind("quota:free_pool")
                .execute(&mut *tx)
                .await
            {
                tracing::error!(error = %err, "Failed to acquire free-pool quota lock");
                return QuotaReservationOutcome::StoreUnavailable;
            }

            let pool_daily: Option<i64> = match sqlx::query_scalar(
                "INSERT INTO free_pool_usage (window_kind, window_start, requests)
                 VALUES ('day', date_trunc('day', NOW()), 1)
                 ON CONFLICT (window_kind, window_start)
                 DO UPDATE SET requests = free_pool_usage.requests + 1
                 WHERE free_pool_usage.requests < $1
                 RETURNING requests",
            )
            .bind(pool_daily_limit)
            .fetch_optional(&mut *tx)
            .await
            {
                Ok(value) => value,
                Err(err) => {
                    tracing::error!(error = %err, "Failed to reserve daily free-pool quota");
                    return QuotaReservationOutcome::StoreUnavailable;
                }
            };
            if pool_daily.is_none() {
                let used: i64 = sqlx::query_scalar(
                    "SELECT COALESCE((SELECT requests FROM free_pool_usage
                       WHERE window_kind = 'day' AND window_start = date_trunc('day', NOW())), 0)",
                )
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(pool_daily_limit);
                let retry_after_secs: i64 = sqlx::query_scalar(
                    "SELECT GREATEST(1, EXTRACT(EPOCH FROM
                       (date_trunc('day', NOW()) + INTERVAL '1 day' - NOW()))::bigint)",
                )
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(86_400);
                let _ = tx.rollback().await;
                return QuotaReservationOutcome::Denied {
                    scope: "free_pool".to_string(),
                    window: "day".to_string(),
                    limit: pool_daily_limit.max(0) as u64,
                    used: used.max(0) as u64,
                    retry_after_secs: retry_after_secs.max(1) as u64,
                };
            }

            let pool_monthly: Option<i64> = match sqlx::query_scalar(
                "INSERT INTO free_pool_usage (window_kind, window_start, requests)
                 VALUES ('month', date_trunc('month', NOW()), 1)
                 ON CONFLICT (window_kind, window_start)
                 DO UPDATE SET requests = free_pool_usage.requests + 1
                 WHERE free_pool_usage.requests < $1
                 RETURNING requests",
            )
            .bind(pool_monthly_limit)
            .fetch_optional(&mut *tx)
            .await
            {
                Ok(value) => value,
                Err(err) => {
                    tracing::error!(error = %err, "Failed to reserve monthly free-pool quota");
                    return QuotaReservationOutcome::StoreUnavailable;
                }
            };
            if pool_monthly.is_none() {
                let used: i64 = sqlx::query_scalar(
                    "SELECT COALESCE((SELECT requests FROM free_pool_usage
                       WHERE window_kind = 'month' AND window_start = date_trunc('month', NOW())), 0)",
                )
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(pool_monthly_limit);
                let retry_after_secs: i64 = sqlx::query_scalar(
                    "SELECT GREATEST(1, EXTRACT(EPOCH FROM
                       (date_trunc('month', NOW()) + INTERVAL '1 month' - NOW()))::bigint)",
                )
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(2_592_000);
                let _ = tx.rollback().await;
                return QuotaReservationOutcome::Denied {
                    scope: "free_pool".to_string(),
                    window: "month".to_string(),
                    limit: pool_monthly_limit.max(0) as u64,
                    used: used.max(0) as u64,
                    retry_after_secs: retry_after_secs.max(1) as u64,
                };
            }
        }

        if let Err(err) = tx.commit().await {
            tracing::error!(error = %err, "Failed to commit quota reservation");
            return QuotaReservationOutcome::StoreUnavailable;
        }
        QuotaReservationOutcome::Granted {
            daily_used: daily_used.max(0) as u64,
            monthly_used: monthly_used.max(0) as u64,
        }
    }

    /// Atomically consumes one token from the tenant's cluster-wide RPS bucket.
    ///
    /// Only quota-enrolled tenants participate, so enabling this limiter does not
    /// change admission for existing tenants. The row lock serializes concurrent
    /// reservations for the same tenant across every replica.
    pub async fn reserve_rate_limit(&self, tenant_id: &str, rps: f64) -> RateLimitOutcome {
        if rps <= 0.0 {
            return RateLimitOutcome::Allowed {
                limit: 0,
                remaining: 0,
                reset_secs: 0,
            };
        }
        let Some(pool) = self.get_pool().await else {
            return RateLimitOutcome::StoreUnavailable;
        };
        let mut tx = match pool.begin().await {
            Ok(tx) => tx,
            Err(err) => {
                tracing::error!(error = %err, "Failed to begin rate-limit transaction");
                return RateLimitOutcome::StoreUnavailable;
            }
        };

        let enrolled: Option<(i32,)> =
            match sqlx::query_as("SELECT 1 FROM tenant_quota_limits WHERE tenant_id = $1")
                .bind(tenant_id)
                .fetch_optional(&mut *tx)
                .await
            {
                Ok(row) => row,
                Err(err) => {
                    tracing::error!(error = %err, "Failed to read rate-limit enrollment");
                    return RateLimitOutcome::StoreUnavailable;
                }
            };
        if enrolled.is_none() {
            let _ = tx.rollback().await;
            return RateLimitOutcome::NotEnrolled;
        }

        // Seed a full bucket on first use. On conflict this is a no-op, and the
        // following SELECT ... FOR UPDATE waits for any in-flight reservation.
        if let Err(err) = sqlx::query(
            "INSERT INTO tenant_rate_limit_buckets (tenant_id, tokens, last_refill)
             VALUES ($1, $2, clock_timestamp())
             ON CONFLICT (tenant_id) DO NOTHING",
        )
        .bind(tenant_id)
        .bind(rps.max(1.0))
        .execute(&mut *tx)
        .await
        {
            tracing::error!(error = %err, "Failed to seed rate-limit bucket");
            return RateLimitOutcome::StoreUnavailable;
        }

        let bucket: Option<(f64, DateTime<Utc>, DateTime<Utc>)> = match sqlx::query_as(
            "SELECT tokens, last_refill, clock_timestamp() AS now
             FROM tenant_rate_limit_buckets
             WHERE tenant_id = $1
             FOR UPDATE",
        )
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await
        {
            Ok(row) => row,
            Err(err) => {
                tracing::error!(error = %err, "Failed to lock rate-limit bucket");
                return RateLimitOutcome::StoreUnavailable;
            }
        };
        let Some((stored_tokens, last_refill, now)) = bucket else {
            let _ = tx.rollback().await;
            return RateLimitOutcome::StoreUnavailable;
        };

        let capacity = rps.max(1.0);
        let elapsed = (now - last_refill).num_milliseconds().max(0) as f64 / 1000.0;
        let tokens = (stored_tokens + elapsed * rps).min(capacity);
        let limit = capacity.round() as u64;

        let (tokens, outcome) = if tokens >= 1.0 {
            let tokens = tokens - 1.0;
            let remaining = tokens.floor() as u64;
            let reset_secs = ((capacity - tokens) / rps).ceil().max(0.0) as u64;
            (
                tokens,
                RateLimitOutcome::Allowed {
                    limit,
                    remaining,
                    reset_secs,
                },
            )
        } else {
            let retry_after_secs = ((1.0 - tokens) / rps).ceil().max(1.0) as u64;
            (
                tokens,
                RateLimitOutcome::Denied {
                    limit,
                    retry_after_secs,
                },
            )
        };

        if let Err(err) = sqlx::query(
            "UPDATE tenant_rate_limit_buckets
             SET tokens = $2, last_refill = $3
             WHERE tenant_id = $1",
        )
        .bind(tenant_id)
        .bind(tokens)
        .bind(now)
        .execute(&mut *tx)
        .await
        {
            tracing::error!(error = %err, "Failed to update rate-limit bucket");
            return RateLimitOutcome::StoreUnavailable;
        }

        if let Err(err) = tx.commit().await {
            tracing::error!(error = %err, "Failed to commit rate-limit reservation");
            return RateLimitOutcome::StoreUnavailable;
        }
        outcome
    }

    /// Drops quota usage windows that cannot affect the current day or month.
    /// Called once per process startup so the table does not grow without a
    /// background job.
    pub async fn prune_quota_usage(&self) -> Result<()> {
        let pool = self.get_pool().await.ok_or_else(|| {
            SunbornError::database("PostgreSQL connection pool unavailable for quota pruning")
        })?;
        sqlx::query(
            "DELETE FROM tenant_quota_usage
             WHERE window_start < date_trunc('month', NOW()) - INTERVAL '12 months'",
        )
        .execute(&pool)
        .await
        .map_err(|e| SunbornError::database(e.to_string()))?;
        sqlx::query(
            "DELETE FROM free_pool_usage
             WHERE window_start < date_trunc('month', NOW()) - INTERVAL '12 months'",
        )
        .execute(&pool)
        .await
        .map_err(|e| SunbornError::database(e.to_string()))?;
        // An idle bucket refills to full on next use, so removing it is safe.
        sqlx::query(
            "DELETE FROM tenant_rate_limit_buckets
             WHERE last_refill < NOW() - INTERVAL '1 day'",
        )
        .execute(&pool)
        .await
        .map_err(|e| SunbornError::database(e.to_string()))?;
        Ok(())
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

    async fn failed_outbox_depth(&self) -> Result<usize> {
        if let Some(pool) = self.get_pool().await {
            let row: (i64,) =
                sqlx::query_as("SELECT COUNT(*) FROM outbox_events WHERE status = 'FAILED'")
                    .fetch_one(&pool)
                    .await
                    .map_err(|e| SunbornError::database(e.to_string()))?;
            return Ok(row.0.max(0) as usize);
        }
        self.fallback_store.failed_outbox_depth().await
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

    async fn scan_cache_entries_cursor(
        &self,
        limit: usize,
        cursor: Option<(chrono::DateTime<chrono::Utc>, String)>,
    ) -> Result<Vec<DurableCacheRecord>> {
        if let Some(pool) = self.get_pool().await {
            let res = match cursor.as_ref() {
                Some((cursor_time, cursor_key)) => {
                    sqlx::query(
                        "SELECT key, tenant_id, application_id, model, policy_version, hash, prompt_canonical, content, hit_count, created_at, expires_at, last_accessed_at, certificate_json
                         FROM durable_cache_entries
                         WHERE (created_at, key) < ($1, $2)
                         ORDER BY created_at DESC, key DESC LIMIT $3",
                    )
                    .bind(cursor_time)
                    .bind(cursor_key)
                    .bind(limit as i64)
                    .fetch_all(&pool)
                    .await
                }
                None => {
                    sqlx::query(
                        "SELECT key, tenant_id, application_id, model, policy_version, hash, prompt_canonical, content, hit_count, created_at, expires_at, last_accessed_at, certificate_json
                         FROM durable_cache_entries
                         ORDER BY created_at DESC, key DESC LIMIT $1",
                    )
                    .bind(limit as i64)
                    .fetch_all(&pool)
                    .await
                }
            };
            match res {
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
                    tracing::error!(error = %err, "PostgreSQL scan_cache_entries_cursor failed");
                    if self.strict_durability {
                        return Err(sunborn_core::SunbornError::database(format!(
                            "PostgreSQL scan_cache_entries_cursor failed: {err}"
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
            .scan_cache_entries_cursor(limit, cursor)
            .await
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

            // 5b. Delete tenant quota limits and usage windows
            let _ = sqlx::query("DELETE FROM tenant_quota_usage WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await;
            let _ = sqlx::query("DELETE FROM tenant_quota_limits WHERE tenant_id = $1")
                .bind(tenant_id)
                .execute(&pool)
                .await;
            let _ = sqlx::query("DELETE FROM tenant_rate_limit_buckets WHERE tenant_id = $1")
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

            // Count quota limits and usage windows. A failed quota delete must
            // surface as a residual rather than a clean erasure report.
            let quota_row: (i64,) = sqlx::query_as(
                "SELECT
                   (SELECT COUNT(*) FROM tenant_quota_limits WHERE tenant_id = $1)
                 + (SELECT COUNT(*) FROM tenant_quota_usage WHERE tenant_id = $1)
                 + (SELECT COUNT(*) FROM tenant_rate_limit_buckets WHERE tenant_id = $1)",
            )
            .bind(tenant_id)
            .fetch_one(&pool)
            .await
            .map_err(|err| {
                SunbornError::database(format!("PostgreSQL count tenant quota rows failed: {err}"))
            })?;
            pg_count += quota_row.0 as usize;
        } else if self.strict_durability {
            return Err(sunborn_core::SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        let mem_count = self.fallback_store.count_tenant_entries(tenant_id).await?;
        Ok(pg_count.max(mem_count))
    }

    async fn create_erasure_task(&self, tenant_id: &str) -> Result<TenantErasureTaskRecord> {
        let task_id = Uuid::new_v4();
        let now = Utc::now();
        if let Some(pool) = self.get_pool().await {
            let row = sqlx::query(
                "INSERT INTO tenant_erasure_tasks 
                 (task_id, tenant_id, current_step, status, created_at, updated_at)
                 VALUES ($1, $2, 'INITIATED', 'IN_PROGRESS', $3, $3)
                 RETURNING task_id, tenant_id, current_step, status, last_error, shred_duration_ns,
                           deleted_db_entries, deleted_exact_cache, deleted_vectors,
                           deleted_execution_tools, deleted_learning_events, verified_integrity,
                           created_at, updated_at",
            )
            .bind(task_id)
            .bind(tenant_id)
            .bind(now)
            .fetch_one(&pool)
            .await;

            match row {
                Ok(r) => {
                    let task = TenantErasureTaskRecord {
                        task_id: r.get("task_id"),
                        tenant_id: r.get("tenant_id"),
                        current_step: r.get("current_step"),
                        status: r.get("status"),
                        last_error: r.get("last_error"),
                        shred_duration_ns: r.get("shred_duration_ns"),
                        deleted_db_entries: r.get("deleted_db_entries"),
                        deleted_exact_cache: r.get("deleted_exact_cache"),
                        deleted_vectors: r.get("deleted_vectors"),
                        deleted_execution_tools: r.get("deleted_execution_tools"),
                        deleted_learning_events: r.get("deleted_learning_events"),
                        verified_integrity: r
                            .get::<Option<bool>, _>("verified_integrity")
                            .unwrap_or(false),
                        created_at: r.get("created_at"),
                        updated_at: r.get("updated_at"),
                    };
                    let _ = self.fallback_store.update_erasure_task(&task).await;
                    return Ok(task);
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL create_erasure_task failed");
                    if self.strict_durability {
                        return Err(SunbornError::database(format!(
                            "PostgreSQL create_erasure_task failed: {err}"
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        self.fallback_store.create_erasure_task(tenant_id).await
    }

    async fn update_erasure_task(&self, task: &TenantErasureTaskRecord) -> Result<()> {
        if let Some(pool) = self.get_pool().await {
            let res = sqlx::query(
                "UPDATE tenant_erasure_tasks
                 SET current_step = $2,
                     status = $3,
                     last_error = $4,
                     shred_duration_ns = $5,
                     deleted_db_entries = $6,
                     deleted_exact_cache = $7,
                     deleted_vectors = $8,
                     deleted_execution_tools = $9,
                     deleted_learning_events = $10,
                     verified_integrity = $11,
                     updated_at = NOW()
                 WHERE task_id = $1",
            )
            .bind(task.task_id)
            .bind(&task.current_step)
            .bind(&task.status)
            .bind(&task.last_error)
            .bind(task.shred_duration_ns)
            .bind(task.deleted_db_entries)
            .bind(task.deleted_exact_cache)
            .bind(task.deleted_vectors)
            .bind(task.deleted_execution_tools)
            .bind(task.deleted_learning_events)
            .bind(task.verified_integrity)
            .execute(&pool)
            .await;

            match res {
                Ok(_) => {
                    let _ = self.fallback_store.update_erasure_task(task).await;
                    return Ok(());
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL update_erasure_task failed");
                    if self.strict_durability {
                        return Err(SunbornError::database(format!(
                            "PostgreSQL update_erasure_task failed: {err}"
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        self.fallback_store.update_erasure_task(task).await
    }

    async fn get_erasure_task(&self, task_id: Uuid) -> Result<Option<TenantErasureTaskRecord>> {
        if let Some(pool) = self.get_pool().await {
            let row = sqlx::query(
                "SELECT task_id, tenant_id, current_step, status, last_error, shred_duration_ns,
                        deleted_db_entries, deleted_exact_cache, deleted_vectors,
                        deleted_execution_tools, deleted_learning_events, verified_integrity,
                        created_at, updated_at
                 FROM tenant_erasure_tasks
                 WHERE task_id = $1",
            )
            .bind(task_id)
            .fetch_optional(&pool)
            .await;

            match row {
                Ok(Some(r)) => {
                    return Ok(Some(TenantErasureTaskRecord {
                        task_id: r.get("task_id"),
                        tenant_id: r.get("tenant_id"),
                        current_step: r.get("current_step"),
                        status: r.get("status"),
                        last_error: r.get("last_error"),
                        shred_duration_ns: r.get("shred_duration_ns"),
                        deleted_db_entries: r.get("deleted_db_entries"),
                        deleted_exact_cache: r.get("deleted_exact_cache"),
                        deleted_vectors: r.get("deleted_vectors"),
                        deleted_execution_tools: r.get("deleted_execution_tools"),
                        deleted_learning_events: r.get("deleted_learning_events"),
                        verified_integrity: r
                            .get::<Option<bool>, _>("verified_integrity")
                            .unwrap_or(false),
                        created_at: r.get("created_at"),
                        updated_at: r.get("updated_at"),
                    }));
                }
                Ok(None) => return Ok(None),
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL get_erasure_task failed");
                    if self.strict_durability {
                        return Err(SunbornError::database(format!(
                            "PostgreSQL get_erasure_task failed: {err}"
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        self.fallback_store.get_erasure_task(task_id).await
    }

    async fn get_active_erasure_task(
        &self,
        tenant_id: &str,
    ) -> Result<Option<TenantErasureTaskRecord>> {
        if let Some(pool) = self.get_pool().await {
            let row = sqlx::query(
                "SELECT task_id, tenant_id, current_step, status, last_error, shred_duration_ns,
                        deleted_db_entries, deleted_exact_cache, deleted_vectors,
                        deleted_execution_tools, deleted_learning_events, verified_integrity,
                        created_at, updated_at
                 FROM tenant_erasure_tasks
                 WHERE tenant_id = $1 AND status = 'IN_PROGRESS'
                 ORDER BY created_at DESC
                 LIMIT 1",
            )
            .bind(tenant_id)
            .fetch_optional(&pool)
            .await;

            match row {
                Ok(Some(r)) => {
                    return Ok(Some(TenantErasureTaskRecord {
                        task_id: r.get("task_id"),
                        tenant_id: r.get("tenant_id"),
                        current_step: r.get("current_step"),
                        status: r.get("status"),
                        last_error: r.get("last_error"),
                        shred_duration_ns: r.get("shred_duration_ns"),
                        deleted_db_entries: r.get("deleted_db_entries"),
                        deleted_exact_cache: r.get("deleted_exact_cache"),
                        deleted_vectors: r.get("deleted_vectors"),
                        deleted_execution_tools: r.get("deleted_execution_tools"),
                        deleted_learning_events: r.get("deleted_learning_events"),
                        verified_integrity: r
                            .get::<Option<bool>, _>("verified_integrity")
                            .unwrap_or(false),
                        created_at: r.get("created_at"),
                        updated_at: r.get("updated_at"),
                    }));
                }
                Ok(None) => return Ok(None),
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL get_active_erasure_task failed");
                    if self.strict_durability {
                        return Err(SunbornError::database(format!(
                            "PostgreSQL get_active_erasure_task failed: {err}"
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        self.fallback_store.get_active_erasure_task(tenant_id).await
    }

    async fn list_incomplete_erasure_tasks(&self) -> Result<Vec<TenantErasureTaskRecord>> {
        if let Some(pool) = self.get_pool().await {
            let rows = sqlx::query(
                "SELECT task_id, tenant_id, current_step, status, last_error, shred_duration_ns,
                        deleted_db_entries, deleted_exact_cache, deleted_vectors,
                        deleted_execution_tools, deleted_learning_events, verified_integrity,
                        created_at, updated_at
                 FROM tenant_erasure_tasks
                 WHERE status IN ('IN_PROGRESS', 'FAILED')
                 ORDER BY created_at ASC",
            )
            .fetch_all(&pool)
            .await;

            match rows {
                Ok(list) => {
                    return Ok(list
                        .into_iter()
                        .map(|r| TenantErasureTaskRecord {
                            task_id: r.get("task_id"),
                            tenant_id: r.get("tenant_id"),
                            current_step: r.get("current_step"),
                            status: r.get("status"),
                            last_error: r.get("last_error"),
                            shred_duration_ns: r.get("shred_duration_ns"),
                            deleted_db_entries: r.get("deleted_db_entries"),
                            deleted_exact_cache: r.get("deleted_exact_cache"),
                            deleted_vectors: r.get("deleted_vectors"),
                            deleted_execution_tools: r.get("deleted_execution_tools"),
                            deleted_learning_events: r.get("deleted_learning_events"),
                            verified_integrity: r
                                .get::<Option<bool>, _>("verified_integrity")
                                .unwrap_or(false),
                            created_at: r.get("created_at"),
                            updated_at: r.get("updated_at"),
                        })
                        .collect());
                }
                Err(err) => {
                    tracing::error!(error = %err, "PostgreSQL list_incomplete_erasure_tasks failed");
                    if self.strict_durability {
                        return Err(SunbornError::database(format!(
                            "PostgreSQL list_incomplete_erasure_tasks failed: {err}"
                        )));
                    }
                }
            }
        } else if self.strict_durability {
            return Err(SunbornError::database(
                "PostgreSQL connection pool unavailable; fail-closed durability enforced",
            ));
        }
        self.fallback_store.list_incomplete_erasure_tasks().await
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
            elapsed < Duration::from_millis(250),
            "Circuit breaker must fail fast under 250ms"
        );
    }
}
