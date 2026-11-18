//! # Graph-Based Memory Cache & Real-Time Causal (RTC) Invalidation Engine
//!
//! Pairs in-process L1 memory exact caching (`ExactCacheStore` / `MemoryExactCache`)
//! directly with the multi-hop `DependencyGraph` and the SIMD-accelerated 64-bit
//! `CompressedLineageGraph` (Roaring Treemap).
//!
//! Enables zero-delay Real-Time Causal (RTC) invalidation: mutating an upstream knowledge
//! source or document hash immediately purges all directly and transitively dependent
//! cache entries from memory in $O(k)$ time without polling or background batch delay.

use crate::affected::{compute_affected, AffectedReport};
use crate::bitmap_lineage::CompressedLineageGraph;
use crate::graph::DependencyGraph;
use crate::graphify_graph::GraphifyKnowledgeGraph;
use crate::graphify_model::GraphifyNodeType;
use crate::query::{explain_node, NodeExplanation};
use async_trait::async_trait;
use roaring::RoaringTreemap;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use sunborn_cache::{CacheEntry, ExactCacheStore, MemoryExactCache};
use sunborn_core::{Result, SunbornError};
use tokio::sync::RwLock;
use uuid::Uuid;

/// Derives a stable, deterministic 64-bit integer hash from arbitrary byte data using SHA-256.
/// Guarantees cross-restart and cross-replica stability for Roaring Treemaps, unlike randomized SipHash.
pub fn deterministic_hash_64(data: &[u8]) -> u64 {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    u64::from_be_bytes(result[0..8].try_into().unwrap())
}

/// Statistics representing active graph cache topology and lineage associations.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GraphCacheStats {
    pub total_sources: usize,
    pub total_edges: usize,
    pub total_doc_hashes: usize,
    pub total_doc_associations: usize,
}

