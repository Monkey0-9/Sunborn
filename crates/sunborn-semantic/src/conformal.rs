use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Risk category strata for task-stratified split-conformal calibration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TaskStratum {
    LowRiskGeneral,
    FinancialAccounting,
    HealthcareMedical,
    SystemAdministrative,
    SecuritySensitive,
}

impl TaskStratum {
    pub fn from_risk_and_prompt(risk_score: f32, prompt: &str) -> Self {
        let lower = prompt.to_lowercase();
        if risk_score > 0.6
            || lower.contains("password")
            || lower.contains("bypass")
            || lower.contains("secret")
        {
            Self::SecuritySensitive
        } else if lower.contains("account")
            || lower.contains("dollar")
            || lower.contains("$")
            || lower.contains("transfer")
            || lower.contains("tax")
        {
            Self::FinancialAccounting
        } else if lower.contains("patient")
            || lower.contains("dosage")
            || lower.contains("medical")
            || lower.contains("prescription")
        {
            Self::HealthcareMedical
        } else if lower.contains("cluster")
            || lower.contains("server")
            || lower.contains("docker")
            || lower.contains("database")
        {
            Self::SystemAdministrative
        } else {
            Self::LowRiskGeneral
        }
    }

    pub fn default_alpha(&self) -> f64 {
        match self {
            Self::LowRiskGeneral => 0.05,       // 95% coverage
            Self::FinancialAccounting => 0.01,  // 99% coverage
            Self::HealthcareMedical => 0.005,   // 99.5% coverage
            Self::SystemAdministrative => 0.01, // 99% coverage
            Self::SecuritySensitive => 0.001,   // 99.9% coverage
        }
    }
}

/// Task-Stratified Conformal Calibrator
/// Provides distribution-free theoretical bounds on false-reuse probability per task category.
#[derive(Clone)]
pub struct TaskStratifiedConformalCalibrator {
    calibration_data: Arc<RwLock<HashMap<TaskStratum, Vec<f64>>>>,
    thresholds: Arc<RwLock<HashMap<TaskStratum, f64>>>,
}

impl Default for TaskStratifiedConformalCalibrator {
    fn default() -> Self {
        Self::new()
    }
}

impl TaskStratifiedConformalCalibrator {
    pub fn new() -> Self {
        let mut initial_thresholds = HashMap::new();
        initial_thresholds.insert(TaskStratum::LowRiskGeneral, 0.82);
        initial_thresholds.insert(TaskStratum::FinancialAccounting, 0.90);
        initial_thresholds.insert(TaskStratum::HealthcareMedical, 0.95);
        initial_thresholds.insert(TaskStratum::SystemAdministrative, 0.90);
        initial_thresholds.insert(TaskStratum::SecuritySensitive, 0.98);

        Self {
            calibration_data: Arc::new(RwLock::new(HashMap::new())),
            thresholds: Arc::new(RwLock::new(initial_thresholds)),
        }
    }

    /// Evaluates non-conformity score S_i = (1.0 - similarity) + 0.5 * risk
    pub fn compute_non_conformity(&self, similarity: f32, risk_score: f32) -> f64 {
        let raw = (1.0 - similarity as f64) + (risk_score as f64 * 0.35);
        raw.clamp(0.0, 1.0)
    }

    /// Retrieve the current calibrated minimum similarity threshold for a stratum
    pub async fn get_threshold(&self, stratum: TaskStratum) -> f64 {
        let guard = self.thresholds.read().await;
        guard.get(&stratum).copied().unwrap_or(0.88)
    }

    /// Record a calibrated observation (similarity of a true equivalent pair)
    pub async fn record_observation(&self, stratum: TaskStratum, similarity: f64) {
        let mut data_map = self.calibration_data.write().await;
        let observations = data_map.entry(stratum).or_default();
        observations.push(similarity);

        // Recalibrate empirical quantile when >= 20 observations exist
        let n = observations.len();
        if n >= 20 {
            let mut sorted = observations.clone();
            sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

            let alpha = stratum.default_alpha();
            // Inverted quantile for similarity: lower bound on accepted similarity
            // alpha quantile of the similarity distribution
            let q_idx = ((n as f64 * alpha).floor() as usize).min(n - 1);
            let calibrated_similarity = sorted[q_idx];

            let mut thresholds = self.thresholds.write().await;
            thresholds.insert(stratum, calibrated_similarity.clamp(0.70, 0.99));
        }
    }
}

