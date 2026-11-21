use axum::{
    extract::{Request, State},
    http::{header::HeaderValue, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

#[derive(Clone)]
struct Bucket {
    tokens: f64,
    last_update: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitStatus {
    pub allowed: bool,
    pub limit: u32,
    pub remaining: u32,
    pub reset_secs: u64,
}

#[derive(Clone)]
pub struct RateLimiter {
    capacity: f64,
    refill_rate: f64, // tokens per second
    buckets: Arc<Mutex<HashMap<String, Bucket>>>,
    max_entries: usize,
}

impl RateLimiter {
    /// Default ceiling on tracked keys. The map is keyed by tenant identifiers
    /// and, for unauthenticated traffic, peer addresses: both are influenced by
    /// the caller, so an unbounded map is a memory-exhaustion vector.
    const DEFAULT_MAX_ENTRIES: usize = 8192;

    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self::new_bounded(capacity, refill_rate, Self::DEFAULT_MAX_ENTRIES)
    }

    pub fn new_bounded(capacity: f64, refill_rate: f64, max_entries: usize) -> Self {
        Self {
            capacity,
            refill_rate,
            buckets: Arc::new(Mutex::new(HashMap::new())),
            max_entries: max_entries.max(1),
        }
    }

    pub async fn check_status(&self, key: &str) -> RateLimitStatus {
        let mut buckets = self.buckets.lock().await;
        let now = Instant::now();

        if buckets.len() >= self.max_entries && !buckets.contains_key(key) {
            // Drop buckets that have refilled to capacity: they carry no state
            // worth keeping. If that frees nothing, drop the least recently
            // touched bucket so a new caller is never permanently starved and
            // the map stays bounded.
            buckets.retain(|_, bucket| {
                bucket.tokens
                    + now.duration_since(bucket.last_update).as_secs_f64() * self.refill_rate
                    < self.capacity
            });
            if buckets.len() >= self.max_entries {
                if let Some(oldest) = buckets
                    .iter()
                    .min_by(|a, b| a.1.last_update.cmp(&b.1.last_update))
                    .map(|(key, _)| key.clone())
                {
                    buckets.remove(&oldest);
                }
            }
        }

        let bucket = buckets.entry(key.to_string()).or_insert_with(|| Bucket {
            tokens: self.capacity,
            last_update: now,
        });

        let elapsed = now.duration_since(bucket.last_update).as_secs_f64();
        bucket.tokens = (bucket.tokens + elapsed * self.refill_rate).min(self.capacity);
        bucket.last_update = now;

        let limit = self.capacity.round() as u32;

        if bucket.tokens >= 1.0 {
            bucket.tokens -= 1.0;
            let remaining = bucket.tokens.floor() as u32;
            let reset_secs = if self.refill_rate > 0.0 {
                ((self.capacity - bucket.tokens) / self.refill_rate).ceil() as u64
            } else {
                0
            };
            RateLimitStatus {
                allowed: true,
                limit,
                remaining,
                reset_secs,
            }
        } else {
            let remaining = 0;
            let reset_secs = if self.refill_rate > 0.0 {
                ((1.0 - bucket.tokens) / self.refill_rate).ceil().max(1.0) as u64
            } else {
                1
            };
            RateLimitStatus {
                allowed: false,
                limit,
                remaining,
                reset_secs,
            }
        }
    }

    pub async fn check(&self, key: &str) -> bool {
        self.check_status(key).await.allowed
    }

    pub async fn reset(&self, key: &str) {
        let mut buckets = self.buckets.lock().await;
        buckets.remove(key);
    }

    pub async fn clear(&self) {
        let mut buckets = self.buckets.lock().await;
        buckets.clear();
    }
}

/// Specialized Rate Limiter for Authentication & Login Endpoints.
/// Mitigates brute-force attacks, credential stuffing, and auth service denial-of-service.
#[derive(Clone)]
pub struct LoginRateLimiter {
    limiter: RateLimiter,
}

impl Default for LoginRateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginRateLimiter {
    pub fn new() -> Self {
        let capacity = std::env::var("SUNBORN_LOGIN_RATE_LIMIT_CAPACITY")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(5.0);
        let refill_rate = std::env::var("SUNBORN_LOGIN_RATE_LIMIT_REFILL_RATE")
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .unwrap_or(1.0); // 1 token per second default

        Self {
            limiter: RateLimiter::new(capacity, refill_rate),
        }
    }

    pub fn with_limits(capacity: f64, refill_rate: f64) -> Self {
        Self {
            limiter: RateLimiter::new(capacity, refill_rate),
        }
    }

    /// Feature Flag: Check if login rate limiting is enabled.
    /// Supports dynamic kill-switch `SUNBORN_FEATURE_LOGIN_RATE_LIMIT=false` for instant rollback.
    pub fn is_feature_enabled() -> bool {
        match sunborn_core::config::parse_bool_env("SUNBORN_FEATURE_LOGIN_RATE_LIMIT", true) {
            Ok(enabled) => enabled,
            Err(err) => {
                tracing::error!(
                    error = %err,
                    "Invalid SUNBORN_FEATURE_LOGIN_RATE_LIMIT value; failing closed with rate limiting ENABLED"
                );
                true
            }
        }
    }

    pub async fn check_login_attempt(&self, client_key: &str) -> RateLimitStatus {
        if !Self::is_feature_enabled() {
            return RateLimitStatus {
                allowed: true,
                limit: 1000,
                remaining: 1000,
                reset_secs: 0,
            };
        }
        self.limiter.check_status(client_key).await
    }

    pub async fn reset(&self, client_key: &str) {
        self.limiter.reset(client_key).await;
    }

    pub async fn clear(&self) {
        self.limiter.clear().await;
    }
}

static LOGIN_LIMITER: std::sync::OnceLock<LoginRateLimiter> = std::sync::OnceLock::new();

pub fn get_login_rate_limiter() -> &'static LoginRateLimiter {
    LOGIN_LIMITER.get_or_init(LoginRateLimiter::new)
}

fn set_rate_headers(
    response: &mut Response,
    limit: u64,
    remaining: u64,
    reset_secs: u64,
    retry_after_secs: Option<u64>,
) {
    let headers = response.headers_mut();
    if let Ok(value) = HeaderValue::from_str(&limit.to_string()) {
        headers.insert("x-ratelimit-limit", value);
    }
    if let Ok(value) = HeaderValue::from_str(&remaining.to_string()) {
        headers.insert("x-ratelimit-remaining", value);
    }
    if let Ok(value) = HeaderValue::from_str(&reset_secs.to_string()) {
        headers.insert("x-ratelimit-reset", value);
    }
    if let Some(retry_after_secs) = retry_after_secs {
        if let Ok(value) = HeaderValue::from_str(&retry_after_secs.to_string()) {
            headers.insert("retry-after", value);
        }
    }
}

/// Enforces the cluster-wide, per-tenant RPS bucket for quota-enrolled tenants.
///
/// This layer must run inside the authentication layer so the tenant comes from
/// the verified credential, never from a caller header. Tenants without a quota
/// row are passed through unchanged.
/// Per-peer throttle applied *before* authentication.
///
/// [`rate_limit_middleware`] budgets per tenant and reads `SecurityContext`, which
/// `auth_middleware` inserts only after a credential has been accepted. A request
/// carrying an invalid credential is therefore rejected before it is charged to
/// any budget, leaving credential guessing unthrottled. This middleware runs
/// outside auth and charges the peer address instead, so rejected attempts still
/// cost the caller.
///
/// Known limit, stated rather than implied: the buckets are per process, so the
/// effective ceiling is the configured rate multiplied by the replica count. It is
/// a brute-force brake, not a cluster-wide quota; tenant quotas remain the
/// cluster-wide mechanism.
pub async fn pre_auth_rate_limit_middleware(req: Request, next: Next) -> Response {
    if req
        .extensions()
        .get::<crate::middleware::auth::SecurityContext>()
        .is_some()
    {
        // Authenticated: the tenant limiter owns this request. Charging it here
        // as well would double-count and halve the configured tenant budget.
        return next.run(req).await;
    }

    let Some(peer_ip) = req
        .extensions()
        .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
        .map(|info| info.0.ip())
    else {
        // Fail open when no peer address is available (in-process tests, or a
        // server started without connect-info). Refusing here would take the
        // service down over a plumbing detail; authenticated traffic is still
        // covered by the tenant limiter.
        return next.run(req).await;
    };

    // Loopback traffic (local benchmark runners, self-probes, sidecars) is trusted
    // from an anti-bruteforce perspective and must not be choked by the pre-auth limiter.
    if peer_ip.is_loopback() {
        return next.run(req).await;
    }

    // Health, readiness, metrics, and build identity are unauthenticated,
    // read-only, and polled by infrastructure (kubelet probes, Prometheus).
    // Throttling them would turn a busy node into a restart loop, so they are
    // exempt from the anti-bruteforce limiter by design.
    if is_probe_path(req.uri().path()) {
        return next.run(req).await;
    }

    let status = pre_auth_limiter().check_status(&peer_ip.to_string()).await;
    if status.allowed {
        return next.run(req).await;
    }

    let mut response = crate::handlers::AppError::from(sunborn_core::SunbornError::rate_limit(
        "Too many unauthenticated requests from this address; retry shortly".to_string(),
    ))
    .into_response();
    *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
    response.headers_mut().insert(
        axum::http::header::RETRY_AFTER,
        HeaderValue::from_str(&status.reset_secs.to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("1")),
    );
    response
}

/// Unauthenticated, read-only endpoints polled by infrastructure. These bypass
/// the pre-auth limiter so probe traffic can never be mistaken for abuse.
pub fn is_probe_path(path: &str) -> bool {
    matches!(path, "/health" | "/ready" | "/metrics" | "/version")
}

fn pre_auth_limiter() -> &'static RateLimiter {
    static LIMITER: std::sync::OnceLock<RateLimiter> = std::sync::OnceLock::new();
    LIMITER.get_or_init(|| {
        let rps = std::env::var("SUNBORN_PREAUTH_RPS")
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| *value > 0.0)
            .unwrap_or(20.0);
        let burst = std::env::var("SUNBORN_PREAUTH_BURST")
            .ok()
            .and_then(|value| value.parse::<f64>().ok())
            .filter(|value| *value > 0.0)
            .unwrap_or(rps * 5.0);
        RateLimiter::new(burst, rps)
    })
}

