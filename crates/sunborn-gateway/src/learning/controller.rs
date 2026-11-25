//! Layer 5: Controlled Promotion & Rollout Controller
//! Promotes signed, versioned policies only after correctness and security gates pass.
//! Manages canary rollouts, live metric monitoring, automatic rollback, and safety invariants.

use crate::learning::calibration::{CandidatePolicy, PolicyWeights};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::RwLock;
use sunborn_core::sync::{read_lock, write_lock};
use sunborn_core::{Result, SunbornError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromotionAuditLog {
    pub policy_id: String,
    pub previous_policy_id: String,
    pub version: u32,
    pub action: String, // "CANARY_STARTED", "FULLY_PROMOTED", "ROLLED_BACK"
    pub canary_percentage: u8,
    pub reason: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RollbackTrigger {
    FalseHitDetected,
    LatencySlaViolation,
    ErrorRateSpike,
    ManualIntervention,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanarySafetyThresholds {
    pub min_samples_for_rate_check: u64,
    pub max_false_hit_rate: f64,
    pub max_absolute_false_hits: u64,
    pub max_error_rate: f64,
    pub latency_sla_ms: f64,
    pub max_latency_violation_rate: f64,
}

impl Default for CanarySafetyThresholds {
    fn default() -> Self {
        Self {
            min_samples_for_rate_check: 20,
            max_false_hit_rate: 0.01,
            max_absolute_false_hits: 2,
            max_error_rate: 0.05,
            latency_sla_ms: 5000.0,
            max_latency_violation_rate: 0.10,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CanaryLiveMetrics {
    pub total_requests: u64,
    pub false_hits: u64,
    pub errors: u64,
    pub latency_violations: u64,
    pub total_latency_ms: f64,
}

#[derive(Debug, Clone)]
pub struct PolicySelection {
    pub policy_id: String,
    pub version: u32,
    pub weights: PolicyWeights,
    pub is_canary: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedPolicyState {
    pub active_policy: CandidatePolicy,
    pub canary_policy: Option<(CandidatePolicy, u8)>,
    pub audit_logs: Vec<PromotionAuditLog>,
}

pub struct PolicyPromotionController {
    active_policy: RwLock<CandidatePolicy>,
    canary_policy: RwLock<Option<(CandidatePolicy, u8)>>, // (policy, canary_percent)
    canary_metrics: RwLock<CanaryLiveMetrics>,
    canary_thresholds: RwLock<CanarySafetyThresholds>,
    audit_logs: RwLock<Vec<PromotionAuditLog>>,
    persist_path: Option<PathBuf>,
}

impl Default for PolicyPromotionController {
    fn default() -> Self {
        let default_path = std::env::var("SUNBORN_POLICY_REGISTRY_PATH")
            .ok()
            .filter(|s| !s.trim().is_empty())
            .map(PathBuf::from);
        Self::new_with_persistence(default_path)
    }
}

impl PolicyPromotionController {
    pub fn new() -> Self {
        Self {
            active_policy: RwLock::new(Self::baseline_policy()),
            canary_policy: RwLock::new(None),
            canary_metrics: RwLock::new(CanaryLiveMetrics::default()),
            canary_thresholds: RwLock::new(CanarySafetyThresholds::default()),
            audit_logs: RwLock::new(Vec::new()),
            persist_path: None,
        }
    }

    pub fn new_with_persistence(persist_path: Option<PathBuf>) -> Self {
        let mut initial_active = Self::baseline_policy();
        let mut initial_canary = None;
        let mut initial_logs = Vec::new();

        if let Some(ref path) = persist_path {
            if path.exists() {
                if let Ok(file) = File::open(path) {
                    if let Ok(state) = serde_json::from_reader::<_, PersistedPolicyState>(file) {
                        initial_active = state.active_policy;
                        initial_canary = state.canary_policy;
                        initial_logs = state.audit_logs;
                    }
                }
            }
        }

        Self {
            active_policy: RwLock::new(initial_active),
            canary_policy: RwLock::new(initial_canary),
            canary_metrics: RwLock::new(CanaryLiveMetrics::default()),
            canary_thresholds: RwLock::new(CanarySafetyThresholds::default()),
            audit_logs: RwLock::new(initial_logs),
            persist_path,
        }
    }

    pub fn is_durable(&self) -> bool {
        self.persist_path.is_some()
    }

    pub fn storage_tier(&self) -> &'static str {
        if self.is_durable() {
            "durable-registry-json"
        } else {
            "process-local-ephemeral"
        }
    }

    fn baseline_policy() -> CandidatePolicy {
        CandidatePolicy {
            policy_id: "policy-v1-baseline".to_string(),
            version: 1,
            weights: PolicyWeights::default(),
            created_at: Utc::now(),
            evaluation_report: crate::learning::calibration::CalibrationReport {
                total_samples: 0,
                training_samples: 0,
                held_out_samples: 0,
                held_out_evaluated: 0,
                held_out_admitted: 0,
                held_out_false_hits: 0,
                held_out_stale_hits: 0,
                held_out_ground_truth_mismatches: 0,
                training_false_hit_rate: 0.0,
                held_out_false_hit_rate: 0.0,
                held_out_false_hit_upper_bound_95: 0.0,
                held_out_wilson_upper_bound_95: 0.0,
                held_out_abstention_rate: 0.0,
                estimated_cost_reduction_pct: 0.0,
                is_statistically_sound: true,
            },
            is_promoted: true,
        }
    }

    fn save_state(&self) {
        if let Some(ref path) = self.persist_path {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let state = PersistedPolicyState {
                active_policy: read_lock(&self.active_policy).clone(),
                canary_policy: read_lock(&self.canary_policy).clone(),
                audit_logs: read_lock(&self.audit_logs).clone(),
            };
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)
            {
                if let Ok(json_bytes) = serde_json::to_vec_pretty(&state) {
                    let _ = file.write_all(&json_bytes);
                    let _ = file.sync_all();
                }
            }
        }
    }

    /// Retrieve the currently active production policy weights
    pub fn get_active_weights(&self) -> PolicyWeights {
        read_lock(&self.active_policy).weights.clone()
    }

    /// Retrieve the active policy
    pub fn get_active_policy(&self) -> CandidatePolicy {
        read_lock(&self.active_policy).clone()
    }

    /// Retrieve active canary status if running
    pub fn get_canary_status(&self) -> Option<(CandidatePolicy, u8)> {
        read_lock(&self.canary_policy).clone()
    }

    /// Retrieve live telemetry metrics for the active canary window
    pub fn get_canary_metrics(&self) -> CanaryLiveMetrics {
        read_lock(&self.canary_metrics).clone()
    }

    /// Update canary monitoring thresholds
    pub fn set_canary_thresholds(&self, thresholds: CanarySafetyThresholds) {
        *write_lock(&self.canary_thresholds) = thresholds;
    }

    /// Select which policy weights to apply for a given request based on request hash and canary split
    pub fn select_policy_for_request(&self, request_hash: u64) -> PolicyWeights {
        self.select_policy_decision_for_request(request_hash)
            .weights
    }

    /// Select policy weights and full attribution metadata (policy_id, version, is_canary)
    pub fn select_policy_decision_for_request(&self, request_hash: u64) -> PolicySelection {
        let canary_guard = read_lock(&self.canary_policy);
        if let Some((canary, pct)) = canary_guard.as_ref() {
            let bucket = (request_hash % 100) as u8;
            if bucket < *pct {
                return PolicySelection {
                    policy_id: canary.policy_id.clone(),
                    version: canary.version,
                    weights: canary.weights.clone(),
                    is_canary: true,
                };
            }
        }
        let active = read_lock(&self.active_policy);
        PolicySelection {
            policy_id: active.policy_id.clone(),
            version: active.version,
            weights: active.weights.clone(),
            is_canary: false,
        }
    }

    /// Record a live request or shadow evaluation outcome and automatically trigger
    /// canary rollback if safety thresholds (false hits, error rate, latency SLA) are breached.
    pub fn record_canary_observation(
        &self,
        is_canary: bool,
        latency_ms: f64,
        is_error: bool,
        is_false_hit: bool,
    ) -> Option<RollbackTrigger> {
        if !is_canary || read_lock(&self.canary_policy).is_none() {
            return None;
        }

        let thresholds = read_lock(&self.canary_thresholds).clone();
        {
            let mut m = write_lock(&self.canary_metrics);
            m.total_requests += 1;
            if is_false_hit {
                m.false_hits += 1;
            }
            if is_error {
                m.errors += 1;
            }
            if latency_ms.is_finite() && latency_ms >= 0.0 {
                m.total_latency_ms += latency_ms;
                if latency_ms > thresholds.latency_sla_ms {
                    m.latency_violations += 1;
                }
            }
        }

        self.evaluate_and_enforce_canary_health()
    }

    /// Evaluate live canary metrics against safety thresholds and automatically roll back on breach.
    pub fn evaluate_and_enforce_canary_health(&self) -> Option<RollbackTrigger> {
        if read_lock(&self.canary_policy).is_none() {
            return None;
        }
        let m = read_lock(&self.canary_metrics).clone();
        let t = read_lock(&self.canary_thresholds).clone();

        if m.false_hits >= t.max_absolute_false_hits
            || (m.total_requests >= t.min_samples_for_rate_check
                && (m.false_hits as f64 / m.total_requests as f64) > t.max_false_hit_rate)
        {
            let reason = format!(
                "Automated monitor detected canary false hits ({}/{} requests, limit {:.2}% / max {})",
                m.false_hits,
                m.total_requests,
                t.max_false_hit_rate * 100.0,
                t.max_absolute_false_hits
            );
            let _ = self.trigger_rollback(RollbackTrigger::FalseHitDetected, &reason);
            return Some(RollbackTrigger::FalseHitDetected);
        }

        if m.total_requests >= t.min_samples_for_rate_check
            && (m.errors as f64 / m.total_requests as f64) > t.max_error_rate
        {
            let reason = format!(
                "Automated monitor detected canary error rate spike ({}/{} requests, limit {:.2}%)",
                m.errors,
                m.total_requests,
                t.max_error_rate * 100.0
            );
            let _ = self.trigger_rollback(RollbackTrigger::ErrorRateSpike, &reason);
            return Some(RollbackTrigger::ErrorRateSpike);
        }

        if m.total_requests >= t.min_samples_for_rate_check
            && (m.latency_violations as f64 / m.total_requests as f64)
                > t.max_latency_violation_rate
        {
            let reason = format!(
                "Automated monitor detected canary latency SLA violations ({}/{} > {:.1}ms)",
                m.latency_violations, m.total_requests, t.latency_sla_ms
            );
            let _ = self.trigger_rollback(RollbackTrigger::LatencySlaViolation, &reason);
            return Some(RollbackTrigger::LatencySlaViolation);
        }

        None
    }

    /// Start a staged canary rollout for a validated candidate policy
    pub fn start_canary(
        &self,
        candidate: CandidatePolicy,
        canary_percent: u8,
        actor: &str,
    ) -> Result<()> {
        // Enforce Adaptive Closed-Loop Safety Gating:
        if !candidate.evaluation_report.is_statistically_sound {
            return Err(SunbornError::forbidden(
                "Promotion rejected: candidate policy failed statistical soundness gate (insufficient sample size or confidence upper bound exceeds limit)",
            ));
        }

        if candidate.weights.min_similarity_threshold < 0.75 {
            return Err(SunbornError::forbidden(
                "Security invariant violation: minimum similarity threshold cannot be below 0.75",
            ));
        }

        let clamped_pct = canary_percent.min(25); // Max canary limit before full promotion
        let policy_id = candidate.policy_id.clone();
        let prev_id = read_lock(&self.active_policy).policy_id.clone();
        let version = candidate.version;

        {
            let mut canary_guard = write_lock(&self.canary_policy);
            *canary_guard = Some((candidate, clamped_pct));
        }
        *write_lock(&self.canary_metrics) = CanaryLiveMetrics::default();

        let log = PromotionAuditLog {
            policy_id,
            previous_policy_id: prev_id,
            version,
            action: "CANARY_STARTED".to_string(),
            canary_percentage: clamped_pct,
            reason: format!("Canary rollout initiated at {}% by {}", clamped_pct, actor),
            timestamp: Utc::now(),
        };

        write_lock(&self.audit_logs).push(log);
        self.save_state();
        Ok(())
    }

    /// Promote canary policy to 100% active production policy
    pub fn complete_promotion(&self, actor: &str) -> Result<CandidatePolicy> {
        let (canary, prev_id) = {
            let mut canary_guard = write_lock(&self.canary_policy);
            let (mut c, _) = canary_guard.take().ok_or_else(|| {
                SunbornError::validation_error("No active canary policy to promote")
            })?;
            c.is_promoted = true;

            let mut active_guard = write_lock(&self.active_policy);
            let pid = active_guard.policy_id.clone();
            *active_guard = c.clone();
            (c, pid)
        };
        *write_lock(&self.canary_metrics) = CanaryLiveMetrics::default();

        let log = PromotionAuditLog {
            policy_id: canary.policy_id.clone(),
            previous_policy_id: prev_id,
            version: canary.version,
            action: "FULLY_PROMOTED".to_string(),
            canary_percentage: 100,
            reason: format!("Policy fully promoted to production by {}", actor),
            timestamp: Utc::now(),
        };

        write_lock(&self.audit_logs).push(log);
        self.save_state();
        Ok(canary)
    }

    /// Automated or manual emergency rollback of the canary policy
    pub fn trigger_rollback(&self, trigger: RollbackTrigger, reason: &str) -> Result<()> {
        let rollback_log = {
            let mut canary_guard = write_lock(&self.canary_policy);
            if let Some((canary, _)) = canary_guard.take() {
                let active_guard = read_lock(&self.active_policy);
                Some(PromotionAuditLog {
                    policy_id: active_guard.policy_id.clone(),
                    previous_policy_id: canary.policy_id.clone(),
                    version: active_guard.version,
                    action: "ROLLED_BACK".to_string(),
                    canary_percentage: 0,
                    reason: format!("Rollback triggered by {:?}: {}", trigger, reason),
                    timestamp: Utc::now(),
                })
            } else {
                None
            }
        };

        if let Some(log) = rollback_log {
            tracing::warn!(
                trigger = ?trigger,
                reason = %reason,
                "CANARY_ROLLED_BACK: Emergency rollback restored active baseline policy"
            );

            *write_lock(&self.canary_metrics) = CanaryLiveMetrics::default();
            write_lock(&self.audit_logs).push(log);
            self.save_state();
        }
        Ok(())
    }

    /// Fetch audit trail of policy promotions and rollbacks
    pub fn get_audit_trail(&self) -> Vec<PromotionAuditLog> {
        read_lock(&self.audit_logs).clone()
    }

    /// Get active policy ID
    pub fn get_active_policy_id(&self) -> String {
        read_lock(&self.active_policy).policy_id.clone()
    }

    /// Get active policy version
    pub fn get_active_policy_version(&self) -> u32 {
        read_lock(&self.active_policy).version
    }
}
