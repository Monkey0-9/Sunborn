// ============================================================================
// crates/sunborn-invalidation/src/watermark.rs
// Watermark-Fenced Write-Behind & Granular Lineage Generation Invalidation
// Prevents false write-back aborts by tracking per-key monotonic watermarks
// rather than a single coarse global atomic counter.
// ============================================================================
use dashmap::DashMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LineageWatermark {
    pub key: String,
    pub generation: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ReadView {
    pub watermarks: Vec<LineageWatermark>,
}

impl ReadView {
    pub fn new(watermarks: Vec<LineageWatermark>) -> Self {
        Self { watermarks }
    }

    pub fn is_empty(&self) -> bool {
        self.watermarks.is_empty()
    }
}

#[derive(Clone, Default)]
pub struct WatermarkLineageFencer {
    watermarks: Arc<DashMap<String, u64>>,
}

impl WatermarkLineageFencer {
    pub fn new() -> Self {
        Self {
            watermarks: Arc::new(DashMap::new()),
        }
    }

    pub fn scoped_key(tenant_id: &str, resource_id: &str) -> String {
        format!("{}:{}", tenant_id, resource_id)
    }

    /// Snapshots the current generation of a set of lineage dependencies.
    /// Used when an origin LLM request or DAG execution starts.
    pub fn snapshot_read_view(&self, lineage_keys: &[String]) -> ReadView {
        let mut snapshots = Vec::with_capacity(lineage_keys.len());
        for key in lineage_keys {
            let gen = self.watermarks.get(key).map(|entry| *entry).unwrap_or(0);
            snapshots.push(LineageWatermark {
                key: key.clone(),
                generation: gen,
            });
        }
        ReadView {
            watermarks: snapshots,
        }
    }

    /// Monotonically advances the watermark generation for a specific lineage key.
    /// Atomically invalidates any in-flight writebacks depending on this lineage key,
    /// while leaving completely unaffected other documents and tenants.
    pub fn advance_watermark(&self, lineage_key: &str) -> u64 {
        let mut entry = self.watermarks.entry(lineage_key.to_string()).or_insert(0);
        *entry = entry.saturating_add(1);
        *entry
    }

    /// Validates an in-flight ReadView against the current watermark generations.
    /// Returns true iff every lineage key in ReadView still matches its snapshotted generation.
    pub fn validate_and_commit(&self, read_view: &ReadView) -> bool {
        for wm in &read_view.watermarks {
            let current_gen = self
                .watermarks
                .get(&wm.key)
                .map(|entry| *entry)
                .unwrap_or(0);
            if current_gen != wm.generation {
                // Monotonic generation diverged during origin call; fence off stale writeback
                return false;
            }
        }
        true
    }

    pub fn current_generation(&self, lineage_key: &str) -> u64 {
        self.watermarks
            .get(lineage_key)
            .map(|entry| *entry)
            .unwrap_or(0)
    }

    pub fn clear(&self) {
        self.watermarks.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_view_commit_success_when_unmodified() {
        let fencer = WatermarkLineageFencer::new();
        let k1 = WatermarkLineageFencer::scoped_key("tenant_1", "doc_42");
        let k2 = WatermarkLineageFencer::scoped_key("tenant_1", "doc_99");

        let read_view = fencer.snapshot_read_view(&[k1.clone(), k2.clone()]);
        assert_eq!(read_view.watermarks.len(), 2);
        assert_eq!(read_view.watermarks[0].generation, 0);

        // Origin LLM call finishes without invalidations -> safe to commit
        assert!(fencer.validate_and_commit(&read_view));
    }

    #[test]
    fn test_watermark_invalidation_aborts_only_affected_in_flight_request() {
        let fencer = WatermarkLineageFencer::new();

        let key_a = WatermarkLineageFencer::scoped_key("tenant_alpha", "doc_financials");
        let key_b = WatermarkLineageFencer::scoped_key("tenant_beta", "doc_engineering");

        // Request A (Tenant Alpha) snapshots doc_financials
        let read_view_a = fencer.snapshot_read_view(std::slice::from_ref(&key_a));

        // Request B (Tenant Beta) snapshots doc_engineering
        let read_view_b = fencer.snapshot_read_view(std::slice::from_ref(&key_b));

        // Tenant Alpha updates doc_financials -> watermark advances
        let new_gen = fencer.advance_watermark(&key_a);
        assert_eq!(new_gen, 1);

        // Request A finishes its origin call: must be ABORTED because doc_financials mutated
        assert!(
            !fencer.validate_and_commit(&read_view_a),
            "Mutated lineage key must abort write-behind"
        );

        // Request B finishes its origin call: must SUCCEED because doc_engineering was untouched!
        assert!(
            fencer.validate_and_commit(&read_view_b),
            "Unrelated tenant/document must never suffer false writeback aborts"
        );
    }
}