pub async fn rate_limit_middleware(
    State(state): State<crate::state::AppState>,
    req: Request,
    next: Next,
) -> Response {
    let tenant = match req
        .extensions()
        .get::<crate::middleware::auth::SecurityContext>()
    {
        Some(sec_ctx) => sec_ctx.tenant_id.0.clone(),
        None => return next.run(req).await,
    };

    match state.quota_ledger.reserve_rate(&tenant).await {
        Ok(None) => next.run(req).await,
        Ok(Some(status)) => {
            let mut response = next.run(req).await;
            set_rate_headers(
                &mut response,
                status.limit,
                status.remaining,
                status.reset_secs,
                None,
            );
            response
        }
        Err(crate::quota::RateLimitError::Exceeded {
            limit,
            retry_after_secs,
        }) => {
            let mut response = crate::handlers::AppError::from(sunborn_core::SunbornError::rate_limit(format!(
                "Cluster-wide rate limit of {} requests/second exceeded for tenant '{}'; retry after {}s",
                limit, tenant, retry_after_secs
            )))
            .into_response();
            *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;
            set_rate_headers(
                &mut response,
                limit,
                0,
                retry_after_secs,
                Some(retry_after_secs),
            );
            response
        }
        Err(crate::quota::RateLimitError::Unavailable(detail)) => {
            tracing::error!(tenant = %tenant, error = %detail, "Cluster rate-limit store unavailable; failing closed");
            let mut response = crate::handlers::AppError::from(
                sunborn_core::SunbornError::cache_unavailable(detail),
            )
            .into_response();
            *response.status_mut() = StatusCode::SERVICE_UNAVAILABLE;
            response
        }
    }
}

