//! Two-phase token-aware rate limiter (RPM, RPD, TPM, TPD, and concurrency).
//!
//! Phase 1 (`pre_commit`) runs before upstream dispatch:
//! - Acquires a concurrency slot (released automatically on `Drop` if not committed)
//! - Checks and increments RPM / RPD counters
//! - Checks TPM / TPD headroom
//!
//! Phase 2 (`commit_tokens`) runs after the response lands:
//! - Charges actual `prompt_tokens + completion_tokens` to TPM / TPD
//! - Releases the concurrency slot

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenQuotaPolicy {
    pub rpm_limit: Option<u64>,
    pub rpd_limit: Option<u64>,
    pub tpm_limit: Option<u64>,
    pub tpd_limit: Option<u64>,
    pub concurrency_limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenRateLimitStatus {
    pub rpm_limit: Option<u64>,
    pub rpm_used: u64,
    pub rpm_reset_secs: u64,
    pub tpm_limit: Option<u64>,
    pub tpm_used: u64,
    pub tpm_reset_secs: u64,
    pub rpd_limit: Option<u64>,
    pub rpd_used: u64,
    pub tpd_limit: Option<u64>,
    pub tpd_used: u64,
    pub concurrency_limit: Option<u32>,
    pub in_flight: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenLimitError {
    Exceeded {
        dimension: &'static str,
        limit: u64,
        used: u64,
        retry_after_secs: u64,
    },
}

impl std::fmt::Display for TokenLimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Exceeded {
                dimension,
                limit,
                used,
                retry_after_secs,
            } => write!(
                f,
                "Token rate limit exceeded on {} ({}/{} used); retry after {}s",
                dimension, used, limit, retry_after_secs
            ),
        }
    }
}

#[derive(Debug)]
struct WindowCounter {
    count: u64,
    window_start: Instant,
    window_len: Duration,
}

impl WindowCounter {
    fn new(window_len: Duration, now: Instant) -> Self {
        Self {
            count: 0,
            window_start: now,
            window_len,
        }
    }

    fn roll_if_expired(&mut self, now: Instant) {
        if now.duration_since(self.window_start) >= self.window_len {
            self.count = 0;
            self.window_start = now;
        }
    }

    fn reset_secs(&self, now: Instant) -> u64 {
        let elapsed = now.duration_since(self.window_start);
        self.window_len.saturating_sub(elapsed).as_secs().max(1)
    }
}

#[derive(Debug)]
struct BucketState {
    rpm: WindowCounter,
    rpd: WindowCounter,
    tpm: WindowCounter,
    tpd: WindowCounter,
    in_flight: u32,
}

impl BucketState {
    fn new(now: Instant) -> Self {
        Self {
            rpm: WindowCounter::new(Duration::from_secs(60), now),
            rpd: WindowCounter::new(Duration::from_secs(86_400), now),
            tpm: WindowCounter::new(Duration::from_secs(60), now),
            tpd: WindowCounter::new(Duration::from_secs(86_400), now),
            in_flight: 0,
        }
    }

    fn roll_all(&mut self, now: Instant) {
        self.rpm.roll_if_expired(now);
        self.rpd.roll_if_expired(now);
        self.tpm.roll_if_expired(now);
        self.tpd.roll_if_expired(now);
    }
}

#[derive(Default)]
struct Inner {
    policies: HashMap<String, TokenQuotaPolicy>,
    buckets: HashMap<String, BucketState>,
}

/// Thread-safe two-phase token-aware rate limiter.
#[derive(Clone, Default)]
pub struct TokenRateLimiter {
    inner: Arc<Mutex<Inner>>,
}

/// RAII reservation guard returned by [`TokenRateLimiter::pre_commit`].
/// Automatically releases the concurrency slot on drop if `commit_tokens` is not called.
pub struct TokenReservation {
    key: String,
    limiter: TokenRateLimiter,
    committed: bool,
}

impl TokenReservation {
    /// Phase 2: commit actual token usage and release the concurrency slot.
    pub fn commit_tokens(mut self, prompt_tokens: u32, completion_tokens: u32) {
        let total = u64::from(prompt_tokens) + u64::from(completion_tokens);
        self.limiter.commit_internal(&self.key, total);
        self.committed = true;
    }
}

impl Drop for TokenReservation {
    fn drop(&mut self) {
        if !self.committed {
            self.limiter.release_concurrency_only(&self.key);
        }
    }
}

