//! Versioned Cache Lifecycle, Adaptive Retention Utility ($V_i$), Safe Refresh &
//! Predictive Prefetch Dispatcher (§8 / SB-08 & SB-11).
//!
//! State machine:
//! `Candidate -> Validating -> Admitted -> Active -> RevalidationRequired -> Refreshing -> Superseded | Evicted | Quarantined`
//! Never mutates cached entries silently in-place; tracks versioned transitions,
//! evicts lowest utility $V_i$ under configured capacity budgets, and prohibits
//! prefetching or refreshing mutating side-effecting workloads.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::RwLock;
use sunborn_core::sync::{read_lock, write_lock};
use sunborn_core::{RequestEnvelope, TenantId};

use crate::five_gates::FiveGateEvaluator;
use crate::learning::PolicyWeights;
use crate::routing::predictive::PrefetchPrediction;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CacheLifecycleState {
    Candidate,
    Validating,
    Admitted,
    Active,
    RevalidationRequired,
    Refreshing,
    Superseded,
    Evicted,
    Quarantined,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleTransition {
    pub cache_key: String,
    pub version: u64,
    pub from: CacheLifecycleState,
    pub to: CacheLifecycleState,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionedCacheMetadata {
    pub cache_key: String,
    pub tenant_id: TenantId,
    pub entry_version: u64,
    pub state: CacheLifecycleState,
    pub expected_reuse_prob: f64,
    pub avoided_cost_usd: f64,
    pub verified_quality: f64,
    pub volatility_risk: f64,
    pub storage_cost_bytes: usize,
    pub safety_gates_valid: bool,
    pub updated_at: DateTime<Utc>,
}

pub struct CacheLifecycleManager {
    entries: RwLock<HashMap<String, VersionedCacheMetadata>>,
    transitions: RwLock<Vec<LifecycleTransition>>,
    /// Keys of side-channel timing summaries (outcome -> latencies in ms)
    timing_histograms: RwLock<HashMap<String, Vec<f64>>>,
    /// Quarantined keys excluded from all admission & retention
    quarantined_keys: RwLock<HashSet<String>>,
}

impl Default for CacheLifecycleManager {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheLifecycleManager {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            transitions: RwLock::new(Vec::new()),
            timing_histograms: RwLock::new(HashMap::new()),
            quarantined_keys: RwLock::new(HashSet::new()),
        }
    }

    pub fn admit_versioned(
        &self,
        cache_key: &str,
        tenant_id: &TenantId,
        avoided_cost_usd: f64,
        verified_quality: f64,
        volatility_risk: f64,
        storage_cost_bytes: usize,
    ) -> VersionedCacheMetadata {
        let mut entries = write_lock(&self.entries);
        let prev_ver = entries.get(cache_key).map(|e| e.entry_version).unwrap_or(0);
        if prev_ver > 0 {
            if let Some(prev) = entries.get_mut(cache_key) {
                let prev_state = prev.state;
                prev.state = CacheLifecycleState::Superseded;
                self.push_transition(
                    cache_key,
                    prev_ver,
                    prev_state,
                    CacheLifecycleState::Superseded,
                    "SUPERSEDED_BY_NEWER_VERSION",
                );
            }
        }

        let new_ver = prev_ver + 1;
        let meta = VersionedCacheMetadata {
            cache_key: cache_key.to_string(),
            tenant_id: tenant_id.clone(),
            entry_version: new_ver,
            state: CacheLifecycleState::Active,
            expected_reuse_prob: 0.5,
            avoided_cost_usd: avoided_cost_usd.max(0.0),
            verified_quality: verified_quality.clamp(0.0, 1.0),
            volatility_risk: volatility_risk.clamp(0.0, 1.0),
            storage_cost_bytes: storage_cost_bytes.max(1),
            safety_gates_valid: true,
            updated_at: Utc::now(),
        };
        entries.insert(cache_key.to_string(), meta.clone());
        self.push_transition(
            cache_key,
            new_ver,
            CacheLifecycleState::Admitted,
            CacheLifecycleState::Active,
            "ADMISSION_COMMITTED",
        );
        meta
    }

    pub fn transition(
        &self,
        cache_key: &str,
        next_state: CacheLifecycleState,
        reason: &str,
    ) -> Option<VersionedCacheMetadata> {
        if next_state == CacheLifecycleState::Quarantined {
            write_lock(&self.quarantined_keys).insert(cache_key.to_string());
        } else if next_state == CacheLifecycleState::Active {
            write_lock(&self.quarantined_keys).remove(cache_key);
        }

        let mut entries = write_lock(&self.entries);
        let Some(entry) = entries.get_mut(cache_key) else {
            drop(entries);
            if next_state == CacheLifecycleState::Quarantined {
                self.push_transition(
                    cache_key,
                    0,
                    CacheLifecycleState::Candidate,
                    CacheLifecycleState::Quarantined,
                    reason,
                );
            }
            return None;
        };
        let prev = entry.state;
        entry.state = next_state;
        entry.updated_at = Utc::now();
        if next_state == CacheLifecycleState::Quarantined {
            entry.safety_gates_valid = false;
        } else if next_state == CacheLifecycleState::Active {
            entry.safety_gates_valid = true;
        }
        let ver = entry.entry_version;
        let cloned = entry.clone();
        drop(entries);
        self.push_transition(cache_key, ver, prev, next_state, reason);
        Some(cloned)
    }

    pub fn is_quarantined(&self, cache_key: &str) -> bool {
        read_lock(&self.quarantined_keys).contains(cache_key)
    }

    pub fn due_for_revalidation(&self) -> Vec<VersionedCacheMetadata> {
        read_lock(&self.entries)
            .values()
            .filter(|e| {
                e.state == CacheLifecycleState::RevalidationRequired && e.safety_gates_valid
            })
            .cloned()
            .collect()
    }

    pub fn get_metadata(&self, cache_key: &str) -> Option<VersionedCacheMetadata> {
        read_lock(&self.entries).get(cache_key).cloned()
    }

    /// Computes remaining sleep duration (in ms) when a timing floor (`SUNBORN_TIMING_FLOOR_MS`)
    /// is configured to mitigate cache-probing timing side channels (§13.3).
    pub fn timing_equalizer_delay_ms(floor_ms: u64, elapsed_ms: u64) -> u64 {
        floor_ms.saturating_sub(elapsed_ms)
    }

    /// Adaptive utility score $V_i = \alpha R_i + \beta C_i + \gamma Q_i - \delta S_i - \epsilon M_i$ (§8.4),
    /// driven by live `PolicyWeights::{eviction_cost_weight, eviction_staleness_weight}`.
    /// Entries with failed safety gates always score `-inf` and are evicted first.
    pub fn compute_utility_score(entry: &VersionedCacheMetadata, weights: &PolicyWeights) -> f64 {
        if !entry.safety_gates_valid || entry.state == CacheLifecycleState::Quarantined {
            return f64::NEG_INFINITY;
        }
        let alpha = 0.25;
        let beta = weights.eviction_cost_weight as f64;
        let gamma = 0.20;
        let delta = weights.eviction_staleness_weight as f64;
        let eps = 0.05;
        let mem_norm = (entry.storage_cost_bytes as f64 / 4096.0).min(10.0);

        alpha * entry.expected_reuse_prob
            + beta * (entry.avoided_cost_usd * 100.0)
            + gamma * entry.verified_quality
            - delta * entry.volatility_risk
            - eps * mem_norm
    }

    /// Selects keys for adaptive eviction until at most `target_max_entries` remain active.
    pub fn select_eviction_candidates(
        &self,
        weights: &PolicyWeights,
        target_max_entries: usize,
    ) -> Vec<String> {
        let entries = read_lock(&self.entries);
        let mut active: Vec<&VersionedCacheMetadata> = entries
            .values()
            .filter(|e| {
                matches!(
                    e.state,
                    CacheLifecycleState::Active | CacheLifecycleState::Quarantined
                )
            })
            .collect();
        if active.len() <= target_max_entries {
            return Vec::new();
        }
        active.sort_by(|a, b| {
            let va = Self::compute_utility_score(a, weights);
            let vb = Self::compute_utility_score(b, weights);
            va.partial_cmp(&vb).unwrap_or(std::cmp::Ordering::Equal)
        });
        let num_to_evict = active.len().saturating_sub(target_max_entries);
        active
            .into_iter()
            .take(num_to_evict)
            .map(|e| e.cache_key.clone())
            .collect()
    }

    /// Evaluates whether a predicted follow-up request is eligible for safe read-only
    /// speculative prefetching (§8.5): enforces `PolicyWeights::prefetch_confidence_threshold`
    /// and strictly rejects any mutating/side-effecting G4 action.
    pub fn filter_safe_prefetch_candidates(
        predictions: &[PrefetchPrediction],
        sample_request: &RequestEnvelope,
        weights: &PolicyWeights,
    ) -> Vec<PrefetchPrediction> {
        let g4 = FiveGateEvaluator::evaluate_g4_effects(sample_request);
        if !g4.passed {
            return Vec::new();
        }
        predictions
            .iter()
            .filter(|p| p.confidence_score >= weights.prefetch_confidence_threshold)
            .cloned()
            .collect()
    }

    /// Records timing side-channel observation grouped by outcome bucket (`exact_hit`,
    /// `semantic_hit`, `origin_miss`, `auth_reject`) for observability (§13.3).
    pub fn record_timing(&self, outcome_bucket: &str, duration_ms: f64) {
        let mut map = write_lock(&self.timing_histograms);
        let vec = map.entry(outcome_bucket.to_string()).or_default();
        vec.push(duration_ms);
        if vec.len() > 2048 {
            let drain = vec.len() - 2048;
            vec.drain(0..drain);
        }
    }

    pub fn timing_summary(&self) -> HashMap<String, (usize, f64, f64)> {
        let map = read_lock(&self.timing_histograms);
        let mut out = HashMap::new();
        for (bucket, samples) in map.iter() {
            if samples.is_empty() {
                continue;
            }
            let mut sorted = samples.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let p50 = sorted[sorted.len() / 2];
            let p95 = sorted[((sorted.len() * 95) / 100).min(sorted.len() - 1)];
            out.insert(bucket.clone(), (sorted.len(), p50, p95));
        }
        out
    }

    pub fn transition_history(&self, cache_key: &str) -> Vec<LifecycleTransition> {
        read_lock(&self.transitions)
            .iter()
            .filter(|t| t.cache_key == cache_key)
            .cloned()
            .collect()
    }

    fn push_transition(
        &self,
        cache_key: &str,
        version: u64,
        from: CacheLifecycleState,
        to: CacheLifecycleState,
        reason: &str,
    ) {
        let mut log = write_lock(&self.transitions);
        log.push(LifecycleTransition {
            cache_key: cache_key.to_string(),
            version,
            from,
            to,
            reason: reason.to_string(),
            timestamp: Utc::now(),
        });
        if log.len() > 10_000 {
            let drain = log.len() - 10_000;
            log.drain(0..drain);
        }
    }
}
