use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use sunborn_core::{Result, SunbornError, TenantId};
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct KnowledgeSource {
    pub id: Uuid,
    pub tenant_id: TenantId,
    pub name: String,
    pub uri: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DocumentVersion {
    pub id: Uuid,
    pub source_id: Uuid,
    pub version_tag: String,
    pub content_hash: String,
}

/// Abstract Knowledge Graph Store for tracking fine-grained knowledge dependencies
/// and transitive invalidation paths across multi-tenant cache topologies.
#[async_trait]
pub trait KnowledgeGraphStore: Send + Sync {
    /// Identifier of the active graph backend (e.g. "in-memory", "neo4j-cypher", "mock-cypher")
    fn backend_name(&self) -> &str;

    /// Record a dependency relationship: cache_key depends on knowledge source_id
    async fn record_dependency(&self, source_id: Uuid, cache_key: &str) -> Result<()>;

    /// Find all cache keys directly or transitively dependent on a knowledge source
    async fn get_affected_cache_keys(&self, source_id: &Uuid) -> Result<Vec<String>>;

    /// Remove a single dependency edge between a knowledge source and a cache key
    async fn remove_dependency(&self, source_id: &Uuid, cache_key: &str) -> Result<()>;

    /// Clear recorded dependencies for a source after invalidation
    async fn clear_dependencies(&self, source_id: &Uuid) -> Result<()>;

    /// Execute a raw or parameterized Cypher query against the graph backend
    async fn execute_cypher(
        &self,
        cypher: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value>;

    /// Connectivity / liveness health check
    async fn is_healthy(&self) -> bool;

    /// Find all upstream knowledge sources that a cache key depends on
    async fn get_upstream_sources(&self, _cache_key: &str) -> Result<Vec<Uuid>> {
        Ok(Vec::new())
    }

    /// Retrieve summary statistics: (number of sources, number of dependency edges)
    async fn get_graph_stats(&self) -> Result<(usize, usize)> {
        Ok((0, 0))
    }

    /// Record a key-to-key dependency edge: child_key transitively depends on parent_key
    async fn record_key_dependency(&self, _child_key: &str, _parent_key: &str) -> Result<()> {
        Ok(())
    }
}

/// In-memory hash-based knowledge graph implementation for high-speed local development and testing.
///
/// Both maps live behind one lock so a reader observes a single consistent
/// snapshot. Two independent locks allowed a concurrent key-edge write between
/// the dependency read and the hierarchy read, which could truncate a
/// transitive invalidation set for that call.
/// Upper bound on live lineage edges held by one in-memory graph.
///
/// Lineage grew without limit before: every cached response records an edge and
/// nothing reclaimed them unless the owning source was invalidated, so a long
/// lived process leaked memory monotonically. 1,000,000 edges is far beyond any
/// working set observed in testing while keeping worst-case memory bounded.
pub const DEFAULT_MAX_EDGES: usize = 1_000_000;

fn capacity_error(max_edges: usize) -> SunbornError {
    SunbornError::capacity_exceeded(format!(
        "dependency graph edge capacity exhausted ({max_edges} edges); invalidate lineage before recording more"
    ))
}

#[derive(Clone)]
pub struct InMemoryGraphStore {
    state: Arc<RwLock<InMemoryGraphState>>,
    max_edges: usize,
}

impl Default for InMemoryGraphStore {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
struct InMemoryGraphState {
    dependencies: HashMap<Uuid, HashSet<String>>,
    key_hierarchy: HashMap<String, HashSet<String>>,
    /// Live edges across both maps. Maintained incrementally so the cap check in
    /// `record_dependency` stays O(1) instead of summing the maps on every write.
    edge_count: usize,
}

impl InMemoryGraphStore {
    pub fn new() -> Self {
        Self::with_max_edges(DEFAULT_MAX_EDGES)
    }

    pub fn with_max_edges(max_edges: usize) -> Self {
        Self {
            state: Arc::new(RwLock::new(InMemoryGraphState::default())),
            max_edges,
        }
    }

    pub fn max_edges(&self) -> usize {
        self.max_edges
    }

    /// Remove hierarchy edges that no live source dependency can reach.
    ///
    /// Called while holding the write lock after a source is cleared. A shared
    /// key remains reachable through its other source, so pruning is safe.
    fn prune_unreachable_hierarchy(state: &mut InMemoryGraphState) {
        let mut reachable: HashSet<String> = state
            .dependencies
            .values()
            .flat_map(|keys| keys.iter().cloned())
            .collect();
        let mut frontier: Vec<String> = reachable.iter().cloned().collect();
        while let Some(parent) = frontier.pop() {
            if let Some(children) = state.key_hierarchy.get(&parent) {
                for child in children {
                    if reachable.insert(child.clone()) {
                        frontier.push(child.clone());
                    }
                }
            }
        }
        state.key_hierarchy.retain(|parent, children| {
            children.retain(|child| reachable.contains(child));
            reachable.contains(parent) && !children.is_empty()
        });
        // Recompute rather than subtract: pruning drops whole sets and is the
        // one path where a hand-maintained decrement would silently drift.
        state.edge_count = state.dependencies.values().map(HashSet::len).sum::<usize>()
            + state
                .key_hierarchy
                .values()
                .map(HashSet::len)
                .sum::<usize>();
    }

    #[cfg(test)]
    async fn hierarchy_len(&self) -> usize {
        self.state.read().await.key_hierarchy.len()
    }
}

#[async_trait]
impl KnowledgeGraphStore for InMemoryGraphStore {
    fn backend_name(&self) -> &str {
        "in-memory"
    }

    async fn record_dependency(&self, source_id: Uuid, cache_key: &str) -> Result<()> {
        let mut state = self.state.write().await;
        let already_present = state
            .dependencies
            .get(&source_id)
            .is_some_and(|keys| keys.contains(cache_key));
        if !already_present && state.edge_count >= self.max_edges {
            // Refuse rather than evict: dropping an existing edge would leave a
            // cached response with no lineage, so it would survive the very
            // invalidation it should have caught.
            return Err(capacity_error(self.max_edges));
        }
        if state
            .dependencies
            .entry(source_id)
            .or_default()
            .insert(cache_key.to_string())
            && !already_present
        {
            state.edge_count += 1;
        }
        Ok(())
    }

    async fn record_key_dependency(&self, child_key: &str, parent_key: &str) -> Result<()> {
        let mut state = self.state.write().await;
        let already_present = state
            .key_hierarchy
            .get(parent_key)
            .is_some_and(|children| children.contains(child_key));
        if !already_present && state.edge_count >= self.max_edges {
            return Err(capacity_error(self.max_edges));
        }
        if state
            .key_hierarchy
            .entry(parent_key.to_string())
            .or_default()
            .insert(child_key.to_string())
            && !already_present
        {
            state.edge_count += 1;
        }
        Ok(())
    }

    async fn get_affected_cache_keys(&self, source_id: &Uuid) -> Result<Vec<String>> {
        let state = self.state.read().await;
        let mut affected = HashSet::new();

        if let Some(direct) = state.dependencies.get(source_id) {
            for key in direct {
                affected.insert(key.clone());
            }
        }

        let mut frontier: Vec<String> = affected.iter().cloned().collect();
        for _ in 0..10 {
            let mut next_frontier = Vec::new();
            for item in frontier {
                if let Some(children) = state.key_hierarchy.get(&item) {
                    for child in children {
                        if affected.insert(child.clone()) {
                            next_frontier.push(child.clone());
                        }
                    }
                }
            }
            if next_frontier.is_empty() {
                break;
            }
            frontier = next_frontier;
        }

        Ok(affected.into_iter().collect())
    }

    async fn remove_dependency(&self, source_id: &Uuid, cache_key: &str) -> Result<()> {
        let mut state = self.state.write().await;
        let removed = state
            .dependencies
            .get_mut(source_id)
            .map(|set| {
                let removed = set.remove(cache_key);
                let empty = set.is_empty();
                (removed, empty)
            })
            .unwrap_or((false, false));
        if removed.0 {
            state.edge_count = state.edge_count.saturating_sub(1);
        }
        if removed.1 {
            state.dependencies.remove(source_id);
        }
        Self::prune_unreachable_hierarchy(&mut state);
        Ok(())
    }

    async fn clear_dependencies(&self, source_id: &Uuid) -> Result<()> {
        let mut state = self.state.write().await;
        if let Some(removed) = state.dependencies.remove(source_id) {
            state.edge_count = state.edge_count.saturating_sub(removed.len());
        }
        Self::prune_unreachable_hierarchy(&mut state);
        Ok(())
    }

    async fn execute_cypher(
        &self,
        cypher: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "backend": "in-memory",
            "cypher": cypher,
            "params": params,
            "status": "in-memory"
        }))
    }

    async fn is_healthy(&self) -> bool {
        true
    }

    async fn get_upstream_sources(&self, cache_key: &str) -> Result<Vec<Uuid>> {
        let state = self.state.read().await;
        let sources = state
            .dependencies
            .iter()
            .filter_map(|(src, keys)| {
                if keys.contains(cache_key) {
                    Some(*src)
                } else {
                    None
                }
            })
            .collect();
        Ok(sources)
    }

    async fn get_graph_stats(&self) -> Result<(usize, usize)> {
        let state = self.state.read().await;
        let sources = state.dependencies.len();
        let edges = state.dependencies.values().map(|s| s.len()).sum();
        Ok((sources, edges))
    }
}

