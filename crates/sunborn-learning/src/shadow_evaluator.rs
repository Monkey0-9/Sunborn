use crate::observation::ExecutionRecord;
use crate::policy::CachePolicy;
use serde::{Deserialize, Serialize};

/// Strict safety gates governing offline shadow evaluation and admission to canary testing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SafetyGate {
    pub max_tolerable_error_rate: f64,
    pub min_acceptable_hit_rate: f64,
    pub min_evaluation_samples: usize,
}

impl Default for SafetyGate {
    fn default() -> Self {
        Self {
            max_tolerable_error_rate: 0.005, // 0.5% max false hit rate
            min_acceptable_hit_rate: 0.05,   // 5% minimum hit rate
            min_evaluation_samples: 20,
        }
    }
}

/// Result of evaluating a candidate policy against historical observations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ShadowEvaluationResult {
    pub approved: bool,
    pub rejection_reasons: Vec<String>,
    pub projected_hit_rate: f64,
    pub projected_error_rate: f64,
    pub projected_net_savings_multiplier: f64,
}

/// Offline replay evaluator assessing policy safety prior to deployment.
#[derive(Debug, Clone, Default)]
pub struct ShadowEvaluator;

impl ShadowEvaluator {
    pub fn new() -> Self {
        Self
    }

    /// Evaluates candidate policy against historical telemetry.
    pub fn evaluate(
        &self,
        candidate: &CachePolicy,
        records: &[ExecutionRecord],
        gate: &SafetyGate,
    ) -> ShadowEvaluationResult {
        let mut rejection_reasons = Vec::new();

        if records.len() < gate.min_evaluation_samples {
            rejection_reasons.push(format!(
                "Insufficient sample size: {} < required {}",
                records.len(),
                gate.min_evaluation_samples
            ));
            return ShadowEvaluationResult {
                approved: false,
                rejection_reasons,
                projected_hit_rate: 0.0,
                projected_error_rate: 0.0,
                projected_net_savings_multiplier: 0.0,
            };
        }

        // Replay: predict hits based on candidate's similarity threshold
        let mut projected_hits = 0;
        let mut projected_false_hits = 0;
        let mut total_saved = 0.0;

        for r in records {
            // A query is considered hit if its similarity satisfies candidate threshold
            let would_hit = r.similarity_score >= candidate.similarity_threshold;
            if would_hit {
                projected_hits += 1;
                // If it is a hard negative or reported false hit, replaying it as a hit is an error
                if r.reported_false_hit
                    || r.semantic_category == "negation"
                    || r.semantic_category == "numeric"
                {
                    projected_false_hits += 1;
                } else {
                    total_saved += r.token_cost_saved_usd;
                }
            }
        }

        let projected_hit_rate = projected_hits as f64 / records.len() as f64;
        let projected_error_rate = if projected_hits > 0 {
            projected_false_hits as f64 / projected_hits as f64
        } else {
            0.0
        };

        if projected_error_rate > gate.max_tolerable_error_rate {
            rejection_reasons.push(format!(
                "Projected error rate {:.4} exceeds maximum tolerable limit {:.4}",
                projected_error_rate, gate.max_tolerable_error_rate
            ));
        }

        if projected_hit_rate < gate.min_acceptable_hit_rate {
            rejection_reasons.push(format!(
                "Projected hit rate {:.4} below acceptable floor {:.4}",
                projected_hit_rate, gate.min_acceptable_hit_rate
            ));
        }

        let approved = rejection_reasons.is_empty();
        let projected_net_savings_multiplier = if total_saved > 0.0 { total_saved } else { 0.0 };

        ShadowEvaluationResult {
            approved,
            rejection_reasons,
            projected_hit_rate,
            projected_error_rate,
            projected_net_savings_multiplier,
        }
    }
}
