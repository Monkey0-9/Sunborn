use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// Prediction candidate with confidence score and estimated latency savings
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrefetchPrediction {
    pub predicted_action: String,
    pub confidence_score: f64,
    pub estimated_savings_ms: u64,
}

/// Speculative Predictive Prefetching & Proactive Warming Engine
pub struct PredictivePrefetchEngine {
    // Transition graph: from_action -> (to_action -> count)
    transitions: RwLock<HashMap<String, HashMap<String, u64>>>,
    min_confidence_threshold: f64,
}

impl Default for PredictivePrefetchEngine {
    fn default() -> Self {
        Self::new(0.50)
    }
}

impl PredictivePrefetchEngine {
    pub fn new(min_confidence_threshold: f64) -> Self {
        let engine = Self {
            transitions: RwLock::new(HashMap::new()),
            min_confidence_threshold,
        };

        // Seed with standard enterprise agent workflow priors
        engine.record_transition("search_documents", "fetch_document_content");
        engine.record_transition("search_documents", "fetch_document_content");
        engine.record_transition("search_documents", "fetch_document_content");
        engine.record_transition("search_documents", "summarize_results");

        engine.record_transition("fetch_document_content", "analyze_financial_table");
        engine.record_transition("fetch_document_content", "analyze_financial_table");
        engine.record_transition("fetch_document_content", "generate_executive_brief");

        engine
    }

    /// Record an observed sequence transition from action A to action B
    pub fn record_transition(&self, from: &str, to: &str) {
        let mut map = self.transitions.write().unwrap();
        let from_entry = map.entry(from.to_string()).or_default();
        let count = from_entry.entry(to.to_string()).or_insert(0);
        *count += 1;
    }

    /// Predict the most probable next actions with confidence scores above threshold
    pub fn predict_next(&self, current_action: &str) -> Vec<PrefetchPrediction> {
        let map = self.transitions.read().unwrap();
        let Some(destinations) = map.get(current_action) else {
            return Vec::new();
        };

        let total_count: u64 = destinations.values().sum();
        if total_count == 0 {
            return Vec::new();
        }

        let mut predictions = Vec::new();
        for (action, &count) in destinations {
            let confidence = count as f64 / total_count as f64;
            if confidence >= self.min_confidence_threshold {
                predictions.push(PrefetchPrediction {
                    predicted_action: action.clone(),
                    confidence_score: (confidence * 1000.0).round() / 1000.0,
                    estimated_savings_ms: 350,
                });
            }
        }

        predictions.sort_by(|a, b| b.confidence_score.partial_cmp(&a.confidence_score).unwrap());
        predictions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_predictive_prefetch_learning_and_scoring() {
        let engine = PredictivePrefetchEngine::new(0.60);

        // Before training on new sequence
        let initial = engine.predict_next("search_documents");
        assert_eq!(initial.len(), 1);
        assert_eq!(initial[0].predicted_action, "fetch_document_content");
        assert!(initial[0].confidence_score >= 0.75);

        // Train new pattern: "code_review" -> "run_linter" 4 times, -> "run_tests" 1 time
        for _ in 0..4 {
            engine.record_transition("code_review", "run_linter");
        }
        engine.record_transition("code_review", "run_tests");

        let preds = engine.predict_next("code_review");
        assert_eq!(preds.len(), 1);
        assert_eq!(preds[0].predicted_action, "run_linter");
        assert_eq!(preds[0].confidence_score, 0.80);
    }
}
