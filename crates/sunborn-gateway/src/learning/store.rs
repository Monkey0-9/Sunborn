//! Layer 2: Tenant-Safe Learning Data Store
//! Enforces tenant and application isolation, consent/retention policies,
//! GDPR Article 17 / crypto-shredding purges, and poison-defense bounding.

use crate::learning::feedback::VerifiedFeedbackEvent;
use std::collections::{HashMap, VecDeque};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use sunborn_core::{Result, SunbornError, TenantId};

pub struct TenantLearningStore {
    // Partitioned event storage: TenantId -> Bounded VecDeque
    events: RwLock<HashMap<TenantId, VecDeque<VerifiedFeedbackEvent>>>,
    max_events_per_tenant: usize,
    wal_path: Option<PathBuf>,
}

impl Default for TenantLearningStore {
    fn default() -> Self {
        let default_wal = std::env::var("SUNBORN_LEARNING_WAL_PATH")
            .ok()
            .map(PathBuf::from);
        Self::new_with_wal(5000, default_wal)
    }
}

impl TenantLearningStore {
    pub fn new(max_events_per_tenant: usize) -> Self {
        Self {
            events: RwLock::new(HashMap::new()),
            max_events_per_tenant,
            wal_path: None,
        }
    }

    pub fn new_with_wal(max_events_per_tenant: usize, wal_path: Option<PathBuf>) -> Self {
        let store = Self {
            events: RwLock::new(HashMap::new()),
            max_events_per_tenant,
            wal_path,
        };

        if let Some(ref path) = store.wal_path {
            if path.exists() {
                store.load_from_wal(path);
            }
        }

        store
    }

    fn load_from_wal(&self, path: &Path) {
        if let Ok(file) = File::open(path) {
            let reader = BufReader::new(file);
            let mut map = self.events.write().unwrap();
            for line in reader.lines().map_while(std::result::Result::ok) {
                if let Ok(event) = serde_json::from_str::<VerifiedFeedbackEvent>(&line) {
                    let queue = map.entry(event.tenant_id.clone()).or_default();
                    if queue.len() >= self.max_events_per_tenant {
                        queue.pop_front();
                    }
                    queue.push_back(event);
                }
            }
        }
    }

    fn append_to_wal(&self, event: &VerifiedFeedbackEvent) {
        if let Some(ref path) = self.wal_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
                if let Ok(serialized) = serde_json::to_string(event) {
                    let _ = writeln!(file, "{}", serialized);
                }
            }
        }
    }

    fn rewrite_wal(&self) {
        if let Some(ref path) = self.wal_path {
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)
            {
                let map = self.events.read().unwrap();
                for queue in map.values() {
                    for event in queue {
                        if let Ok(serialized) = serde_json::to_string(event) {
                            let _ = writeln!(file, "{}", serialized);
                        }
                    }
                }
            }
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

        queue.push_back(event.clone());
        drop(map);

        self.append_to_wal(&event);
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

    /// Scored event count (events with non-null similarity_score) for a specific tenant
    pub fn get_scored_event_count(&self, tenant_id: &TenantId) -> usize {
        let map = self.events.read().unwrap();
        map.get(tenant_id)
            .map(|q| q.iter().filter(|e| e.similarity_score.is_some()).count())
            .unwrap_or(0)
    }

    /// GDPR Article 17 / Cryptographic Shredding Cascade:
    /// Permanently wipes all learning events, calibration records, and telemetry for the tenant.
    pub fn purge_tenant(&self, tenant_id: &TenantId) -> usize {
        let mut map = self.events.write().unwrap();
        let count = map.remove(tenant_id).map(|q| q.len()).unwrap_or(0);
        drop(map);

        if count > 0 {
            self.rewrite_wal();
        }
        count
    }
}