impl TokenRateLimiter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Configures or updates the token quota policy for a tenant or virtual key.
    pub fn set_policy(&self, key: impl Into<String>, policy: TokenQuotaPolicy) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.policies.insert(key.into(), policy);
    }

    pub fn get_policy(&self, key: &str) -> Option<TokenQuotaPolicy> {
        let inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        inner.policies.get(key).cloned()
    }

    /// Phase 1: check concurrency, RPM/RPD, and TPM/TPD headroom before upstream dispatch.
    pub fn pre_commit(&self, key: &str) -> Result<TokenReservation, TokenLimitError> {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let Some(policy) = inner.policies.get(key).cloned() else {
            return Ok(TokenReservation {
                key: key.to_string(),
                limiter: self.clone(),
                committed: false,
            });
        };

        let now = Instant::now();
        let bucket = inner
            .buckets
            .entry(key.to_string())
            .or_insert_with(|| BucketState::new(now));
        bucket.roll_all(now);

        if let Some(conc_limit) = policy.concurrency_limit {
            if bucket.in_flight >= conc_limit {
                return Err(TokenLimitError::Exceeded {
                    dimension: "concurrency",
                    limit: u64::from(conc_limit),
                    used: u64::from(bucket.in_flight),
                    retry_after_secs: 1,
                });
            }
        }
        if let Some(rpm_limit) = policy.rpm_limit {
            if bucket.rpm.count >= rpm_limit {
                return Err(TokenLimitError::Exceeded {
                    dimension: "rpm",
                    limit: rpm_limit,
                    used: bucket.rpm.count,
                    retry_after_secs: bucket.rpm.reset_secs(now),
                });
            }
        }
        if let Some(rpd_limit) = policy.rpd_limit {
            if bucket.rpd.count >= rpd_limit {
                return Err(TokenLimitError::Exceeded {
                    dimension: "rpd",
                    limit: rpd_limit,
                    used: bucket.rpd.count,
                    retry_after_secs: bucket.rpd.reset_secs(now),
                });
            }
        }
        if let Some(tpm_limit) = policy.tpm_limit {
            if bucket.tpm.count >= tpm_limit {
                return Err(TokenLimitError::Exceeded {
                    dimension: "tpm",
                    limit: tpm_limit,
                    used: bucket.tpm.count,
                    retry_after_secs: bucket.tpm.reset_secs(now),
                });
            }
        }
        if let Some(tpd_limit) = policy.tpd_limit {
            if bucket.tpd.count >= tpd_limit {
                return Err(TokenLimitError::Exceeded {
                    dimension: "tpd",
                    limit: tpd_limit,
                    used: bucket.tpd.count,
                    retry_after_secs: bucket.tpd.reset_secs(now),
                });
            }
        }

        bucket.in_flight = bucket.in_flight.saturating_add(1);
        bucket.rpm.count = bucket.rpm.count.saturating_add(1);
        bucket.rpd.count = bucket.rpd.count.saturating_add(1);

        Ok(TokenReservation {
            key: key.to_string(),
            limiter: self.clone(),
            committed: false,
        })
    }

    fn commit_internal(&self, key: &str, tokens: u64) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        if let Some(bucket) = inner.buckets.get_mut(key) {
            bucket.roll_all(now);
            bucket.tpm.count = bucket.tpm.count.saturating_add(tokens);
            bucket.tpd.count = bucket.tpd.count.saturating_add(tokens);
            bucket.in_flight = bucket.in_flight.saturating_sub(1);
        }
    }

    fn release_concurrency_only(&self, key: &str) {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(bucket) = inner.buckets.get_mut(key) {
            bucket.in_flight = bucket.in_flight.saturating_sub(1);
        }
    }

    pub fn status(&self, key: &str) -> TokenRateLimitStatus {
        let mut inner = self.inner.lock().unwrap_or_else(|e| e.into_inner());
        let policy = inner.policies.get(key).cloned().unwrap_or_default();
        let now = Instant::now();
        let bucket = inner
            .buckets
            .entry(key.to_string())
            .or_insert_with(|| BucketState::new(now));
        bucket.roll_all(now);
        TokenRateLimitStatus {
            rpm_limit: policy.rpm_limit,
            rpm_used: bucket.rpm.count,
            rpm_reset_secs: bucket.rpm.reset_secs(now),
            tpm_limit: policy.tpm_limit,
            tpm_used: bucket.tpm.count,
            tpm_reset_secs: bucket.tpm.reset_secs(now),
            rpd_limit: policy.rpd_limit,
            rpd_used: bucket.rpd.count,
            tpd_limit: policy.tpd_limit,
            tpd_used: bucket.tpd.count,
            concurrency_limit: policy.concurrency_limit,
            in_flight: bucket.in_flight,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_phase_tpm_and_concurrency_guard_releases_on_drop() {
        let limiter = TokenRateLimiter::new();
        limiter.set_policy(
            "tenant-a",
            TokenQuotaPolicy {
                rpm_limit: Some(10),
                rpd_limit: None,
                tpm_limit: Some(100),
                tpd_limit: None,
                concurrency_limit: Some(1),
            },
        );

        let res1 = limiter.pre_commit("tenant-a").expect("first slot");
        // Second concurrent request is blocked
        let err = limiter
            .pre_commit("tenant-a")
            .err()
            .expect("concurrency full");
        assert!(matches!(
            err,
            TokenLimitError::Exceeded {
                dimension: "concurrency",
                ..
            }
        ));

        // Dropping without commit releases concurrency slot without charging tokens
        drop(res1);

        let res2 = limiter
            .pre_commit("tenant-a")
            .expect("slot released on drop");
        res2.commit_tokens(60, 50); // 110 > 100 TPM

        let tpm_err = limiter.pre_commit("tenant-a").err().expect("tpm exceeded");
        assert!(matches!(
            tpm_err,
            TokenLimitError::Exceeded {
                dimension: "tpm",
                ..
            }
        ));
    }
}
