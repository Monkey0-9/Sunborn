/// Anomaly scoring for vector similarity patterns and cluster density shifts.
#[derive(Debug, Clone, Default)]
pub struct EmbeddingAnomalyDetector;

impl EmbeddingAnomalyDetector {
    pub fn new() -> Self {
        Self
    }

    /// Calculates anomaly score based on similarity value, length discrepancy, and variance.
    pub fn score_discrepancy(
        &self,
        original_len: usize,
        candidate_len: usize,
        similarity: f32,
    ) -> f32 {
        let max_len = original_len.max(candidate_len) as f32;
        let min_len = original_len.min(candidate_len) as f32;
        let length_ratio = if max_len > 0.0 {
            min_len / max_len
        } else {
            1.0
        };

        // High similarity with massive length discrepancy indicates injected payload
        if similarity > 0.90 && length_ratio < 0.5 {
            1.0 - length_ratio
        } else {
            0.0
        }
    }
}
