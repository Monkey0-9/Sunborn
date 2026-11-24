use crate::cost_model::CostComponents;
use serde::{Deserialize, Serialize};

/// High-level FinOps optimization objective governing cache admission and reuse policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum OptimizationObjective {
    /// Maximize net economic value: reuse whenever V_reuse > 0.
    #[default]
    LowestCost,
    /// Optimize for low tail latency: prioritize cached paths if overhead < target.
    LowestLatency,
    /// Minimize risk: abstain unless false-hit risk penalty is minimal.
    HighestReliability,
    /// Balance cost reduction with safety margin.
    Balanced,
    /// Zero-tolerance: require 100% cryptographic certainty and zero risk penalty.
    StrictestCorrectness,
}

/// Economic optimizer evaluating reuse decisions against defined FinOps goals.
#[derive(Debug, Clone)]
pub struct EconomicOptimizer {
    pub objective: OptimizationObjective,
}

impl Default for EconomicOptimizer {
    fn default() -> Self {
        Self::new(OptimizationObjective::LowestCost)
    }
}

impl EconomicOptimizer {
    pub fn new(objective: OptimizationObjective) -> Self {
        Self { objective }
    }

    /// Evaluates whether a reuse decision satisfies the configured optimization objective.
    pub fn should_reuse(&self, costs: CostComponents) -> bool {
        let net_benefit = costs.calculate_net_benefit();

        match self.objective {
            OptimizationObjective::LowestCost => net_benefit > 0.0,
            OptimizationObjective::LowestLatency => {
                // In LowestLatency mode, reuse even if small cost parity, provided not net negative
                net_benefit >= -0.0001
            }
            OptimizationObjective::HighestReliability => {
                net_benefit > 0.0 && costs.risk_expected_penalty_usd < 0.00005
            }
            OptimizationObjective::Balanced => {
                net_benefit > 0.0005 && costs.risk_expected_penalty_usd < 0.0002
            }
            OptimizationObjective::StrictestCorrectness => {
                net_benefit > 0.0 && costs.risk_expected_penalty_usd == 0.0
            }
        }
    }

    /// Calculates FinOps return on investment (ROI) multiplier.
    pub fn calculate_roi(&self, total_avoided: f64, total_overhead: f64) -> f64 {
        if total_overhead > 0.0 {
            (total_avoided - total_overhead) / total_overhead
        } else {
            0.0
        }
    }
}
