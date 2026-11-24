use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// State vector capturing the authoritative multi-dimensional versioning epoch.
/// F(q,t) = f(D_t, P_t, A_t, M_t, E_t)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporalState {
    pub document_version: u64,
    pub permission_version: u64,
    pub config_version: u64,
    pub model_version: u64,
    pub epoch: u64,
}

impl Default for TemporalState {
    fn default() -> Self {
        Self {
            document_version: 1,
            permission_version: 1,
            config_version: 1,
            model_version: 1,
            epoch: 1,
        }
    }
}

/// The required version preconditions bound to a reusable computation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FreshnessVector {
    pub required_doc_version: u64,
    pub required_perm_version: u64,
    pub required_config_version: u64,
    pub required_model_version: u64,
    pub max_staleness_epoch: u64,
}

/// Outcome of evaluating causal freshness against authoritative state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CausalEvaluationResult {
    Fresh { confidence_score: f32 },
    StaleDocument { expected: u64, current: u64 },
    StalePermission { expected: u64, current: u64 },
    StaleConfig { expected: u64, current: u64 },
    StaleModel { expected: u64, current: u64 },
    ExpiredEpoch { max_epoch: u64, current_epoch: u64 },
    DecayedConfidence { score: f32, threshold: f32 },
    NotFound,
}

impl CausalEvaluationResult {
    pub fn is_fresh(&self) -> bool {
        matches!(self, CausalEvaluationResult::Fresh { .. })
    }
}

/// Policy for decaying temporal confidence over time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub enum ConfidenceDecayPolicy {
    #[default]
    None,
    Linear {
        ttl_seconds: u64,
    },
    Exponential {
        lambda: f32,
    },
}

/// Thread-safe multi-dimensional temporal validity engine.
#[derive(Clone)]
pub struct TemporalValidityEngine {
    states: Arc<DashMap<Uuid, TemporalState>>,
}

impl Default for TemporalValidityEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl TemporalValidityEngine {
    pub fn new() -> Self {
        Self {
            states: Arc::new(DashMap::new()),
        }
    }

    pub fn register_state(&self, id: Uuid, state: TemporalState) {
        self.states.insert(id, state);
    }

    pub fn update_document_version(&self, id: Uuid, version: u64) {
        if let Some(mut entry) = self.states.get_mut(&id) {
            entry.document_version = version;
            entry.epoch += 1;
        } else {
            let state = TemporalState {
                document_version: version,
                ..Default::default()
            };
            self.states.insert(id, state);
        }
    }

    pub fn update_permission_version(&self, id: Uuid, version: u64) {
        if let Some(mut entry) = self.states.get_mut(&id) {
            entry.permission_version = version;
            entry.epoch += 1;
        }
    }

    pub fn update_config_version(&self, id: Uuid, version: u64) {
        if let Some(mut entry) = self.states.get_mut(&id) {
            entry.config_version = version;
            entry.epoch += 1;
        }
    }

    pub fn update_model_version(&self, id: Uuid, version: u64) {
        if let Some(mut entry) = self.states.get_mut(&id) {
            entry.model_version = version;
            entry.epoch += 1;
        }
    }

    pub fn advance_epoch(&self, id: Uuid, epoch: u64) {
        if let Some(mut entry) = self.states.get_mut(&id) {
            entry.epoch = epoch;
        }
    }

    /// Evaluates multi-dimensional freshness against current state.
    pub fn evaluate_freshness(
        &self,
        id: &Uuid,
        vector: &FreshnessVector,
        current_epoch: u64,
    ) -> CausalEvaluationResult {
        let current_state = match self.states.get(id) {
            Some(s) => *s,
            None => return CausalEvaluationResult::NotFound,
        };

        if current_state.document_version != vector.required_doc_version {
            return CausalEvaluationResult::StaleDocument {
                expected: vector.required_doc_version,
                current: current_state.document_version,
            };
        }

        if current_state.permission_version != vector.required_perm_version {
            return CausalEvaluationResult::StalePermission {
                expected: vector.required_perm_version,
                current: current_state.permission_version,
            };
        }

        if current_state.config_version != vector.required_config_version {
            return CausalEvaluationResult::StaleConfig {
                expected: vector.required_config_version,
                current: current_state.config_version,
            };
        }

        if current_state.model_version != vector.required_model_version {
            return CausalEvaluationResult::StaleModel {
                expected: vector.required_model_version,
                current: current_state.model_version,
            };
        }

        if current_epoch > vector.max_staleness_epoch {
            return CausalEvaluationResult::ExpiredEpoch {
                max_epoch: vector.max_staleness_epoch,
                current_epoch,
            };
        }

        CausalEvaluationResult::Fresh {
            confidence_score: 1.0,
        }
    }

    /// Computes decayed temporal confidence score according to specified decay policy.
    pub fn compute_confidence(
        created_at: DateTime<Utc>,
        now: DateTime<Utc>,
        policy: ConfidenceDecayPolicy,
    ) -> f32 {
        let elapsed = (now - created_at).num_seconds().max(0) as f32;
        match policy {
            ConfidenceDecayPolicy::None => 1.0,
            ConfidenceDecayPolicy::Linear { ttl_seconds } => {
                let ttl = ttl_seconds as f32;
                if elapsed >= ttl {
                    0.0
                } else {
                    1.0 - (elapsed / ttl)
                }
            }
            ConfidenceDecayPolicy::Exponential { lambda } => (-lambda * elapsed).exp(),
        }
    }
}