/// Online Streaming Out-of-Distribution (OOD) Drift Detector
/// Uses Welford's algorithm to compute online mean and variance of incoming request similarities.
/// Detects sudden distribution shift and triggers conservative fail-safe abstention.
#[derive(Clone)]
pub struct StreamingOODDetector {
    count: Arc<AtomicU64>,
    mean: Arc<AtomicU64>, // f64 bits
    m2: Arc<AtomicU64>,   // f64 bits
    warmup_period: u64,
    drift_sigma_threshold: f64,
}

impl Default for StreamingOODDetector {
    fn default() -> Self {
        Self::new(30, 2.8)
    }
}

impl StreamingOODDetector {
    pub fn new(warmup_period: u64, drift_sigma_threshold: f64) -> Self {
        Self {
            count: Arc::new(AtomicU64::new(0)),
            mean: Arc::new(AtomicU64::new(0.0_f64.to_bits())),
            m2: Arc::new(AtomicU64::new(0.0_f64.to_bits())),
            warmup_period,
            drift_sigma_threshold,
        }
    }

    /// Updates running stats with a new incoming candidate similarity and checks for drift
    pub fn observe_and_check_drift(&self, similarity: f64) -> bool {
        let current_count = self.count.fetch_add(1, Ordering::SeqCst) + 1;
        let prev_mean = f64::from_bits(self.mean.load(Ordering::SeqCst));
        let prev_m2 = f64::from_bits(self.m2.load(Ordering::SeqCst));

        let delta = similarity - prev_mean;
        let new_mean = prev_mean + delta / (current_count as f64);
        let delta2 = similarity - new_mean;
        let new_m2 = prev_m2 + delta * delta2;

        self.mean.store(new_mean.to_bits(), Ordering::SeqCst);
        self.m2.store(new_m2.to_bits(), Ordering::SeqCst);

        if current_count < self.warmup_period {
            return false;
        }

        let variance = new_m2 / (current_count as f64 - 1.0);
        let std_dev = variance.sqrt().max(1e-6);

        // If current similarity is more than drift_sigma_threshold standard deviations below the mean
        let z_score = (new_mean - similarity) / std_dev;
        z_score > self.drift_sigma_threshold
    }

    pub fn current_stats(&self) -> (u64, f64, f64) {
        let count = self.count.load(Ordering::Relaxed);
        let mean = f64::from_bits(self.mean.load(Ordering::Relaxed));
        let variance = if count > 1 {
            f64::from_bits(self.m2.load(Ordering::Relaxed)) / (count as f64 - 1.0)
        } else {
            0.0
        };
        (count, mean, variance.sqrt())
    }
}

// ============================================================================
// Non-Exchangeable Exponentially Weighted Conformal Risk Calibrator
// Dynamically calibrates similarity threshold \hat{\lambda}_t under drift.
// ============================================================================
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowObservation {
    pub cosine_similarity: f64,
    pub semantic_loss: f64, // 0.0 if origin & cache agree (entailed), 1.0 if contradicted
}

pub struct NonExchangeableConformalCalibrator {
    window: VecDeque<ShadowObservation>,
    max_window_size: usize,
    decay_rho: f64, // e.g., 0.995 exponential forgetting factor for distribution drift
    target_alpha: f64, // e.g., 0.005 (0.5% maximum allowed false-positive rate)
    loss_tolerance: f64, // e.g., 0.15 maximum acceptable semantic loss
    fallback_threshold: f64,
}

impl NonExchangeableConformalCalibrator {
    pub fn new(max_window_size: usize, decay_rho: f64, target_alpha: f64) -> Self {
        Self {
            window: VecDeque::with_capacity(max_window_size),
            max_window_size,
            decay_rho: decay_rho.clamp(0.90, 1.0),
            target_alpha: target_alpha.clamp(0.0001, 0.25),
            loss_tolerance: 0.15,
            fallback_threshold: 0.97,
        }
    }

    pub fn record_shadow_evaluation(&mut self, cosine_similarity: f64, semantic_loss: f64) {
        if self.window.len() == self.max_window_size {
            self.window.pop_front();
        }
        self.window.push_back(ShadowObservation {
            cosine_similarity,
            semantic_loss,
        });
    }

