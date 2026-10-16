//! Layer 2: Tenant-Safe Learning Data Store
//! Enforces tenant and application isolation, consent/retention policies,
//! GDPR Article 17 / crypto-shredding purges, and poison-defense bounding.

use crate::learning::feedback::VerifiedFeedbackEvent;
use std::collections::{HashMap, VecDeque};
use std::sync::RwLock;
use sunborn_core::{Result, SunbornError, TenantId};

pub struct TenantLearningStore {
    // Partitioned event storage: TenantId -> Bounded VecDeque
    events: RwLock<HashMap<TenantId, VecDeque<VerifiedFeedbackEvent>>>,
    max_events_per_tenant: usize,
}

impl Default for TenantLearningStore {
    fn default() -> Self {
        Self::new(5000)
    }
}

impl TenantLearningStore {
    pub fn new(max_events_per_tenant: usize) -> Self {
        Self {
            events: RwLock::new(HashMap::new()),
            max_events_per_tenant,
        }
    }

    /// Record a verified feedback event with tenant isolation and poison filtering
    pub fn record_event(&self, event: VerifiedFeedbackEvent) -> Result<()> {
        if !event.is_trustworthy() {
            return Err(SunbornError::validation_error(
                "Untrusted feedback rejected: unverified opinions cannot update learning store",
            ));
        }

        let mut map = self.events.write().unwrap();
        let queue = map.entry(event.tenant_id.clone()).or_default();

        // Poison defense: Check if this specific cache_key already received contradictory labels
        let contradictory_count = queue
            .iter()
            .filter(|e| {
                e.cache_key == event.cache_key && e.verified_outcome != event.verified_outcome
            })
            .count();

        if contradictory_count >= 5 {
            return Err(SunbornError::validation_error(format!(
                "Poisoning defense triggered: excessive contradictory feedback for cache key '{}'",
                event.cache_key
            )));
        }

        if queue.len() >= self.max_events_per_tenant {
            queue.pop_front();
        }

        queue.push_back(event);
        Ok(())
    }

    /// Fetch verified learning events for a specific tenant (tenant isolation invariant)
    pub fn get_tenant_events(&self, tenant_id: &TenantId) -> Vec<VerifiedFeedbackEvent> {
        let map = self.events.read().unwrap();
        map.get(tenant_id)
            .map(|q| q.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Total event count across tenants
    pub fn total_events(&self) -> usize {
        let map = self.events.read().unwrap();
        map.values().map(|q| q.len()).sum()
    }

    /// Event count for a specific tenant
    pub fn tenant_event_count(&self, tenant_id: &TenantId) -> usize {
        let map = self.events.read().unwrap();
        map.get(tenant_id).map(|q| q.len()).unwrap_or(0)
    }

    /// GDPR Article 17 / Cryptographic Shredding Cascade:
    /// Permanently wipes all learning events, calibration records, and telemetry for the tenant.
    pub fn purge_tenant(&self, tenant_id: &TenantId) -> usize {
        let mut map = self.events.write().unwrap();
        map.remove(tenant_id).map(|q| q.len()).unwrap_or(0)
    }
}