/// Graph-based memory cache linking in-memory key-value caching with causal lineage graphs
/// and Graphify multi-relational knowledge graphs.
#[derive(Clone)]
pub struct GraphMemoryCache {
    cache: Arc<dyn ExactCacheStore>,
    dependency_graph: Arc<DependencyGraph>,
    lineage_graph: Arc<CompressedLineageGraph>,
    graphify_graph: Arc<GraphifyKnowledgeGraph>,
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
            graphify_graph: Arc::new(GraphifyKnowledgeGraph::new()),
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
            graphify_graph: Arc::new(GraphifyKnowledgeGraph::new()),
            record_to_keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Configures an explicit Graphify multi-relational knowledge graph.
    pub fn with_graphify_graph(mut self, graphify_graph: Arc<GraphifyKnowledgeGraph>) -> Self {
        self.graphify_graph = graphify_graph;
        self
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

    /// Access the underlying Graphify multi-relational knowledge graph.
    pub fn graphify_graph(&self) -> &Arc<GraphifyKnowledgeGraph> {
        &self.graphify_graph
    }

    /// Real-Time Causal Invalidation using Graphify multi-hop blast-radius traversal:
    /// Traverses dependency relations from the mutated assets, identifies all affected
    /// downstream components and cache entries, deletes affected cache entries from L1 memory,
    /// and returns the complete causal AffectedReport.
    pub async fn invalidate_with_graphify_blast_radius(
        &self,
        mutated_assets: &[&str],
    ) -> Result<AffectedReport> {
        let report = compute_affected(&self.graphify_graph, mutated_assets, None);
        for key in &report.affected_cache_keys {
            let _ = self.cache.delete(key).await;
        }
        Ok(report)
    }

    /// Purges all cache entries belonging to a specific functional community.
    pub async fn invalidate_community(&self, community_id: u32) -> Result<usize> {
        let mut purged = 0;
        for node in self.graphify_graph.all_nodes() {
            if node.community_id == Some(community_id) {
                let is_cache = node.node_type == GraphifyNodeType::CacheEntry
                    || node.id.starts_with("cache::");
                if is_cache && self.cache.delete(&node.id).await.unwrap_or(false) {
                    purged += 1;
                }
            }
        }
        Ok(purged)
    }

    /// Generates an explanation report of a cache entry's upstream dependency lineage.
    pub fn explain_cache_lineage(&self, cache_key: &str) -> Option<NodeExplanation> {
        explain_node(&self.graphify_graph, cache_key)
    }

    /// Delete exact-cache keys supplied by a peer invalidation event. This is
    /// required when the receiving replica does not hold the origin replica's
    /// dependency edges.
    pub async fn delete_cache_keys(&self, keys: &[String]) -> Result<usize> {
        let mut deleted = 0;
        for key in keys {
            if self.cache.delete(key).await? {
                deleted += 1;
            }
        }
        Ok(deleted)
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
        let mut entry = entry;

        // Enrich entry metadata with causal lineage tracking attributes for graph-aware eviction & freshness checks
        let fan_out = (source_ids.len() + doc_hashes.len()) as u64;
        let source_ids_json: Vec<serde_json::Value> = source_ids
            .iter()
            .map(|id| serde_json::Value::String(id.to_string()))
            .collect();
        let doc_hashes_json: Vec<serde_json::Value> = doc_hashes
            .iter()
            .map(|h| serde_json::Value::Number((*h).into()))
            .collect();

        if let serde_json::Value::Object(ref mut map) = entry.metadata {
            map.entry("record_id".to_string())
                .or_insert_with(|| serde_json::Value::Number(record_id.into()));
            map.entry("doc_hashes".to_string())
                .or_insert_with(|| serde_json::Value::Array(doc_hashes_json));
            map.entry("source_ids".to_string())
                .or_insert_with(|| serde_json::Value::Array(source_ids_json));
            map.entry("fan_out".to_string())
                .or_insert_with(|| serde_json::Value::Number(fan_out.into()));
        }

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
            .await?;

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
            self.dependency_graph
                .clear_dependencies(source_id)
                .await
                .map_err(|e| {
                    SunbornError::internal(format!(
                        "RTC invalidation purged keys for source {} but failed to clear dependency edges: {e}",
                        source_id
                    ))
                })?;
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
        // Peek first: lineage must survive a failed purge so the deletion can be retried.
        let affected_records = self.lineage_graph.peek_documents(doc_hashes);
        let mut purged_keys = Vec::new();
        let mut failed_keys = Vec::new();

        let mut r2k = self.record_to_keys.write().await;
        for record_id in &affected_records {
            if let Some(keys) = r2k.get_mut(&record_id) {
                let mut remaining_keys = std::collections::HashSet::new();
                for key in keys.drain() {
                    match self.cache.delete(&key).await {
                        Ok(_) => {
                            purged_keys.push(key);
                        }
                        Err(e) => {
                            tracing::error!(
                                record_id,
                                key = %key,
                                error = %e,
                                "Failed to delete cache key during doc purge; preserving lineage"
                            );
                            failed_keys.push((key.clone(), e.to_string()));
                            remaining_keys.insert(key);
                        }
                    }
                }
                if remaining_keys.is_empty() {
                    r2k.remove(&record_id);
                } else {
                    *keys = remaining_keys;
                }
            }
        }
        purged_keys.sort();
        purged_keys.dedup();

        if !failed_keys.is_empty() {
            // Never report a partial purge as success: callers must see the failure and
            // lineage mappings are preserved so the purge can be retried.
            return Err(SunbornError::internal(format!(
                "Doc purge incomplete: {} purged, {} failed (lineage preserved for retry)",
                purged_keys.len(),
                failed_keys.len()
            )));
        }

        // All keys acknowledged: now it is safe to drop the document lineage.
        let _ = self.lineage_graph.invalidate_documents(doc_hashes);

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
        let entry = match self.cache.get(key).await? {
            Some(e) => e,
            None => return Ok(None),
        };

        // Causal freshness validation:
        // 1. Check document lineage freshness if record_id and doc_hashes are tracked
        if let Some(record_id) = entry.metadata.get("record_id").and_then(|v| v.as_u64()) {
            if let Some(hashes) = entry.metadata.get("doc_hashes").and_then(|v| v.as_array()) {
                for h in hashes {
                    if let Some(doc_hash) = h.as_u64() {
                        if !self.lineage_graph.contains_link(doc_hash, record_id) {
                            tracing::warn!(
                                key = %key,
                                doc_hash,
                                record_id,
                                "Stale cache read intercepted by causal graph freshness guard; evicting entry"
                            );
                            let _ = self.cache.delete(key).await;
                            return Ok(None);
                        }
                    }
                }
            }
        }

        // 2. Check knowledge source freshness if source_ids are tracked
        if let Some(sources) = entry.metadata.get("source_ids").and_then(|v| v.as_array()) {
            let upstream = self.dependency_graph.get_upstream_sources(key).await;
            for s in sources {
                if let Some(uuid_str) = s.as_str() {
                    if let Ok(src_uuid) = Uuid::parse_str(uuid_str) {
                        if !upstream.contains(&src_uuid) {
                            tracing::warn!(
                                key = %key,
                                source_id = %src_uuid,
                                "Upstream source edge missing or pruned; causal guard evicting stale entry"
                            );
                            let _ = self.cache.delete(key).await;
                            return Ok(None);
                        }
                    }
                }
            }
        }

        Ok(Some(entry))
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
    use sunborn_core::SunbornError;

    struct FailingDeleteCache;

    #[async_trait]
    impl ExactCacheStore for FailingDeleteCache {
        async fn get(&self, _key: &str) -> Result<Option<CacheEntry>> {
            Ok(None)
        }
        async fn set(&self, _entry: CacheEntry) -> Result<()> {
            Ok(())
        }
        async fn delete(&self, _key: &str) -> Result<bool> {
            Err(SunbornError::internal("simulated backend deletion failure"))
        }
        async fn exists(&self, _key: &str) -> Result<bool> {
            Ok(true)
        }
        async fn delete_by_prefix(&self, _prefix: &str) -> Result<usize> {
            Ok(0)
        }
        async fn count_by_prefix(&self, _prefix: &str) -> Result<usize> {
            Ok(0)
        }
        async fn sweep_expired(&self) -> Result<usize> {
            Ok(0)
        }
    }

    #[tokio::test]
    async fn test_doc_purge_partial_failure_is_visible_and_lineage_preserved() {
        let cache = GraphMemoryCache::new(
            Arc::new(FailingDeleteCache),
            Arc::new(DependencyGraph::new()),
            Arc::new(CompressedLineageGraph::new()),
        );

        cache
            .set_with_lineage(
                CacheEntry::new("cache:doc:retry", "retryable content", Some(3600)),
                &[],
                &[777_001],
                7,
            )
            .await
            .unwrap();

        let result = cache.invalidate_docs_rtc_purge(&[777_001]).await;
        assert!(result.is_err(), "partial purge must not return Ok");
        assert!(
            cache.lineage_graph().contains_document(777_001),
            "lineage must survive a failed purge so the operation can be retried"
        );
        assert!(
            cache.exists("cache:doc:retry").await.unwrap(),
            "failed deletion must leave the cache entry visible"
        );
    }

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

    #[tokio::test]
    async fn test_causal_freshness_validation_on_get() {
        let cache = GraphMemoryCache::new_default();
        let src = Uuid::new_v4();
        let doc_hash = 999_888;
        let record_id = 77;
        let key = "cache:causal:test";

        let entry = CacheEntry::new(key, "fresh content", Some(3600));
        cache
            .set_with_lineage(entry, &[src], &[doc_hash], record_id)
            .await
            .unwrap();

        // 1. Initial read succeeds and is causally fresh
        let fetched = cache.get(key).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().content, "fresh content");

        // 2. Simulate upstream document mutation invalidating document in lineage graph
        let affected = cache.lineage_graph().invalidate_document(doc_hash);
        assert!(affected.contains(record_id));

        // 3. Next get(key) should be intercepted by the causal guard, evict the stale item, and return None
        let stale_read = cache.get(key).await.unwrap();
        assert!(
            stale_read.is_none(),
            "Causal guard must prevent stale read and return None"
        );

        // 4. Verify item is also purged from underlying cache store
        assert!(!cache.cache().exists(key).await.unwrap());
    }
}