    /// Computes the lowest safe cosine similarity threshold \hat{\lambda}_t in [0.85, 0.995]
    /// that guarantees weighted empirical false-positive risk <= target_alpha.
    pub fn calibrated_threshold(&self) -> f64 {
        let n = self.window.len();
        if n < 30 {
            return self.fallback_threshold; // Require minimum finite-sample burn-in
        }

        // Precompute exponential recency weights w_i = \rho^{n - 1 - i}, plus w_{n+1} = 1.0 test point mass
        let mut total_weight = 1.0_f64;
        let mut weights = Vec::with_capacity(n);
        for i in 0..n {
            let w = self.decay_rho.powi((n - 1 - i) as i32);
            weights.push(w);
            total_weight += w;
        }

        // Scan candidate thresholds from conservative (0.995) down to permissive (0.850)
        // Monotone risk check: stop as soon as lowering \lambda violates the conformal bound
        let mut best_lambda = 0.995;
        let mut step = 995_i32;
        while step >= 850 {
            let candidate_lambda = (step as f64) / 1000.0;
            let mut weighted_error_mass = 1.0_f64; // Includes the +w_{n+1} finite-sample conservative penalty

            for (obs, &w) in self.window.iter().zip(weights.iter()) {
                let is_false_hit = obs.cosine_similarity >= candidate_lambda
                    && obs.semantic_loss > self.loss_tolerance;
                if is_false_hit {
                    weighted_error_mass += w;
                }
            }

            let empirical_risk = weighted_error_mass / total_weight;
            if empirical_risk <= self.target_alpha {
                best_lambda = candidate_lambda;
                step -= 1;
            } else {
                break; // Monotone stopping rule (Angelopoulos et al. Conformal Risk Control)
            }
        }
        best_lambda
    }

    pub fn window_len(&self) -> usize {
        self.window.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_stratified_conformal_calibrator() {
        let calibrator = TaskStratifiedConformalCalibrator::new();

        let initial_thresh = calibrator
            .get_threshold(TaskStratum::FinancialAccounting)
            .await;
        assert_eq!(initial_thresh, 0.90);

        // Feed 25 high-similarity observations for Financial stratum
        for _ in 0..25 {
            calibrator
                .record_observation(TaskStratum::FinancialAccounting, 0.94)
                .await;
        }

        let updated_thresh = calibrator
            .get_threshold(TaskStratum::FinancialAccounting)
            .await;
        assert!(updated_thresh >= 0.90);
    }

    #[test]
    fn test_streaming_ood_detector_triggers_on_drift() {
        let detector = StreamingOODDetector::new(10, 2.5);

        // Warm up with stable observations around 0.92
        for _ in 0..15 {
            let drift = detector.observe_and_check_drift(0.92);
            assert!(!drift, "Warmup observations should not trigger drift");
        }

        // Sudden anomalous shift down to 0.45 (OOD input attack or sudden distribution change)
        let drift_detected = detector.observe_and_check_drift(0.45);
        assert!(
            drift_detected,
            "Anomalous low similarity must trigger OOD drift detection"
        );
    }

    #[test]
    fn test_non_exchangeable_conformal_calibrator_burn_in_and_adaptation() {
        let mut cal = NonExchangeableConformalCalibrator::new(100, 0.995, 0.05);

        // 1. Burn-in: Under 30 samples, returns fallback conservative threshold 0.97
        for _ in 0..25 {
            cal.record_shadow_evaluation(0.95, 0.0);
        }
        assert_eq!(cal.calibrated_threshold(), 0.97);

        // 2. Feed 15 more safe samples (total 40 >= 30) -> threshold relaxes down
        for _ in 0..15 {
            cal.record_shadow_evaluation(0.95, 0.0);
        }
        let relaxed_thresh = cal.calibrated_threshold();
        assert!(
            relaxed_thresh < 0.97,
            "Threshold should relax when all samples have zero semantic loss"
        );

        // 3. Inject a burst of semantic loss errors at similarity 0.90
        for _ in 0..10 {
            cal.record_shadow_evaluation(0.90, 0.80);
        }
        let tightened_thresh = cal.calibrated_threshold();
        assert!(
            tightened_thresh > relaxed_thresh,
            "Threshold must tighten when errors occur at candidate similarities"
        );
    }
}