#[cfg(test)]
mod limiter_bounds_tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn map_stays_bounded_under_many_distinct_keys() {
        let limiter = RateLimiter::new_bounded(2.0, 0.0, 4);
        for index in 0..500 {
            let _ = limiter.check_status(&format!("peer-{index}")).await;
        }
        let tracked = limiter.buckets.lock().await.len();
        assert!(
            tracked <= 4,
            "tracked keys must stay within the cap, found {tracked}"
        );
    }

    #[tokio::test]
    async fn exhausted_budget_is_denied_then_refills() {
        let limiter = RateLimiter::new(2.0, 100.0);
        assert!(limiter.check("peer").await);
        assert!(limiter.check("peer").await);
        assert!(
            !limiter.check("peer").await,
            "third request must be denied with a two-token bucket"
        );
        tokio::time::sleep(Duration::from_millis(40)).await;
        assert!(
            limiter.check("peer").await,
            "bucket must refill after the configured rate elapses"
        );
    }

    #[tokio::test]
    async fn keys_are_isolated_from_each_other() {
        let limiter = RateLimiter::new(1.0, 0.0);
        assert!(limiter.check("attacker").await);
        assert!(!limiter.check("attacker").await);
        assert!(
            limiter.check("victim").await,
            "one peer exhausting its budget must not deny another peer"
        );
    }
}
