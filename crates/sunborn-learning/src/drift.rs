use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Workload distribution profile over semantic categories.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkloadDistribution {
    pub category_proportions: HashMap<String, f64>,
}

impl WorkloadDistribution {
    pub fn from_counts(counts: &HashMap<String, usize>) -> Self {
        let total: usize = counts.values().sum();
        let mut category_proportions = HashMap::new();
        if total > 0 {
            for (k, v) in counts {
                category_proportions.insert(k.clone(), *v as f64 / total as f64);
            }
        }
        Self {
            category_proportions,
        }
    }
}

/// Distribution shift and concept drift detector.
#[derive(Debug, Clone)]
pub struct DriftDetector {
    baseline: WorkloadDistribution,
    divergence_threshold: f64,
}

impl DriftDetector {
    pub fn new(baseline: WorkloadDistribution, divergence_threshold: f64) -> Self {
        Self {
            baseline,
            divergence_threshold,
        }
    }

    /// Calculates Total Variation Distance (TVD) between baseline and observed distribution.
    pub fn total_variation_distance(&self, observed: &WorkloadDistribution) -> f64 {
        let mut keys: std::collections::HashSet<String> =
            self.baseline.category_proportions.keys().cloned().collect();
        keys.extend(observed.category_proportions.keys().cloned());

        let mut sum_diff = 0.0;
        for k in keys {
            let p = self
                .baseline
                .category_proportions
                .get(&k)
                .copied()
                .unwrap_or(0.0);
            let q = observed
                .category_proportions
                .get(&k)
                .copied()
                .unwrap_or(0.0);
            sum_diff += (p - q).abs();
        }

        0.5 * sum_diff
    }

    /// Determines whether observed distribution has drifted sufficiently to trigger recalibration.
    pub fn detect_drift(&self, observed: &WorkloadDistribution) -> bool {
        self.total_variation_distance(observed) >= self.divergence_threshold
    }
}
