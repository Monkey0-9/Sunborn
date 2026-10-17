use crate::store::{CacheEntry, ExactCacheStore};
use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use sunborn_core::Result;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitStatus {
    Closed,
    Open,
    HalfOpen,
}

#[derive(Debug)]
struct CircuitStateInner {
    status: CircuitStatus,
    opened_at: Option<Instant>,
}

#[derive(Clone)]
pub struct CircuitBreakerCache<T: ExactCacheStore> {
    inner: T,
    state: Arc<RwLock<CircuitStateInner>>,
    consecutive_failures: Arc<AtomicU32>,
    failure_threshold: u32,
    recovery_timeout: Duration,
}

impl<T: ExactCacheStore> CircuitBreakerCache<T> {
    pub fn new(inner: T, failure_threshold: u32, recovery_timeout: Duration) -> Self {
        Self {
            inner,
            state: Arc::new(RwLock::new(CircuitStateInner {
                status: CircuitStatus::Closed,
                opened_at: None,
            })),
            consecutive_failures: Arc::new(AtomicU32::new(0)),
            failure_threshold,
            recovery_timeout,
        }
    }

    pub async fn status(&self) -> CircuitStatus {
        let mut state = self.state.write().await;
        if state.status == CircuitStatus::Open {
            if let Some(opened_at) = state.opened_at {
                if opened_at.elapsed() >= self.recovery_timeout {
                    state.status = CircuitStatus::HalfOpen;
                }
            }
        }
        state.status
    }

    pub async fn trip(&self) {
        let mut state = self.state.write().await;
        state.status = CircuitStatus::Open;
        state.opened_at = Some(Instant::now());
        self.consecutive_failures
            .store(self.failure_threshold, Ordering::SeqCst);
    }

    pub async fn reset(&self) {
        let mut state = self.state.write().await;
        state.status = CircuitStatus::Closed;
        state.opened_at = None;
        self.consecutive_failures.store(0, Ordering::SeqCst);
    }

    async fn record_success(&self) {
        let mut state = self.state.write().await;
        state.status = CircuitStatus::Closed;
        state.opened_at = None;
        self.consecutive_failures.store(0, Ordering::SeqCst);
    }

    async fn record_failure(&self) {
        let failures = self.consecutive_failures.fetch_add(1, Ordering::SeqCst) + 1;
        if failures >= self.failure_threshold {
            let mut state = self.state.write().await;
            state.status = CircuitStatus::Open;
            state.opened_at = Some(Instant::now());
        }
    }
}

#[async_trait]
impl<T: ExactCacheStore + Send + Sync> ExactCacheStore for CircuitBreakerCache<T> {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
        let status = self.status().await;
        if status == CircuitStatus::Open {
            // Fail-safe graceful degradation: return None so origin handles request
            return Ok(None);
        }

        match self.inner.get(key).await {
            Ok(entry) => {
                if status == CircuitStatus::HalfOpen {
                    self.record_success().await;
                }
                Ok(entry)
            }
            Err(_err) => {
                self.record_failure().await;
                // Graceful fallback on cache error: return None rather than failing the request
                Ok(None)
            }
        }
    }

    async fn set(&self, entry: CacheEntry) -> Result<()> {
        let status = self.status().await;
        if status == CircuitStatus::Open {
            // Gracefully drop write when backend is down
            return Ok(());
        }

        match self.inner.set(entry).await {
            Ok(()) => {
                if status == CircuitStatus::HalfOpen {
                    self.record_success().await;
                }
                Ok(())
            }
            Err(_err) => {
                self.record_failure().await;
                Ok(())
            }
        }
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        let status = self.status().await;
        if status == CircuitStatus::Open {
            return Ok(false);
        }

        match self.inner.delete(key).await {
            Ok(deleted) => {
                if status == CircuitStatus::HalfOpen {
                    self.record_success().await;
                }
                Ok(deleted)
            }
            Err(_err) => {
                self.record_failure().await;
                Ok(false)
            }
        }
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        let status = self.status().await;
        if status == CircuitStatus::Open {
            return Ok(false);
        }

        match self.inner.exists(key).await {
            Ok(exists) => {
                if status == CircuitStatus::HalfOpen {
                    self.record_success().await;
                }
                Ok(exists)
            }
            Err(_err) => {
                self.record_failure().await;
                Ok(false)
            }
        }
    }

    async fn delete_by_prefix(&self, prefix: &str) -> Result<usize> {
        let status = self.status().await;
        if status == CircuitStatus::Open {
            return Ok(0);
        }
        self.inner.delete_by_prefix(prefix).await
    }

    async fn count_by_prefix(&self, prefix: &str) -> Result<usize> {
        let status = self.status().await;
        if status == CircuitStatus::Open {
            return Ok(0);
        }
        self.inner.count_by_prefix(prefix).await
    }

    async fn sweep_expired(&self) -> Result<usize> {
        let status = self.status().await;
        if status == CircuitStatus::Open {
            return Ok(0);
        }
        self.inner.sweep_expired().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MemoryExactCache;

    #[tokio::test]
    async fn test_circuit_breaker_trip_and_fallback() {
        let inner = MemoryExactCache::new();
        let cb = CircuitBreakerCache::new(inner, 2, Duration::from_millis(50));

        assert_eq!(cb.status().await, CircuitStatus::Closed);

        // Manually trip the circuit
        cb.trip().await;
        assert_eq!(cb.status().await, CircuitStatus::Open);

        // Get should return None gracefully during outage
        let res = cb.get("any_key").await.unwrap();
        assert!(res.is_none());

        // Wait for recovery timeout
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert_eq!(cb.status().await, CircuitStatus::HalfOpen);

        // Successful call resets to Closed
        let entry = CacheEntry::new("key1", "val1", Some(60));
        cb.set(entry).await.unwrap();
        let retrieved = cb.get("key1").await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(cb.status().await, CircuitStatus::Closed);
    }
}
