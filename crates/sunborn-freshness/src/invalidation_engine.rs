use dashmap::DashMap;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

/// Invalidation engine for tracking reverse dependencies and executing selective dependency refresh.
#[derive(Clone)]
pub struct DependencyGraphInvalidator {
    /// Mapping of Document/Source ID -> Set of dependent Cache/Execution keys
    dependencies: Arc<DashMap<Uuid, HashSet<String>>>,
}

impl Default for DependencyGraphInvalidator {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyGraphInvalidator {
    pub fn new() -> Self {
        Self {
            dependencies: Arc::new(DashMap::new()),
        }
    }

    /// Records that a cache entry depends on a given source document ID.
    pub fn record_dependency(&self, doc_id: Uuid, cache_key: impl Into<String>) {
        self.dependencies
            .entry(doc_id)
            .or_default()
            .insert(cache_key.into());
    }

    /// Invalidates all cache entries that transitively depend on the mutated source ID.
    /// Returns the list of affected cache keys for selective eviction or proactive refresh.
    pub fn invalidate_source(&self, doc_id: &Uuid) -> Vec<String> {
        match self.dependencies.remove(doc_id) {
            Some((_, keys)) => keys.into_iter().collect(),
            None => Vec::new(),
        }
    }

    /// Checks how many cache keys currently depend on a source ID.
    pub fn count_dependents(&self, doc_id: &Uuid) -> usize {
        self.dependencies.get(doc_id).map(|s| s.len()).unwrap_or(0)
    }
}