/// Enterprise Neo4j Cypher Graph Store communicating via transactional HTTP Cypher API.
pub struct Neo4jCypherGraphStore {
    endpoint: String,
    client: reqwest::Client,
    auth: Option<(String, String)>,
}

impl Neo4jCypherGraphStore {
    pub fn new(endpoint: impl Into<String>, auth: Option<(&str, &str)>) -> Self {
        Self {
            endpoint: endpoint.into(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(5))
                .build()
                .unwrap_or_default(),
            auth: auth.map(|(u, p)| (u.to_string(), p.to_string())),
        }
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
}

#[async_trait]
impl KnowledgeGraphStore for Neo4jCypherGraphStore {
    fn backend_name(&self) -> &str {
        "neo4j-cypher"
    }

    async fn record_dependency(&self, source_id: Uuid, cache_key: &str) -> Result<()> {
        let cypher = "MERGE (s:KnowledgeSource {id: $source_id}) \
                      MERGE (k:CacheKey {key: $cache_key}) \
                      MERGE (k)-[:DEPENDS_ON]->(s)";
        let params = serde_json::json!({
            "source_id": source_id.to_string(),
            "cache_key": cache_key
        });
        self.execute_cypher(cypher, params).await.map(|_| ())
    }

    async fn get_affected_cache_keys(&self, source_id: &Uuid) -> Result<Vec<String>> {
        let cypher =
            "MATCH (k:CacheKey)-[:DEPENDS_ON*1..10]->(s:KnowledgeSource {id: $source_id}) \
                      RETURN DISTINCT k.key AS key";
        let params = serde_json::json!({
            "source_id": source_id.to_string()
        });
        let res = self.execute_cypher(cypher, params).await?;
        let mut keys = Vec::new();
        if let Some(results) = res.get("results").and_then(|r| r.as_array()) {
            for result in results {
                if let Some(data) = result.get("data").and_then(|d| d.as_array()) {
                    for row in data {
                        if let Some(row_val) = row.get("row").and_then(|r| r.as_array()) {
                            if let Some(key_str) = row_val.first().and_then(|k| k.as_str()) {
                                keys.push(key_str.to_string());
                            }
                        }
                    }
                }
            }
        }
        Ok(keys)
    }

