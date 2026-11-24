use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Runtime observation record capturing execution evidence and outcome.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionRecord {
    pub query_id: String,
    pub tenant_id: String,
    pub application_id: String,
    pub semantic_category: String,
    pub similarity_score: f32,
    pub was_reused: bool,
    pub reported_false_hit: bool,
    pub latency_origin_ms: f64,
    pub latency_cache_ms: f64,
    pub token_cost_saved_usd: f64,
    pub timestamp_epoch_ms: i64,
}

/// Sliding window of telemetry observations for offline replay and evaluation.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TelemetryWindow {
    records: Vec<ExecutionRecord>,
}

impl TelemetryWindow {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
        }
    }

    pub fn record(&mut self, rec: ExecutionRecord) {
        self.records.push(rec);
    }

    pub fn records(&self) -> &[ExecutionRecord] {
        &self.records
    }

    pub fn total_queries(&self) -> usize {
        self.records.len()
    }

    pub fn hit_count(&self) -> usize {
        self.records.iter().filter(|r| r.was_reused).count()
    }

    pub fn hit_rate(&self) -> f64 {
        if self.records.is_empty() {
            0.0
        } else {
            self.hit_count() as f64 / self.records.len() as f64
        }
    }

    pub fn false_hit_count(&self) -> usize {
        self.records.iter().filter(|r| r.reported_false_hit).count()
    }

    pub fn empirical_error_rate(&self) -> f64 {
        if self.records.is_empty() {
            0.0
        } else {
            self.false_hit_count() as f64 / self.records.len() as f64
        }
    }

    pub fn category_distribution(&self) -> HashMap<String, usize> {
        let mut counts = HashMap::new();
        for r in &self.records {
            *counts.entry(r.semantic_category.clone()).or_insert(0) += 1;
        }
        counts
    }
}
