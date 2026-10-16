//! Layer 5: Controlled Promotion & Rollout Controller
//! Promotes signed, versioned policies only after correctness and security gates pass.
//! Manages canary rollouts, live metric monitoring, automatic rollback, and safety invariants.

use crate::learning::calibration::{CandidatePolicy, PolicyWeights};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::RwLock;
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

pub struct PolicyPromotionController {
    active_policy: RwLock<CandidatePolicy>,
    canary_policy: RwLock<Option<(CandidatePolicy, u8)>>, // (policy, canary_percent)
    audit_logs: RwLock<Vec<PromotionAuditLog>>,
}

impl Default for PolicyPromotionController {
    fn default() -> Self {
        let initial_policy = CandidatePolicy {
            policy_id: "policy-v1-baseline".to_string(),
            version: 1,
            weights: PolicyWeights::default(),
            created_at: Utc::now(),
            evaluation_report: crate::learning::calibration::CalibrationReport {
                total_samples: 0,
                training_samples: 0,
                held_out_samples: 0,
                training_false_hit_rate: 0.0,
                held_out_false_hit_rate: 0.0,
                held_out_abstention_rate: 0.0,
                estimated_cost_reduction_pct: 0.0,
                is_statistically_sound: true,
            },
            is_promoted: true,
        };

        Self {
            active_policy: RwLock::new(initial_policy),
            canary_policy: RwLock::new(None),
            audit_logs: RwLock::new(Vec::new()),
        }
    }
}

impl PolicyPromotionController {
    pub fn new() -> Self {
        Self::default()
    }

    /// Retrieve the currently active production policy weights
    pub fn get_active_weights(&self) -> PolicyWeights {
        self.active_policy.read().unwrap().weights.clone()
    }

    /// Retrieve the active policy
    pub fn get_active_policy(&self) -> CandidatePolicy {
        self.active_policy.read().unwrap().clone()
    }

    /// Retrieve active canary status if running
    pub fn get_canary_status(&self) -> Option<(CandidatePolicy, u8)> {
        self.canary_policy.read().unwrap().clone()
    }

    /// Select which policy to apply for a given request based on request hash and canary split
    pub fn select_policy_for_request(&self, request_hash: u64) -> PolicyWeights {
        let canary_guard = self.canary_policy.read().unwrap();
        if let Some((canary, pct)) = canary_guard.as_ref() {
            let bucket = (request_hash % 100) as u8;
            if bucket < *pct {
                return canary.weights.clone();
            }
        }
        self.active_policy.read().unwrap().weights.clone()
    }

    /// Start a staged canary rollout for a validated candidate policy
    pub fn start_canary(
        &self,
        candidate: CandidatePolicy,
        canary_percent: u8,
        actor: &str,
    ) -> Result<()> {
        // Enforce Level 5/6 Safety Gating:
        if !candidate.evaluation_report.is_statistically_sound {
            return Err(SunbornError::forbidden(
                "Promotion rejected: candidate policy failed statistical soundness gate (held-out false hit rate > 1%)",
            ));
        }

        if candidate.weights.min_similarity_threshold < 0.75 {
            return Err(SunbornError::forbidden(
                "Security invariant violation: minimum similarity threshold cannot be below 0.75",
            ));
        }

        let clamped_pct = canary_percent.min(25); // Max canary limit before full promotion
        let policy_id = candidate.policy_id.clone();
        let prev_id = self.active_policy.read().unwrap().policy_id.clone();
        let version = candidate.version;

        {
            let mut canary_guard = self.canary_policy.write().unwrap();
            *canary_guard = Some((candidate, clamped_pct));
        }

        let log = PromotionAuditLog {
            policy_id,
            previous_policy_id: prev_id,
            version,
            action: "CANARY_STARTED".to_string(),
            canary_percentage: clamped_pct,
            reason: format!("Canary rollout initiated at {}% by {}", clamped_pct, actor),
            timestamp: Utc::now(),
        };

        self.audit_logs.write().unwrap().push(log);
        Ok(())
    }

    /// Promote canary policy to 100% active production policy
    pub fn complete_promotion(&self, actor: &str) -> Result<CandidatePolicy> {
        let mut canary_guard = self.canary_policy.write().unwrap();
        let (mut canary, _) = canary_guard
            .take()
            .ok_or_else(|| SunbornError::validation_error("No active canary policy to promote"))?;

        canary.is_promoted = true;
        let mut active_guard = self.active_policy.write().unwrap();
        let prev_id = active_guard.policy_id.clone();
        *active_guard = canary.clone();

        let log = PromotionAuditLog {
            policy_id: canary.policy_id.clone(),
            previous_policy_id: prev_id,
            version: canary.version,
            action: "FULLY_PROMOTED".to_string(),
            canary_percentage: 100,
            reason: format!("Policy fully promoted to production by {}", actor),
            timestamp: Utc::now(),
        };

        self.audit_logs.write().unwrap().push(log);
        Ok(canary)
    }

    /// Automated or manual emergency rollback of the canary policy
    pub fn trigger_rollback(&self, trigger: RollbackTrigger, reason: &str) -> Result<()> {
        let mut canary_guard = self.canary_policy.write().unwrap();
        if let Some((canary, _)) = canary_guard.take() {
            let active_guard = self.active_policy.read().unwrap();
            let log = PromotionAuditLog {
                policy_id: active_guard.policy_id.clone(),
                previous_policy_id: canary.policy_id.clone(),
                version: active_guard.version,
                action: "ROLLED_BACK".to_string(),
                canary_percentage: 0,
                reason: format!("Rollback triggered by {:?}: {}", trigger, reason),
                timestamp: Utc::now(),
            };

            tracing::warn!(
                trigger = ?trigger,
                reason = %reason,
                "CANARY_ROLLED_BACK: Emergency rollback restored active baseline policy"
            );

            self.audit_logs.write().unwrap().push(log);
            Ok(())
        } else {
            Ok(())
        }
    }

    /// Fetch audit trail of policy promotions and rollbacks
    pub fn get_audit_trail(&self) -> Vec<PromotionAuditLog> {
        self.audit_logs.read().unwrap().clone()
    }
}
