//! Cluster-wide spend velocity enforcement.
//!
//! [`sunborn_core::SpendVelocityController`] is process-local. That is correct
//! for a single replica but lets N replicas each spend up to the configured
//! ceiling. When the gateway is wired to PostgreSQL, reservations and
//! settlements go through [`PostgresDatabaseStore`] instead, where a per-tenant
//! advisory lock makes the check-and-reserve step atomic across replicas.
//!
//! The ledger fails closed: if the durable store is configured but unreachable,
//! requests are rejected rather than silently reverting to an unenforced,
//! process-local budget.

use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use sunborn_core::SpendVelocityController;
use sunborn_db::{PostgresDatabaseStore, SpendReservationOutcome};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum SpendLedgerError {
    /// The tenant's rolling-window commitment would exceed its ceiling.
    CeilingExceeded { committed: f64, ceiling: f64 },
    /// The durable ledger could not be reached; the request must not proceed.
    Unavailable(String),
}

/// In-memory lease of tenant budget to absorb extreme concurrency (100k+ req/sec)
/// without serializing on PostgreSQL transaction locks.
#[derive(Debug)]
pub struct BudgetLease {
    pub reservation_id: Uuid,
    pub leased_micro_usd: i64,
    pub consumed_micro_usd: AtomicI64,
    pub expires_at: Instant,
}

impl BudgetLease {
    pub fn new(reservation_id: Uuid, leased_micro_usd: i64, ttl: Duration) -> Self {
        Self {
            reservation_id,
            leased_micro_usd,
            consumed_micro_usd: AtomicI64::new(0),
            expires_at: Instant::now() + ttl,
        }
    }

    /// Attempts lock-free reservation against the active lease.
    /// Returns true if reserved successfully within the leased capacity.
    pub fn try_reserve(&self, cost_micro_usd: i64) -> bool {
        if Instant::now() >= self.expires_at {
            return false;
        }
        let mut current = self.consumed_micro_usd.load(Ordering::Relaxed);
        loop {
            if current + cost_micro_usd > self.leased_micro_usd {
                return false;
            }
            match self.consumed_micro_usd.compare_exchange_weak(
                current,
                current + cost_micro_usd,
                Ordering::SeqCst,
                Ordering::Relaxed,
            ) {
                Ok(_) => return true,
                Err(actual) => current = actual,
            }
        }
    }
}

/// Facade over the process-local controller and the optional durable ledger.
#[derive(Clone)]
pub struct SpendLedger {
    local: Arc<SpendVelocityController>,
    durable: Option<Arc<PostgresDatabaseStore>>,
    leases: Arc<RwLock<HashMap<String, Arc<BudgetLease>>>>,
}

