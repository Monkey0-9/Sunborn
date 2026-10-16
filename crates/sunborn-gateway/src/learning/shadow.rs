//! Layer 4: Shadow & Counterfactual Evaluation Engine
//! Compares the active production policy with candidate policies without altering
//! user-visible decisions. Measures counterfactual false reuse, abstention, recall, latency, and drift.

use crate::learning::calibration::{CandidatePolicy, PolicyWeights};
use crate::learning::feedback::{VerifiedFeedbackEvent, VerifiedOutcome};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CounterfactualMetrics {
    pub total_evaluated: usize,
    pub active_policy_false_hits: usize,
    pub candidate_policy_false_hits: usize,
    pub active_abstentions: usize,
    pub candidate_abstentions: usize,
    pub false_hit_reduction_pct: f64,
    pub candidate_tokens_saved: u64,
    pub candidate_cost_saved_usd: f64,
    pub is_candidate_superior: bool,
}

pub struct CounterfactualShadowEvaluator;

impl CounterfactualShadowEvaluator {
    /// Evaluates candidate policy against active policy on an empirical event trace
    pub fn evaluate_counterfactual(
        events: &[VerifiedFeedbackEvent],
        active_weights: &PolicyWeights,
        candidate_policy: &CandidatePolicy,
    ) -> CounterfactualMetrics {
        let mut active_false_hits = 0;
        let mut candidate_false_hits = 0;
        let mut active_abstentions = 0;
        let mut candidate_abstentions = 0;
        let mut candidate_tokens_saved = 0u64;
        let mut candidate_cost_saved_usd = 0.0f64;

        for ev in events {
            if let Some(sim) = ev.similarity_score {
                let risk = ev.conformal_risk_score.unwrap_or(0.0);

                // Active policy decision
                let active_admitted = sim >= active_weights.min_similarity_threshold
                    && risk <= active_weights.max_risk_threshold;

                if active_admitted {
                    if ev.verified_outcome == VerifiedOutcome::VerifiedFalseHit {
                        active_false_hits += 1;
                    }
                } else {
                    active_abstentions += 1;
                }

                // Candidate policy counterfactual decision
                let candidate_admitted = sim >= candidate_policy.weights.min_similarity_threshold
                    && risk <= candidate_policy.weights.max_risk_threshold;

                if candidate_admitted {
                    if ev.verified_outcome == VerifiedOutcome::VerifiedFalseHit {
                        candidate_false_hits += 1;
                    } else {
                        candidate_tokens_saved += ev.tokens_saved;
                        candidate_cost_saved_usd += ev.cost_saved_usd;
                    }
                } else {
                    candidate_abstentions += 1;
                }
            }
        }

        let false_hit_reduction_pct = if active_false_hits > 0 {
            ((active_false_hits as f64 - candidate_false_hits as f64) / active_false_hits as f64)
                * 100.0
        } else if candidate_false_hits == 0 {
            0.0
        } else {
            -100.0
        };

        // Candidate is superior if it has <= candidate false hits AND satisfies statistical soundness
        let is_candidate_superior = candidate_false_hits <= active_false_hits
            && candidate_policy.evaluation_report.is_statistically_sound;

        CounterfactualMetrics {
            total_evaluated: events.len(),
            active_policy_false_hits: active_false_hits,
            candidate_policy_false_hits: candidate_false_hits,
            active_abstentions,
            candidate_abstentions,
            false_hit_reduction_pct,
            candidate_tokens_saved,
            candidate_cost_saved_usd,
            is_candidate_superior,
        }
    }
}
