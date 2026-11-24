use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Record of an adversarial candidate quarantined by the threat engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineEntry {
    pub key: String,
    pub original_query: String,
    pub attack_query: String,
    pub threat_category: String,
    pub anomaly_score: f32,
    pub quarantined_at: DateTime<Utc>,
}

/// Thread-safe in-memory quarantine pool isolating adversarial cache candidates.
#[derive(Clone, Default)]
pub struct QuarantinePool {
    entries: Arc<DashMap<String, QuarantineEntry>>,
}

impl QuarantinePool {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(DashMap::new()),
        }
    }

    pub fn quarantine(&self, entry: QuarantineEntry) {
        self.entries.insert(entry.key.clone(), entry);
    }

    pub fn is_quarantined(&self, key: &str) -> bool {
        self.entries.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn all_entries(&self) -> Vec<QuarantineEntry> {
        self.entries.iter().map(|kv| kv.value().clone()).collect()
    }
}
