//! # Graph-Based Memory Cache & Real-Time Causal (RTC) Invalidation Engine
//!
//! Pairs in-process L1 memory exact caching (`ExactCacheStore` / `MemoryExactCache`)
//! directly with the multi-hop `DependencyGraph` and the SIMD-accelerated 64-bit
//! `CompressedLineageGraph` (Roaring Treemap).
//!
//! Enables zero-delay Real-Time Causal (RTC) invalidation: mutating an upstream knowledge
//! source or document hash immediately purges all directly and transitively dependent
//! cache entries from memory in $O(k)$ time without polling or background batch delay.

use crate::bitmap_lineage::CompressedLineageGraph;
use crate::graph::DependencyGraph;
use async_trait::async_trait;
use roaring::RoaringTreemap;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use sunborn_cache::{CacheEntry, ExactCacheStore, MemoryExactCache};
use sunborn_core::{Result, SunbornError};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Statistics representing active graph cache topology and lineage associations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphCacheStats {
    pub total_sources: usize,
    pub total_edges: usize,
    pub total_doc_hashes: usize,
    pub total_doc_associations: usize,
}

/// Graph-based memory cache linking in-memory key-value caching with causal lineage graphs.
#[derive(Clone)]
pub struct GraphMemoryCache {
    cache: Arc<dyn ExactCacheStore>,
    dependency_graph: Arc<DependencyGraph>,
    lineage_graph: Arc<CompressedLineageGraph>,
    record_to_keys: Arc<RwLock<HashMap<u64, HashSet<String>>>>,
}

impl Default for GraphMemoryCache {
    fn default() -> Self {
        Self::new_default()
    }
}

