//! Cluster-wide daily/monthly request quotas and the aggregate free pool.
//!
//! Enrollment is explicit: a tenant is limited only after a quota row is
//! written. The durable path uses a PostgreSQL transaction with per-tenant and
//! free-pool advisory locks, so concurrent replicas cannot both consume the
//! last unit under a limit. Without PostgreSQL, an in-process implementation
//! keeps the same semantics for tests and single-replica development.

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sunborn_db::{
    FreePoolLimits, PostgresDatabaseStore, QuotaReservationOutcome, RateLimitOutcome,
    TenantQuotaLimits,
};
use tokio::sync::Mutex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuotaError {
    Exceeded {
        scope: String,
        window: String,
        limit: u64,
        used: u64,
        retry_after_secs: u64,
    },
    Unavailable(String),
}

#[derive(Clone)]
pub struct QuotaLedger {
    durable: Option<Arc<PostgresDatabaseStore>>,
    local: Arc<Mutex<LocalState>>,
    /// Tenants known to be unenrolled, cached briefly so opting in does not add
    /// a database round trip to every request from every other tenant.
    not_enrolled_until: Arc<Mutex<HashMap<String, Instant>>>,
    rate_limit_rps: f64,
    rate_buckets: Arc<Mutex<HashMap<String, LocalRateBucket>>>,
}