impl SpendLedger {
    pub fn new(
        local: Arc<SpendVelocityController>,
        durable: Option<Arc<PostgresDatabaseStore>>,
    ) -> Self {
        Self {
            local,
            durable,
            leases: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// The process-local controller, retained for readiness checks and metrics.
    pub fn local(&self) -> &Arc<SpendVelocityController> {
        &self.local
    }

    /// True when spend is enforced through the shared durable ledger.
    pub fn is_distributed(&self) -> bool {
        self.durable.is_some()
    }

    /// Returns the scope label reported by observability endpoints.
    pub fn enforcement_scope(&self) -> &'static str {
        if self.is_distributed() {
            "cluster_distributed"
        } else {
            "process_local"
        }
    }

    fn window(&self) -> std::time::Duration {
        self.local.window_duration()
    }

    /// Atomically reserves estimated budget for a tenant before origin dispatch.
    /// Uses adaptive in-memory budget leasing under high throughput to eliminate
    /// database row-lock contention while preserving strict fail-closed ceilings.
    pub async fn reserve(
        &self,
        tenant_id: &str,
        estimated_cost: f64,
    ) -> std::result::Result<Uuid, SpendLedgerError> {
        let Some(durable) = &self.durable else {
            return self.local.reserve_spend(tenant_id, estimated_cost).map_err(
                |(committed, ceiling)| SpendLedgerError::CeilingExceeded { committed, ceiling },
            );
        };

        let cost_micro = sunborn_core::MicroUnits::from_usd(estimated_cost).0;

        // 1. High-concurrency fast path: Lock-free atomic reservation from active lease
        if let Ok(guard) = self.leases.read() {
            if let Some(lease) = guard.get(tenant_id) {
                if lease.try_reserve(cost_micro) {
                    return Ok(lease.reservation_id);
                }
            }
        }

        // 2. Lease acquisition or direct durable reservation
        let ceiling = self.local.ceiling_for_tenant(tenant_id);
        let should_lease = ceiling >= 20.0 && estimated_cost > 0.0;
        let lease_chunk = if should_lease {
            (ceiling * 0.05).clamp(1.0, 5.0)
        } else {
            estimated_cost
        };

        if should_lease && lease_chunk > estimated_cost {
            match durable
                .reserve_spend(tenant_id, lease_chunk, ceiling, self.window())
                .await
            {
                SpendReservationOutcome::Granted(lease_id) => {
                    let lease_micro = sunborn_core::MicroUnits::from_usd(lease_chunk).0;
                    let lease = Arc::new(BudgetLease::new(
                        lease_id,
                        lease_micro,
                        Duration::from_secs(5),
                    ));
                    let _ = lease.try_reserve(cost_micro);
                    if let Ok(mut guard) = self.leases.write() {
                        guard.insert(tenant_id.to_string(), lease);
                    }
                    return Ok(lease_id);
                }
                SpendReservationOutcome::Denied { .. } => {
                    // Headroom might be smaller than full lease chunk; fall through to exact cost reservation
                }
                SpendReservationOutcome::StoreUnavailable => {
                    return Err(SpendLedgerError::Unavailable(
                        "distributed spend ledger unavailable; request rejected (fail-closed)"
                            .to_string(),
                    ));
                }
            }
        }

        // 3. Fallback: Exact direct reservation from PostgreSQL
        match durable
            .reserve_spend(tenant_id, estimated_cost, ceiling, self.window())
            .await
        {
            SpendReservationOutcome::Granted(id) => Ok(id),
            SpendReservationOutcome::Denied { committed, ceiling } => {
                Err(SpendLedgerError::CeilingExceeded { committed, ceiling })
            }
            SpendReservationOutcome::StoreUnavailable => Err(SpendLedgerError::Unavailable(
                "distributed spend ledger unavailable; request rejected (fail-closed)".to_string(),
            )),
        }
    }

    /// Commits actual cost against a reservation and returns window spend.
    pub async fn settle(
        &self,
        tenant_id: &str,
        reservation_id: Uuid,
        actual_cost: f64,
    ) -> std::result::Result<f64, SpendLedgerError> {
        let Some(durable) = &self.durable else {
            return Ok(self
                .local
                .settle_spend(tenant_id, reservation_id, actual_cost));
        };
        match durable
            .settle_spend(tenant_id, reservation_id, actual_cost, self.window())
            .await
        {
            Some(total) => Ok(total),
            None => {
                tracing::error!(
                    tenant = %tenant_id,
                    reservation = %reservation_id,
                    "Failed to settle distributed spend reservation; database error or pool unavailable"
                );
                Err(SpendLedgerError::Unavailable(
                    "distributed spend ledger settlement failed; reservation retained for reconciliation".to_string(),
                ))
            }
        }
    }

    /// Releases an unused reservation without billing spend.
    pub async fn release(&self, tenant_id: &str, reservation_id: Uuid) {
        let Some(durable) = &self.durable else {
            self.local.release_reservation(tenant_id, reservation_id);
            return;
        };
        if !durable.release_spend(tenant_id, reservation_id).await {
            tracing::warn!(
                tenant = %tenant_id,
                reservation = %reservation_id,
                "Failed to release distributed spend reservation; it will expire"
            );
        }
    }

    /// Sets a durable per-tenant ceiling shared by every replica.
    ///
    /// The durable write is authoritative and happens first. If it fails the
    /// local ceiling is left untouched, so a replica can never advertise (and
    /// enforce) a budget that the shared ledger does not agree with. Enforcing
    /// a ceiling the cluster has not committed is worse than rejecting the
    /// request: it silently permits spend that other replicas cannot see.
    pub async fn set_ceiling(
        &self,
        tenant_id: &str,
        ceiling_usd: f64,
    ) -> std::result::Result<(), SpendLedgerError> {
        if let Some(durable) = &self.durable {
            durable
                .set_spend_ceiling(tenant_id, ceiling_usd)
                .await
                .map_err(|err| SpendLedgerError::Unavailable(err.to_string()))?;
        }
        // Only mirror into local state after the durable commit succeeded.
        self.local.set_tenant_ceiling(tenant_id, ceiling_usd);
        Ok(())
    }

    /// Cluster-wide rolling-window spend for a tenant.
    pub async fn current_spend(&self, tenant_id: &str) -> f64 {
        let Some(durable) = &self.durable else {
            return self.local.current_spend(tenant_id);
        };
        match durable.spend_window_total(tenant_id, self.window()).await {
            Some(total) => total,
            None => {
                tracing::warn!(
                    tenant = %tenant_id,
                    "Distributed spend ledger unavailable; reporting local window spend"
                );
                self.local.current_spend(tenant_id)
            }
        }
    }

    /// Clears spend state for a tenant (operator reset).
    pub async fn reset(&self, tenant_id: &str) {
        self.local.reset_tenant(tenant_id);
        if let Some(durable) = &self.durable {
            if !durable.reset_spend(tenant_id).await {
                tracing::warn!(tenant = %tenant_id, "Failed to reset distributed spend ledger");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn local_ledger(ceiling: f64) -> SpendLedger {
        SpendLedger::new(
            Arc::new(SpendVelocityController::new(
                ceiling,
                Duration::from_secs(60),
            )),
            None,
        )
    }

    #[tokio::test]
    async fn local_ledger_reserves_settles_and_releases() {
        let ledger = local_ledger(10.0);
        assert!(!ledger.is_distributed());
        assert_eq!(ledger.enforcement_scope(), "process_local");

        let id = ledger.reserve("t1", 4.0).await.expect("reservation");
        let denied = ledger.reserve("t1", 7.0).await.unwrap_err();
        assert_eq!(
            denied,
            SpendLedgerError::CeilingExceeded {
                committed: 11.0,
                ceiling: 10.0
            }
        );

        assert_eq!(ledger.settle("t1", id, 3.5).await.expect("settlement"), 3.5);
        assert_eq!(ledger.current_spend("t1").await, 3.5);

        let id2 = ledger.reserve("t1", 6.0).await.expect("reservation");
        ledger.release("t1", id2).await;
        assert_eq!(ledger.current_spend("t1").await, 3.5);
    }

    #[tokio::test]
    async fn ceilings_are_per_tenant() {
        let ledger = local_ledger(10.0);
        ledger.set_ceiling("vip", 50.0).await.expect("set ceiling");
        assert!(ledger.reserve("standard", 9.0).await.is_ok());
        assert!(ledger.reserve("standard", 6.0).await.is_err());
        assert!(ledger.reserve("vip", 40.0).await.is_ok());
    }
}