impl GraphMemoryCache {
    /// Creates a graph memory cache wrapping a default in-memory store.
    pub fn new_default() -> Self {
        Self {
            cache: Arc::new(MemoryExactCache::new()),
            dependency_graph: Arc::new(DependencyGraph::new()),
            lineage_graph: Arc::new(CompressedLineageGraph::new()),
            record_to_keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Creates a graph memory cache wrapping an arbitrary ExactCacheStore and DependencyGraph.
    pub fn new(
        cache: Arc<dyn ExactCacheStore>,
        dependency_graph: Arc<DependencyGraph>,
        lineage_graph: Arc<CompressedLineageGraph>,
    ) -> Self {
        Self {
            cache,
            dependency_graph,
            lineage_graph,
            record_to_keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Access the underlying ExactCacheStore.
    pub fn cache(&self) -> &Arc<dyn ExactCacheStore> {
        &self.cache
    }

    /// Access the underlying DependencyGraph.
    pub fn dependency_graph(&self) -> &Arc<DependencyGraph> {
        &self.dependency_graph
    }

    /// Access the underlying CompressedLineageGraph.
    pub fn lineage_graph(&self) -> &Arc<CompressedLineageGraph> {
        &self.lineage_graph
    }

    /// Stores a cache entry in memory and binds it to causal knowledge sources and document hashes.
    /// Implements atomic two-phase admission (SB-010): if dependency registration or cache storage fails,
    /// partial state is rolled back compensatorily to prevent dangling or orphaned lineage.
    pub async fn set_with_lineage(
        &self,
        entry: CacheEntry,
        source_ids: &[Uuid],
        doc_hashes: &[u64],
        record_id: u64,
    ) -> Result<()> {
        let key = entry.key.clone();

        // Phase 1: Record multi-source dependencies in the knowledge graph
        let mut recorded_sources = Vec::new();
        for &source_id in source_ids {
            match self
                .dependency_graph
                .record_dependency(source_id, &key)
                .await
            {
                Ok(_) => recorded_sources.push(source_id),
                Err(err) => {
                    // Rollback previously recorded source dependencies
                    for src in recorded_sources {
                        let _ = self.dependency_graph.remove_dependency(&src, &key).await;
                    }
                    return Err(err);
                }
            }
        }

        // Phase 2: Link document hashes in compressed Roaring bitmap lineage
        if !doc_hashes.is_empty() {
            self.lineage_graph.link_documents(doc_hashes, record_id);
            let mut r2k = self.record_to_keys.write().await;
            r2k.entry(record_id).or_default().insert(key.clone());
        }

        // Phase 3: Store entry in memory cache (atomic compensation on failure)
        if let Err(err) = self.cache.set(entry).await {
            // Rollback graph dependencies
            for src in recorded_sources {
                let _ = self.dependency_graph.remove_dependency(&src, &key).await;
            }
            if !doc_hashes.is_empty() {
                let mut r2k = self.record_to_keys.write().await;
                if let Some(set) = r2k.get_mut(&record_id) {
                    set.remove(&key);
                    if set.is_empty() {
                        r2k.remove(&record_id);
                    }
                }
            }
            return Err(err);
        }

        Ok(())
    }

    /// Real-Time Causal (RTC) Invalidation for a knowledge source:
    /// Traverses the multi-hop dependency graph, purges all affected cache entries
    /// from memory immediately, and clears graph edges only for confirmed purged keys (SB-003).
    pub async fn invalidate_source_rtc(&self, source_id: &Uuid) -> Result<Vec<String>> {
        let affected_keys = self
            .dependency_graph
            .get_affected_cache_keys(source_id)
            .await;

        let mut successfully_purged = Vec::new();
        let mut failed_purges = Vec::new();

        for key in &affected_keys {
            match self.cache.delete(key).await {
                Ok(_) => {
                    successfully_purged.push(key.clone());
                    let _ = self
                        .dependency_graph
                        .remove_dependency(source_id, key)
                        .await;
                }
                Err(err) => {
                    tracing::error!(
                        source_id = %source_id,
                        key = %key,
                        error = %err,
                        "Failed to delete cache key during RTC invalidation; preserving graph dependency"
                    );
                    failed_purges.push((key.clone(), err));
                }
            }
        }

        if failed_purges.is_empty() {
            self.dependency_graph.clear_dependencies(source_id).await;
            Ok(successfully_purged)
        } else {
            Err(SunbornError::internal(format!(
                "RTC invalidation partially failed for source {}: {}/{} keys purged, {} failed",
                source_id,
                successfully_purged.len(),
                affected_keys.len(),
                failed_purges.len()
            )))
        }
    }

    /// Real-Time Causal (RTC) Invalidation for document hashes with full cache eviction:
    /// Evaluates SIMD Roaring Treemap bitmap unions in $O(k)$ time, resolves affected
    /// cache keys, purges them from memory cache, and returns both affected record IDs and purged keys.
    pub async fn invalidate_docs_rtc_purge(
        &self,
        doc_hashes: &[u64],
    ) -> Result<(RoaringTreemap, Vec<String>)> {
        let affected_records = self.lineage_graph.invalidate_documents(doc_hashes);
        let mut purged_keys = Vec::new();

        let mut r2k = self.record_to_keys.write().await;
        for record_id in &affected_records {
            if let Some(keys) = r2k.remove(&record_id) {
                for key in keys {
                    let _ = self.cache.delete(&key).await;
                    purged_keys.push(key);
                }
            }
        }
        purged_keys.sort();
        purged_keys.dedup();

        Ok((affected_records, purged_keys))
    }

    /// Real-Time Causal (RTC) Invalidation for document hashes:
    /// Evaluates SIMD Roaring Treemap bitmap unions in $O(k)$ time and returns affected record IDs.
    pub fn invalidate_docs_rtc(&self, doc_hashes: &[u64]) -> RoaringTreemap {
        self.lineage_graph.invalidate_documents(doc_hashes)
    }

    /// Returns aggregated statistics across dependency graph and compressed roaring lineage.
    pub async fn stats(&self) -> GraphCacheStats {
        let (total_sources, total_edges) = self.dependency_graph.get_graph_stats().await;
        GraphCacheStats {
            total_sources,
            total_edges,
            total_doc_hashes: self.lineage_graph.document_count(),
            total_doc_associations: self.lineage_graph.total_associations(),
        }
    }

    /// Queries upstream sources for a given cache key.
    pub async fn get_upstream_sources(&self, cache_key: &str) -> Vec<Uuid> {
        self.dependency_graph.get_upstream_sources(cache_key).await
    }
}

#[async_trait]
impl ExactCacheStore for GraphMemoryCache {
    async fn get(&self, key: &str) -> Result<Option<CacheEntry>> {
        self.cache.get(key).await
    }

    async fn set(&self, entry: CacheEntry) -> Result<()> {
        self.cache.set(entry).await
    }

    async fn delete(&self, key: &str) -> Result<bool> {
        self.cache.delete(key).await
    }

    async fn exists(&self, key: &str) -> Result<bool> {
        self.cache.exists(key).await
    }

    async fn delete_by_prefix(&self, prefix: &str) -> Result<usize> {
        self.cache.delete_by_prefix(prefix).await
    }

    async fn count_by_prefix(&self, prefix: &str) -> Result<usize> {
        self.cache.count_by_prefix(prefix).await
    }

    async fn sweep_expired(&self) -> Result<usize> {
        self.cache.sweep_expired().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_graph_memory_cache_store_and_rtc_invalidation() {
        let cache = GraphMemoryCache::new_default();
        let src_a = Uuid::new_v4();
        let src_b = Uuid::new_v4();

        // 1. Insert entry 1 tied to src_a and doc_hash 1001
        let entry1 = CacheEntry::new(
            "cache:doc:1",
            "Content for Document 1 regarding Q3 financial revenue",
            Some(3600),
        );
        cache
            .set_with_lineage(entry1, &[src_a], &[1001, 1002], 42)
            .await
            .unwrap();

        // 2. Insert entry 2 tied to src_b and doc_hash 2001
        let entry2 = CacheEntry::new(
            "cache:doc:2",
            "Content for Document 2 regarding HR benefits policy",
            Some(3600),
        );
        cache
            .set_with_lineage(entry2, &[src_b], &[2001], 84)
            .await
            .unwrap();

        // Verify both entries exist in memory
        assert!(cache.exists("cache:doc:1").await.unwrap());
        assert!(cache.exists("cache:doc:2").await.unwrap());

        // 3. Trigger Real-Time Causal (RTC) Invalidation for src_a
        let purged = cache.invalidate_source_rtc(&src_a).await.unwrap();
        assert_eq!(purged, vec!["cache:doc:1".to_string()]);

        // Assert doc:1 is removed from memory cache, doc:2 remains untouched
        assert!(!cache.exists("cache:doc:1").await.unwrap());
        assert!(cache.exists("cache:doc:2").await.unwrap());

        // 4. Trigger SIMD Roaring Treemap RTC invalidation for doc_hashes [2001]
        let (affected_records, doc_keys) = cache.invalidate_docs_rtc_purge(&[2001]).await.unwrap();
        assert_eq!(affected_records.len(), 1);
        assert!(affected_records.contains(84));
        assert_eq!(doc_keys, vec!["cache:doc:2".to_string()]);
        assert!(!cache.exists("cache:doc:2").await.unwrap());
    }

    #[tokio::test]
    async fn test_graph_memory_cache_transitive_rtc() {
        let cache = GraphMemoryCache::new_default();
        let src_policy = Uuid::new_v4();

        let parent_key = "cache:policy:summary";
        let child_key = "cache:policy:slide_deck";

        let entry_parent = CacheEntry::new(parent_key, "Policy executive summary", Some(3600));
        let entry_child = CacheEntry::new(child_key, "Slide deck presentation", Some(3600));

        // Both depend on src_policy
        cache
            .set_with_lineage(entry_parent, &[src_policy], &[5001], 101)
            .await
            .unwrap();
        cache
            .set_with_lineage(entry_child, &[src_policy], &[5001], 102)
            .await
            .unwrap();

        assert!(cache.exists(parent_key).await.unwrap());
        assert!(cache.exists(child_key).await.unwrap());

        // RTC Invalidation purges both dependent keys in real time
        let purged = cache.invalidate_source_rtc(&src_policy).await.unwrap();
        assert_eq!(purged.len(), 2);
        assert!(!cache.exists(parent_key).await.unwrap());
        assert!(!cache.exists(child_key).await.unwrap());
    }
}