const NOT_ENROLLED_TTL: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClusterRateLimitStatus {
    pub limit: u64,
    pub remaining: u64,
    pub reset_secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitError {
    Exceeded { limit: u64, retry_after_secs: u64 },
    Unavailable(String),
}

#[derive(Default)]
struct LocalState {
    tenants: HashMap<String, LocalTenant>,
    pool: Option<LocalPool>,
}

struct LocalRateBucket {
    tokens: f64,
    last_refill: DateTime<Utc>,
}

struct LocalTenant {
    limits: TenantQuotaLimits,
    day_start: DateTime<Utc>,
    month_start: DateTime<Utc>,
    day_used: u64,
    month_used: u64,
}

struct LocalPool {
    limits: FreePoolLimits,
    day_start: DateTime<Utc>,
    month_start: DateTime<Utc>,
    day_used: u64,
    month_used: u64,
}

impl QuotaLedger {
    pub fn new(durable: Option<Arc<PostgresDatabaseStore>>) -> Self {
        let rps = std::env::var("SUNBORN_RATE_LIMIT_RPS")
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| value.is_finite())
            .unwrap_or(100.0);
        Self::with_rate_limit(durable, rps)
    }

    pub fn with_rate_limit(durable: Option<Arc<PostgresDatabaseStore>>, rps: f64) -> Self {
        Self {
            durable,
            local: Arc::new(Mutex::new(LocalState::default())),
            not_enrolled_until: Arc::new(Mutex::new(HashMap::new())),
            rate_limit_rps: rps.max(0.0),
            rate_buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn rate_limit_rps(&self) -> f64 {
        self.rate_limit_rps
    }

    /// True when admission is enforced through the shared durable store.
    pub fn is_distributed(&self) -> bool {
        self.durable.is_some()
    }

    pub fn enforcement_scope(&self) -> &'static str {
        if self.is_distributed() {
            "cluster_distributed"
        } else {
            "process_local"
        }
    }

    /// Enrolls or updates one tenant's quota. Durable first, then local, so a
    /// failed durable write never leaves the replica enforcing a limit that the
    /// rest of the cluster cannot see.
    pub async fn set_tenant_limits(&self, limits: TenantQuotaLimits) -> Result<(), QuotaError> {
        if let Some(durable) = &self.durable {
            durable
                .set_tenant_quota(&limits)
                .await
                .map_err(|err| QuotaError::Unavailable(err.to_string()))?;
        }
        self.not_enrolled_until
            .lock()
            .await
            .remove(&limits.tenant_id);
        let (day_start, month_start) = window_starts(Utc::now());
        let mut state = self.local.lock().await;
        let tenant_id = limits.tenant_id.clone();
        match state.tenants.get_mut(&tenant_id) {
            Some(tenant) => {
                tenant.limits = limits;
                if tenant.day_start != day_start {
                    tenant.day_start = day_start;
                    tenant.day_used = 0;
                }
                if tenant.month_start != month_start {
                    tenant.month_start = month_start;
                    tenant.month_used = 0;
                }
            }
            None => {
                state.tenants.insert(
                    tenant_id,
                    LocalTenant {
                        limits,
                        day_start,
                        month_start,
                        day_used: 0,
                        month_used: 0,
                    },
                );
            }
        }
        Ok(())
    }

    /// Sets or updates the aggregate free-pool ceiling.
    pub async fn set_free_pool_limits(&self, limits: FreePoolLimits) -> Result<(), QuotaError> {
        if let Some(durable) = &self.durable {
            durable
                .set_free_pool_limits(&limits)
                .await
                .map_err(|err| QuotaError::Unavailable(err.to_string()))?;
        }
        let (day_start, month_start) = window_starts(Utc::now());
        let mut state = self.local.lock().await;
        match state.pool.as_mut() {
            Some(pool) => {
                pool.limits = limits;
                if pool.day_start != day_start {
                    pool.day_start = day_start;
                    pool.day_used = 0;
                }
                if pool.month_start != month_start {
                    pool.month_start = month_start;
                    pool.month_used = 0;
                }
            }
            None => {
                state.pool = Some(LocalPool {
                    limits,
                    day_start,
                    month_start,
                    day_used: 0,
                    month_used: 0,
                });
            }
        }
        Ok(())
    }

    /// Drops local counters for a deleted tenant. Durable rows are removed by
    /// the database tenant cascade.
    pub async fn remove_tenant(&self, tenant_id: &str) {
        self.local.lock().await.tenants.remove(tenant_id);
        self.not_enrolled_until.lock().await.remove(tenant_id);
        self.rate_buckets.lock().await.remove(tenant_id);
    }

    /// Consumes one token from the tenant's cluster-wide RPS bucket.
    ///
    /// Returns `Ok(None)` when the tenant is not quota-enrolled, so enabling
    /// this limiter does not change admission for existing tenants.
    pub async fn reserve_rate(
        &self,
        tenant_id: &str,
    ) -> Result<Option<ClusterRateLimitStatus>, RateLimitError> {
        if self.rate_limit_rps <= 0.0 {
            return Ok(None);
        }
        if let Some(durable) = &self.durable {
            let now = Instant::now();
            if let Some(until) = self.not_enrolled_until.lock().await.get(tenant_id) {
                if now < *until {
                    return Ok(None);
                }
            }
            return match durable
                .reserve_rate_limit(tenant_id, self.rate_limit_rps)
                .await
            {
                RateLimitOutcome::Allowed {
                    limit,
                    remaining,
                    reset_secs,
                } => {
                    self.not_enrolled_until.lock().await.remove(tenant_id);
                    Ok(Some(ClusterRateLimitStatus {
                        limit,
                        remaining,
                        reset_secs,
                    }))
                }
                RateLimitOutcome::Denied {
                    limit,
                    retry_after_secs,
                } => Err(RateLimitError::Exceeded {
                    limit,
                    retry_after_secs,
                }),
                RateLimitOutcome::NotEnrolled => {
                    self.not_enrolled_until
                        .lock()
                        .await
                        .insert(tenant_id.to_string(), now + NOT_ENROLLED_TTL);
                    Ok(None)
                }
                RateLimitOutcome::StoreUnavailable => Err(RateLimitError::Unavailable(
                    "distributed rate-limit store unavailable; request rejected (fail-closed)"
                        .to_string(),
                )),
            };
        }
        self.reserve_rate_local(tenant_id, Utc::now()).await
    }

    async fn reserve_rate_local(
        &self,
        tenant_id: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<ClusterRateLimitStatus>, RateLimitError> {
        if self.rate_limit_rps <= 0.0 {
            return Ok(None);
        }
        {
            let state = self.local.lock().await;
            if !state.tenants.contains_key(tenant_id) {
                return Ok(None);
            }
        }

        let capacity = self.rate_limit_rps.max(1.0);
        let mut buckets = self.rate_buckets.lock().await;
        let bucket = buckets
            .entry(tenant_id.to_string())
            .or_insert(LocalRateBucket {
                tokens: capacity,
                last_refill: now,
            });
        let elapsed = (now - bucket.last_refill).num_milliseconds().max(0) as f64 / 1000.0;
        bucket.tokens = (bucket.tokens + elapsed * self.rate_limit_rps).min(capacity);
        bucket.last_refill = now;

        let limit = capacity.round() as u64;
        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            Ok(Some(ClusterRateLimitStatus {
                limit,
                remaining: bucket.tokens.floor() as u64,
                reset_secs: ((capacity - bucket.tokens) / self.rate_limit_rps)
                    .ceil()
                    .max(0.0) as u64,
            }))
        } else {
            Err(RateLimitError::Exceeded {
                limit,
                retry_after_secs: ((1.0 - bucket.tokens) / self.rate_limit_rps)
                    .ceil()
                    .max(1.0) as u64,
            })
        }
    }

    /// Counts one admitted request against every enrolled limit.
    ///
    /// A cache hit still counts as a request; only origin spend is released.
    pub async fn reserve(&self, tenant_id: &str) -> Result<(), QuotaError> {
        if let Some(durable) = &self.durable {
            let now = Instant::now();
            if let Some(until) = self.not_enrolled_until.lock().await.get(tenant_id) {
                if now < *until {
                    return Ok(());
                }
            }
            return match durable.reserve_quota(tenant_id).await {
                QuotaReservationOutcome::Granted { .. } => {
                    self.not_enrolled_until.lock().await.remove(tenant_id);
                    Ok(())
                }
                QuotaReservationOutcome::NotEnrolled => {
                    self.not_enrolled_until
                        .lock()
                        .await
                        .insert(tenant_id.to_string(), now + NOT_ENROLLED_TTL);
                    Ok(())
                }
                QuotaReservationOutcome::Denied {
                    scope,
                    window,
                    limit,
                    used,
                    retry_after_secs,
                } => Err(QuotaError::Exceeded {
                    scope,
                    window,
                    limit,
                    used,
                    retry_after_secs,
                }),
                QuotaReservationOutcome::StoreUnavailable => Err(QuotaError::Unavailable(
                    "distributed quota store unavailable; request rejected (fail-closed)"
                        .to_string(),
                )),
            };
        }
        self.reserve_local(tenant_id, Utc::now()).await
    }

    async fn reserve_local(&self, tenant_id: &str, now: DateTime<Utc>) -> Result<(), QuotaError> {
        let (day_start, month_start) = window_starts(now);
        let day_end = day_start + chrono::Duration::days(1);
        let month_end = next_month_start(month_start);

        let mut state = self.local.lock().await;

        {
            let Some(tenant) = state.tenants.get_mut(tenant_id) else {
                return Ok(());
            };
            if tenant.day_start != day_start {
                tenant.day_start = day_start;
                tenant.day_used = 0;
            }
            if tenant.month_start != month_start {
                tenant.month_start = month_start;
                tenant.month_used = 0;
            }
            if tenant.day_used >= tenant.limits.daily_requests {
                return Err(QuotaError::Exceeded {
                    scope: "tenant".to_string(),
                    window: "day".to_string(),
                    limit: tenant.limits.daily_requests,
                    used: tenant.day_used,
                    retry_after_secs: retry_after(now, day_end),
                });
            }
            if tenant.month_used >= tenant.limits.monthly_requests {
                return Err(QuotaError::Exceeded {
                    scope: "tenant".to_string(),
                    window: "month".to_string(),
                    limit: tenant.limits.monthly_requests,
                    used: tenant.month_used,
                    retry_after_secs: retry_after(now, month_end),
                });
            }
        }

        if let Some(pool) = state.pool.as_mut() {
            if pool.day_start != day_start {
                pool.day_start = day_start;
                pool.day_used = 0;
            }
            if pool.month_start != month_start {
                pool.month_start = month_start;
                pool.month_used = 0;
            }
            if pool.day_used >= pool.limits.daily_requests {
                return Err(QuotaError::Exceeded {
                    scope: "free_pool".to_string(),
                    window: "day".to_string(),
                    limit: pool.limits.daily_requests,
                    used: pool.day_used,
                    retry_after_secs: retry_after(now, day_end),
                });
            }
            if pool.month_used >= pool.limits.monthly_requests {
                return Err(QuotaError::Exceeded {
                    scope: "free_pool".to_string(),
                    window: "month".to_string(),
                    limit: pool.limits.monthly_requests,
                    used: pool.month_used,
                    retry_after_secs: retry_after(now, month_end),
                });
            }
        }

        if let Some(tenant) = state.tenants.get_mut(tenant_id) {
            tenant.day_used += 1;
            tenant.month_used += 1;
        }
        if let Some(pool) = state.pool.as_mut() {
            pool.day_used += 1;
            pool.month_used += 1;
        }
        Ok(())
    }
}

fn window_starts(now: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
    let date = now.date_naive();
    let day = Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("midnight is valid"));
    let month_date = NaiveDate::from_ymd_opt(date.year(), date.month(), 1).expect("valid month");
    let month = Utc.from_utc_datetime(&month_date.and_hms_opt(0, 0, 0).expect("midnight is valid"));
    (day, month)
}

