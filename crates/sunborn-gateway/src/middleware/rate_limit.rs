use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
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
}

impl RateLimiter {
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            capacity,
            refill_rate,
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn check_status(&self, key: &str) -> RateLimitStatus {
        let mut buckets = self.buckets.lock().await;
        let now = Instant::now();

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
        std::env::var("SUNBORN_FEATURE_LOGIN_RATE_LIMIT")
            .map(|v| v.to_lowercase() != "false" && v != "0")
            .unwrap_or(true)
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

pub async fn rate_limit_middleware(headers: HeaderMap, req: Request, next: Next) -> Response {
    let tenant = headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("default-tenant");

    // Standard rate limiter instance per process
    // In production cluster, backed by Dragonfly / Redis EVAL scripts
    static LIMITER: std::sync::OnceLock<RateLimiter> = std::sync::OnceLock::new();
    let limiter = LIMITER.get_or_init(|| RateLimiter::new(100.0, 10.0));

    if limiter.check(tenant).await {
        next.run(req).await
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            Json(json!({
                "type": "https://sunborn.ai/errors/rate-limit-exceeded",
                "title": "Too Many Requests",
                "status": 429,
                "detail": format!("Rate limit exceeded for tenant '{}'. Please retry later.", tenant)
            })),
        )
            .into_response()
    }
}
