use serde::{Deserialize, Serialize};

/// High-level tunable cache policy candidate proposed by learning layer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CachePolicy {
    pub version: u32,
    pub similarity_threshold: f32,
    pub ttl_seconds: u32,
    pub risk_budget: f32,
    pub max_admission_anomaly: f32,
}

impl Default for CachePolicy {
    fn default() -> Self {
        Self {
            version: 1,
            similarity_threshold: 0.92,
            ttl_seconds: 3600,
            risk_budget: 0.05,
            max_admission_anomaly: 0.40,
        }
    }
}

/// Controlled deployment state machine for policy lifecycle.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum PolicyLifecycleState {
    Draft,
    ShadowEvaluating,
    CanaryActive { canary_percentage: f32 },
    Promoted,
    RolledBack { reason: String },
}