fn next_month_start(month_start: DateTime<Utc>) -> DateTime<Utc> {
    let (year, month) = if month_start.month() == 12 {
        (month_start.year() + 1, 1)
    } else {
        (month_start.year(), month_start.month() + 1)
    };
    let date = NaiveDate::from_ymd_opt(year, month, 1).expect("valid next month");
    Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).expect("midnight is valid"))
}

fn retry_after(now: DateTime<Utc>, window_end: DateTime<Utc>) -> u64 {
    (window_end - now).num_seconds().max(1) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(year: i32, month: u32, day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, 0, 0)
            .single()
            .expect("valid timestamp")
    }

    #[tokio::test]
    async fn unenrolled_tenant_is_unchanged() {
        let ledger = QuotaLedger::new(None);
        assert!(ledger.reserve("not-enrolled").await.is_ok());
        assert!(ledger.reserve("not-enrolled").await.is_ok());
    }

    #[tokio::test]
    async fn daily_limit_is_exact_and_reports_retry() {
        let ledger = QuotaLedger::new(None);
        ledger
            .set_tenant_limits(TenantQuotaLimits::new("t1", "free", 2, 10))
            .await
            .expect("enroll");

        assert!(ledger.reserve("t1").await.is_ok());
        assert!(ledger.reserve("t1").await.is_ok());
        match ledger
            .reserve("t1")
            .await
            .expect_err("third request denied")
        {
            QuotaError::Exceeded {
                scope,
                window,
                limit,
                used,
                retry_after_secs,
            } => {
                assert_eq!(scope, "tenant");
                assert_eq!(window, "day");
                assert_eq!(limit, 2);
                assert_eq!(used, 2);
                assert!(retry_after_secs > 0 && retry_after_secs <= 86_400);
            }
            other => panic!("unexpected error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn monthly_limit_binds_across_days() {
        let ledger = QuotaLedger::new(None);
        ledger
            .set_tenant_limits(TenantQuotaLimits::new("t1", "free", 100, 2))
            .await
            .expect("enroll");

        assert!(ledger.reserve_local("t1", at(2026, 1, 1, 12)).await.is_ok());
        assert!(ledger.reserve_local("t1", at(2026, 1, 2, 12)).await.is_ok());
        match ledger
            .reserve_local("t1", at(2026, 1, 3, 12))
            .await
            .expect_err("monthly limit denied")
        {
            QuotaError::Exceeded { window, .. } => assert_eq!(window, "month"),
            other => panic!("unexpected error: {other:?}"),
        }
        assert!(ledger.reserve_local("t1", at(2026, 2, 1, 0)).await.is_ok());
    }

    #[tokio::test]
    async fn cluster_rate_bucket_is_exact_and_refills() {
        let ledger = QuotaLedger::with_rate_limit(None, 2.0);
        ledger
            .set_tenant_limits(TenantQuotaLimits::new("t1", "free", 100, 100))
            .await
            .expect("enroll");

        let t0 = at(2026, 1, 1, 12);
        let first = ledger
            .reserve_rate_local("t1", t0)
            .await
            .expect("first token")
            .expect("enrolled");
        assert_eq!(first.limit, 2);
        assert_eq!(first.remaining, 1);

        let second = ledger
            .reserve_rate_local("t1", t0)
            .await
            .expect("second token")
            .expect("enrolled");
        assert_eq!(second.remaining, 0);

        match ledger.reserve_rate_local("t1", t0).await {
            Err(RateLimitError::Exceeded {
                limit,
                retry_after_secs,
            }) => {
                assert_eq!(limit, 2);
                assert!(retry_after_secs >= 1);
            }
            other => panic!("expected rate limit, got {other:?}"),
        }

        // Half a second refills one token at 2 rps.
        let refilled = ledger
            .reserve_rate_local("t1", t0 + chrono::Duration::milliseconds(500))
            .await
            .expect("refilled token")
            .expect("enrolled");
        assert_eq!(refilled.remaining, 0);
    }

    #[tokio::test]
    async fn rate_limit_ignores_unenrolled_tenants() {
        let ledger = QuotaLedger::with_rate_limit(None, 1.0);
        let status = ledger
            .reserve_rate_local("not-enrolled", at(2026, 1, 1, 12))
            .await
            .expect("unenrolled tenants keep current admission");
        assert!(status.is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_reservations_never_exceed_limit() {
        let ledger = Arc::new(QuotaLedger::new(None));
        ledger
            .set_tenant_limits(TenantQuotaLimits::new("race", "free", 5, 100))
            .await
            .expect("enroll");

        let mut tasks = Vec::new();
        for _ in 0..40 {
            let ledger = ledger.clone();
            tasks.push(tokio::spawn(
                async move { ledger.reserve("race").await.is_ok() },
            ));
        }

        let mut admitted = 0;
        for task in tasks {
            if task.await.expect("task joins") {
                admitted += 1;
            }
        }
        assert_eq!(admitted, 5, "exactly the daily limit must be admitted");
    }

    #[tokio::test]
    async fn free_pool_denial_does_not_consume_tenant_quota() {
        let ledger = QuotaLedger::new(None);
        ledger
            .set_tenant_limits(TenantQuotaLimits::new("a", "free", 10, 10))
            .await
            .expect("enroll a");
        ledger
            .set_tenant_limits(TenantQuotaLimits::new("b", "free", 10, 10))
            .await
            .expect("enroll b");
        ledger
            .set_free_pool_limits(FreePoolLimits::new(1, 10))
            .await
            .expect("set pool");

        assert!(ledger.reserve("a").await.is_ok());
        match ledger.reserve("b").await.expect_err("pool exhausted") {
            QuotaError::Exceeded { scope, window, .. } => {
                assert_eq!(scope, "free_pool");
                assert_eq!(window, "day");
            }
            other => panic!("unexpected error: {other:?}"),
        }

        ledger
            .set_free_pool_limits(FreePoolLimits::new(10, 10))
            .await
            .expect("raise pool");
        assert!(
            ledger.reserve("b").await.is_ok(),
            "b's tenant counter must not have advanced on the pool denial"
        );
    }
}