    async fn remove_dependency(&self, source_id: &Uuid, cache_key: &str) -> Result<()> {
        let cypher = "MATCH (k:CacheKey {key: $cache_key})-[r:DEPENDS_ON]->(s:KnowledgeSource {id: $source_id}) DELETE r";
        let params = serde_json::json!({
            "source_id": source_id.to_string(),
            "cache_key": cache_key,
        });
        self.execute_cypher(cypher, params).await.map(|_| ())
    }

    async fn record_key_dependency(&self, child_key: &str, parent_key: &str) -> Result<()> {
        let cypher = "MERGE (p:CacheKey {key: $parent_key}) \
                      MERGE (c:CacheKey {key: $child_key}) \
                      MERGE (c)-[:DEPENDS_ON]->(p)";
        let params = serde_json::json!({
            "parent_key": parent_key,
            "child_key": child_key,
        });
        self.execute_cypher(cypher, params).await.map(|_| ())
    }

    async fn clear_dependencies(&self, source_id: &Uuid) -> Result<()> {
        let cypher = "MATCH (s:KnowledgeSource {id: $source_id}) DETACH DELETE s";
        let params = serde_json::json!({
            "source_id": source_id.to_string()
        });
        self.execute_cypher(cypher, params).await.map(|_| ())
    }

    async fn execute_cypher(
        &self,
        cypher: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let payload = serde_json::json!({
            "statements": [
                {
                    "statement": cypher,
                    "parameters": params
                }
            ]
        });

        let mut req = self.client.post(&self.endpoint).json(&payload);
        if let Some((user, pass)) = &self.auth {
            req = req.basic_auth(user, Some(pass));
        }

        let resp = req
            .send()
            .await
            .map_err(|e| SunbornError::internal(format!("Neo4j connection error: {e}")))?;

        if !resp.status().is_success() {
            return Err(SunbornError::internal(format!(
                "Neo4j Cypher query failed with status: {}",
                resp.status()
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SunbornError::internal(format!("Failed to parse Neo4j response: {e}")))?;

        if let Some(errors) = body.get("errors").and_then(|e| e.as_array()) {
            if !errors.is_empty() {
                return Err(SunbornError::internal(format!(
                    "Neo4j Cypher error: {:?}",
                    errors
                )));
            }
        }

        Ok(body)
    }

    async fn is_healthy(&self) -> bool {
        let cypher = "RETURN 1 AS ping";
        self.execute_cypher(cypher, serde_json::json!({}))
            .await
            .is_ok()
    }
}

/// Deterministic Mock Neo4j Cypher Store for verifying Cypher query syntax,
/// parameter bindings, and transitive graph traversals in offline CI test environments.
pub struct MockNeo4jCypherStore {
    queries: Arc<RwLock<Vec<(String, serde_json::Value)>>>,
    direct_edges: Arc<RwLock<HashMap<Uuid, HashSet<String>>>>,
    key_hierarchy: Arc<RwLock<HashMap<String, HashSet<String>>>>,
}

impl Default for MockNeo4jCypherStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MockNeo4jCypherStore {
    pub fn new() -> Self {
        Self {
            queries: Arc::new(RwLock::new(Vec::new())),
            direct_edges: Arc::new(RwLock::new(HashMap::new())),
            key_hierarchy: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add a transitive key dependency edge: `child_key` depends on `parent_key`
    pub async fn add_transitive_key_edge(
        &self,
        child_key: impl Into<String>,
        parent_key: impl Into<String>,
    ) {
        let mut hierarchy = self.key_hierarchy.write().await;
        hierarchy
            .entry(parent_key.into())
            .or_default()
            .insert(child_key.into());
    }

    /// Retrieve recorded Cypher queries for assertion
    pub async fn recorded_queries(&self) -> Vec<(String, serde_json::Value)> {
        self.queries.read().await.clone()
    }
}

#[async_trait]
impl KnowledgeGraphStore for MockNeo4jCypherStore {
    fn backend_name(&self) -> &str {
        "mock-neo4j-cypher"
    }

    async fn record_dependency(&self, source_id: Uuid, cache_key: &str) -> Result<()> {
        let cypher = "MERGE (s:KnowledgeSource {id: $source_id}) \
                      MERGE (k:CacheKey {key: $cache_key}) \
                      MERGE (k)-[:DEPENDS_ON]->(s)";
        let params = serde_json::json!({
            "source_id": source_id.to_string(),
            "cache_key": cache_key
        });
        self.execute_cypher(cypher, params).await?;
        let mut edges = self.direct_edges.write().await;
        edges
            .entry(source_id)
            .or_default()
            .insert(cache_key.to_string());
        Ok(())
    }

    async fn record_key_dependency(&self, child_key: &str, parent_key: &str) -> Result<()> {
        self.add_transitive_key_edge(child_key, parent_key).await;
        Ok(())
    }

    async fn get_affected_cache_keys(&self, source_id: &Uuid) -> Result<Vec<String>> {
        let cypher =
            "MATCH (k:CacheKey)-[:DEPENDS_ON*1..10]->(s:KnowledgeSource {id: $source_id}) \
                      RETURN DISTINCT k.key AS key";
        let params = serde_json::json!({
            "source_id": source_id.to_string()
        });
        self.execute_cypher(cypher, params).await?;

        let edges = self.direct_edges.read().await;
        let mut affected = HashSet::new();

        if let Some(direct) = edges.get(source_id) {
            for key in direct {
                affected.insert(key.clone());
            }
        }

        let hierarchy = self.key_hierarchy.read().await;
        let mut frontier: Vec<String> = affected.iter().cloned().collect();
        for _ in 0..10 {
            let mut next_frontier = Vec::new();
            for item in frontier {
                if let Some(children) = hierarchy.get(&item) {
                    for child in children {
                        if affected.insert(child.clone()) {
                            next_frontier.push(child.clone());
                        }
                    }
                }
            }
            if next_frontier.is_empty() {
                break;
            }
            frontier = next_frontier;
        }

        Ok(affected.into_iter().collect())
    }

    async fn remove_dependency(&self, source_id: &Uuid, cache_key: &str) -> Result<()> {
        let cypher = "MATCH (k:CacheKey {key: $cache_key})-[r:DEPENDS_ON]->(s:KnowledgeSource {id: $source_id}) DELETE r";
        let params = serde_json::json!({
            "source_id": source_id.to_string(),
            "cache_key": cache_key,
        });
        self.execute_cypher(cypher, params).await?;
        let mut edges = self.direct_edges.write().await;
        if let Some(set) = edges.get_mut(source_id) {
            set.remove(cache_key);
            if set.is_empty() {
                edges.remove(source_id);
            }
        }
        Ok(())
    }

    async fn clear_dependencies(&self, source_id: &Uuid) -> Result<()> {
        let cypher = "MATCH (s:KnowledgeSource {id: $source_id}) DETACH DELETE s";
        let params = serde_json::json!({
            "source_id": source_id.to_string()
        });
        self.execute_cypher(cypher, params).await?;
        let mut edges = self.direct_edges.write().await;
        edges.remove(source_id);
        Ok(())
    }

    async fn execute_cypher(
        &self,
        cypher: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let mut queries = self.queries.write().await;
        queries.push((cypher.to_string(), params));
        Ok(serde_json::json!({
            "status": "success",
            "results": [
                {
                    "columns": ["result"],
                    "data": []
                }
            ],
            "errors": []
        }))
    }

    async fn is_healthy(&self) -> bool {
        true
    }

    async fn get_upstream_sources(&self, cache_key: &str) -> Result<Vec<Uuid>> {
        let edges = self.direct_edges.read().await;
        let sources = edges
            .iter()
            .filter_map(|(src, keys)| {
                if keys.contains(cache_key) {
                    Some(*src)
                } else {
                    None
                }
            })
            .collect();
        Ok(sources)
    }

    async fn get_graph_stats(&self) -> Result<(usize, usize)> {
        let edges = self.direct_edges.read().await;
        let sources = edges.len();
        let edge_count = edges.values().map(|s| s.len()).sum();
        Ok((sources, edge_count))
    }
}

/// Dependency graph tracking which cache keys depend on which document/source versions.
/// Wraps a polymorphic KnowledgeGraphStore (In-Memory, Neo4j Cypher, or Mock Cypher).
#[derive(Clone)]
pub struct DependencyGraph {
    store: Arc<dyn KnowledgeGraphStore>,
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            store: Arc::new(InMemoryGraphStore::new()),
        }
    }

    pub fn with_store(store: Arc<dyn KnowledgeGraphStore>) -> Self {
        Self { store }
    }

    pub fn new_neo4j(endpoint: impl Into<String>, auth: Option<(&str, &str)>) -> Self {
        Self {
            store: Arc::new(Neo4jCypherGraphStore::new(endpoint, auth)),
        }
    }

    pub fn store(&self) -> &Arc<dyn KnowledgeGraphStore> {
        &self.store
    }

    /// Record a dependency: cache_key depends on knowledge source_id
    pub async fn record_dependency(
        &self,
        source_id: Uuid,
        cache_key: impl Into<String>,
    ) -> Result<()> {
        let key = cache_key.into();
        self.store.record_dependency(source_id, &key).await
    }

    /// Record a key-to-key dependency edge: child_key transitively depends on parent_key
    pub async fn record_key_dependency(
        &self,
        child_key: impl Into<String>,
        parent_key: impl Into<String>,
    ) -> Result<()> {
        self.store
            .record_key_dependency(&child_key.into(), &parent_key.into())
            .await
    }

    /// Find all cache keys that must be invalidated when source_id is updated.
    ///
    /// Errors are propagated rather than collapsed into an empty set: treating a
    /// backend outage as "nothing depends on this source" would report a purge
    /// as successful while leaving stale entries in place, and a caller that then
    /// clears the dependency edges would destroy the lineage needed to retry.
    pub async fn get_affected_cache_keys(&self, source_id: &Uuid) -> Result<Vec<String>> {
        self.store.get_affected_cache_keys(source_id).await
    }

    /// Remove a single dependency edge between a knowledge source and a cache key
    pub async fn remove_dependency(&self, source_id: &Uuid, cache_key: &str) -> Result<()> {
        self.store.remove_dependency(source_id, cache_key).await
    }

    /// Clear recorded dependencies for a source after invalidation.
    /// Failures are surfaced so a caller never reports lineage cleanup it did
    /// not actually perform.
    pub async fn clear_dependencies(&self, source_id: &Uuid) -> Result<()> {
        self.store.clear_dependencies(source_id).await
    }

    /// Find all upstream knowledge sources that a cache key depends on
    pub async fn get_upstream_sources(&self, cache_key: &str) -> Vec<Uuid> {
        self.store
            .get_upstream_sources(cache_key)
            .await
            .unwrap_or_default()
    }

    /// Retrieve summary statistics: (number of sources, number of dependency edges)
    pub async fn get_graph_stats(&self) -> (usize, usize) {
        self.store.get_graph_stats().await.unwrap_or((0, 0))
    }

    /// Execute a Cypher query against the active graph backend
    pub async fn execute_cypher(
        &self,
        cypher: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.store.execute_cypher(cypher, params).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_dependency_graph_lookup() {
        let graph = DependencyGraph::new();
        let src_id = Uuid::new_v4();

        graph.record_dependency(src_id, "key:alpha").await.unwrap();
        graph.record_dependency(src_id, "key:beta").await.unwrap();

        let affected = graph.get_affected_cache_keys(&src_id).await.unwrap();
        assert_eq!(affected.len(), 2);
        assert!(affected.contains(&"key:alpha".to_string()));
        assert!(affected.contains(&"key:beta".to_string()));

        graph.clear_dependencies(&src_id).await.unwrap();
        let after_clear = graph.get_affected_cache_keys(&src_id).await.unwrap();
        assert_eq!(after_clear.len(), 0);
    }

    #[tokio::test]
    async fn test_in_memory_transitive_invalidation() {
        let graph = DependencyGraph::new();
        let src_id = Uuid::new_v4();

        let root_key = "report:summary";
        let child_key1 = "report:chart";
        let child_key2 = "report:table";
        let grandchild_key = "report:widget";

        // Direct dependency: root_key depends on src_id
        graph.record_dependency(src_id, root_key).await.unwrap();

        // Transitive dependencies: child_key1 & child_key2 depend on root_key
        graph
            .record_key_dependency(child_key1, root_key)
            .await
            .unwrap();
        graph
            .record_key_dependency(child_key2, root_key)
            .await
            .unwrap();

        // 2-hop transitive: grandchild_key depends on child_key1
        graph
            .record_key_dependency(grandchild_key, child_key1)
            .await
            .unwrap();

        let affected = graph.get_affected_cache_keys(&src_id).await.unwrap();
        assert_eq!(
            affected.len(),
            4,
            "In-memory graph must traverse multi-hop transitive keys"
        );
        assert!(affected.contains(&root_key.to_string()));
        assert!(affected.contains(&child_key1.to_string()));
        assert!(affected.contains(&child_key2.to_string()));
        assert!(affected.contains(&grandchild_key.to_string()));
    }

    #[tokio::test]
    async fn test_clear_dependencies_prunes_orphaned_key_hierarchy() {
        let store = InMemoryGraphStore::new();
        let source = Uuid::new_v4();
        store
            .record_dependency(source, "report:root")
            .await
            .unwrap();
        store
            .record_key_dependency("report:child", "report:root")
            .await
            .unwrap();
        assert_eq!(store.hierarchy_len().await, 1);

        store.clear_dependencies(&source).await.unwrap();
        assert_eq!(
            store.hierarchy_len().await,
            0,
            "orphaned key edges must not accumulate after invalidation"
        );
    }

    #[tokio::test]
    async fn test_clear_keeps_shared_key_hierarchy_reachable() {
        let store = InMemoryGraphStore::new();
        let source_a = Uuid::new_v4();
        let source_b = Uuid::new_v4();
        for source in [source_a, source_b] {
            store
                .record_dependency(source, "report:root")
                .await
                .unwrap();
        }
        store
            .record_key_dependency("report:child", "report:root")
            .await
            .unwrap();

        store.clear_dependencies(&source_a).await.unwrap();
        assert_eq!(
            store.hierarchy_len().await,
            1,
            "a key still reachable from another source must keep its edges"
        );
        let affected = store.get_affected_cache_keys(&source_b).await.unwrap();
        assert!(affected.contains(&"report:child".to_string()));

        store.clear_dependencies(&source_b).await.unwrap();
        assert_eq!(store.hierarchy_len().await, 0);
    }

    #[tokio::test]
    async fn test_mock_neo4j_cypher_transitive_invalidation_and_audit() {
        let mock_store = Arc::new(MockNeo4jCypherStore::new());
        let graph = DependencyGraph::with_store(mock_store.clone());

        let source_id = Uuid::new_v4();
        let root_key = "sunborn:financial_q3:report";
        let child_key1 = "sunborn:financial_q3:chart_ebitda";
        let child_key2 = "sunborn:financial_q3:slide_executive_summary";

        // 1. Record direct dependency: root_key depends on source_id
        graph.record_dependency(source_id, root_key).await.unwrap();

        // 2. Add transitive dependencies: child_key1 and child_key2 depend on root_key
        mock_store
            .add_transitive_key_edge(child_key1, root_key)
            .await;
        mock_store
            .add_transitive_key_edge(child_key2, child_key1)
            .await;

        // 3. Multi-hop transitive graph traversal resolves all 3 affected keys
        let affected = graph.get_affected_cache_keys(&source_id).await.unwrap();
        assert_eq!(
            affected.len(),
            3,
            "Transitive invalidation must traverse multi-hop graph"
        );
        assert!(affected.contains(&root_key.to_string()));
        assert!(affected.contains(&child_key1.to_string()));
        assert!(affected.contains(&child_key2.to_string()));

        // 4. Assert that exact Cypher statements were generated and executed
        let queries = mock_store.recorded_queries().await;
        assert!(queries
            .iter()
            .any(|(q, _)| q.contains("MERGE (s:KnowledgeSource") && q.contains("[:DEPENDS_ON]")));
        assert!(queries
            .iter()
            .any(|(q, _)| q.contains("MATCH (k:CacheKey)-[:DEPENDS_ON*1..10]->")));

        // 5. Clear dependencies triggers DETACH DELETE
        graph.clear_dependencies(&source_id).await.unwrap();
        let queries_after_clear = mock_store.recorded_queries().await;
        assert!(queries_after_clear
            .iter()
            .any(|(q, _)| q.contains("DETACH DELETE")));

        let affected_after_clear = graph.get_affected_cache_keys(&source_id).await.unwrap();
        assert_eq!(affected_after_clear.len(), 0);
    }

    /// A backend that is unreachable must surface the error instead of being
    /// reported as "no dependents", otherwise an invalidation would silently
    /// succeed and the caller would clear the lineage needed for a retry.
    #[tokio::test]
    async fn test_affected_keys_propagate_backend_failure() {
        struct UnavailableStore;

        #[async_trait::async_trait]
        impl KnowledgeGraphStore for UnavailableStore {
            fn backend_name(&self) -> &str {
                "unavailable"
            }
            async fn record_dependency(&self, _s: Uuid, _k: &str) -> Result<()> {
                Err(SunbornError::internal("graph backend down"))
            }
            async fn get_affected_cache_keys(&self, _s: &Uuid) -> Result<Vec<String>> {
                Err(SunbornError::internal("graph backend down"))
            }
            async fn remove_dependency(&self, _s: &Uuid, _k: &str) -> Result<()> {
                Err(SunbornError::internal("graph backend down"))
            }
            async fn clear_dependencies(&self, _s: &Uuid) -> Result<()> {
                Err(SunbornError::internal("graph backend down"))
            }
            async fn execute_cypher(
                &self,
                _c: &str,
                _p: serde_json::Value,
            ) -> Result<serde_json::Value> {
                Err(SunbornError::internal("graph backend down"))
            }
            async fn is_healthy(&self) -> bool {
                false
            }
        }

        let graph = DependencyGraph::with_store(Arc::new(UnavailableStore));
        let source_id = Uuid::new_v4();

        let err = graph
            .get_affected_cache_keys(&source_id)
            .await
            .expect_err("backend failure must not be reported as an empty dependant set");
        assert!(err.to_string().contains("graph backend down"));
        assert!(graph.clear_dependencies(&source_id).await.is_err());
    }

    #[tokio::test]
    async fn test_neo4j_cypher_store_configuration() {
        let store = Neo4jCypherGraphStore::new(
            "http://neo4j-cluster.corp:7474/db/neo4j/tx/commit",
            Some(("neo4j", "secretpass")),
        );
        assert_eq!(store.backend_name(), "neo4j-cypher");
        assert_eq!(
            store.endpoint(),
            "http://neo4j-cluster.corp:7474/db/neo4j/tx/commit"
        );
    }
}

#[cfg(test)]
mod capacity_tests {
    use super::*;
    use sunborn_core::ErrorCode;

    fn bounded(max_edges: usize) -> DependencyGraph {
        DependencyGraph::with_store(Arc::new(InMemoryGraphStore::with_max_edges(max_edges)))
    }

    #[tokio::test]
    async fn edge_capacity_is_enforced_and_refuses_rather_than_evicts() {
        let graph = bounded(2);
        let src = Uuid::new_v4();

        graph.record_dependency(src, "key:one").await.unwrap();
        graph.record_dependency(src, "key:two").await.unwrap();

        let err = graph
            .record_dependency(src, "key:three")
            .await
            .expect_err("third edge must be refused at capacity");
        assert_eq!(err.code, ErrorCode::CapacityExceeded);

        // Existing lineage must survive: refusing the write must not evict.
        let mut affected = graph.get_affected_cache_keys(&src).await.unwrap();
        affected.sort();
        assert_eq!(affected, vec!["key:one".to_string(), "key:two".to_string()]);
    }

    #[tokio::test]
    async fn re_recording_an_existing_edge_is_idempotent_at_capacity() {
        let graph = bounded(1);
        let src = Uuid::new_v4();

        graph.record_dependency(src, "key:one").await.unwrap();
        // Same edge again: no new lineage, so it must not be rejected.
        graph.record_dependency(src, "key:one").await.unwrap();

        assert_eq!(graph.get_affected_cache_keys(&src).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn removing_lineage_releases_capacity() {
        let graph = bounded(1);
        let src = Uuid::new_v4();

        graph.record_dependency(src, "key:one").await.unwrap();
        assert!(graph.record_dependency(src, "key:two").await.is_err());

        graph.remove_dependency(&src, "key:one").await.unwrap();
        graph
            .record_dependency(src, "key:two")
            .await
            .expect("capacity freed by removal must be reusable");
    }

    #[tokio::test]
    async fn clearing_a_source_releases_all_of_its_capacity() {
        let graph = bounded(2);
        let src = Uuid::new_v4();

        graph.record_dependency(src, "key:one").await.unwrap();
        graph.record_dependency(src, "key:two").await.unwrap();
        graph.clear_dependencies(&src).await.unwrap();

        graph.record_dependency(src, "key:three").await.unwrap();
        graph.record_dependency(src, "key:four").await.unwrap();
    }

    #[tokio::test]
    async fn key_hierarchy_edges_share_the_same_budget() {
        let graph = bounded(2);
        let src = Uuid::new_v4();

        graph.record_dependency(src, "root").await.unwrap();
        graph.record_key_dependency("child", "root").await.unwrap();

        let err = graph
            .record_key_dependency("grandchild", "child")
            .await
            .expect_err("hierarchy edges must consume the same budget");
        assert_eq!(err.code, ErrorCode::CapacityExceeded);
    }

    #[tokio::test]
    async fn default_graph_is_bounded_and_reports_its_limit() {
        let store = InMemoryGraphStore::new();
        assert_eq!(store.max_edges(), DEFAULT_MAX_EDGES);
        const { assert!(DEFAULT_MAX_EDGES >= 100_000) };
    }
}
