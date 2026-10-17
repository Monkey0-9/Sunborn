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
}

/// In-memory hash-based knowledge graph implementation for high-speed local development and testing.
#[derive(Clone, Default)]
pub struct InMemoryGraphStore {
    dependencies: Arc<RwLock<HashMap<Uuid, HashSet<String>>>>,
}

impl InMemoryGraphStore {
    pub fn new() -> Self {
        Self {
            dependencies: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl KnowledgeGraphStore for InMemoryGraphStore {
    fn backend_name(&self) -> &str {
        "in-memory"
    }

    async fn record_dependency(&self, source_id: Uuid, cache_key: &str) -> Result<()> {
        let mut deps = self.dependencies.write().await;
        deps.entry(source_id)
            .or_default()
            .insert(cache_key.to_string());
        Ok(())
    }

    async fn get_affected_cache_keys(&self, source_id: &Uuid) -> Result<Vec<String>> {
        let deps = self.dependencies.read().await;
        Ok(deps
            .get(source_id)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default())
    }

    async fn clear_dependencies(&self, source_id: &Uuid) -> Result<()> {
        let mut deps = self.dependencies.write().await;
        deps.remove(source_id);
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
            "status": "simulated"
        }))
    }

    async fn is_healthy(&self) -> bool {
        true
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
        let cypher = "MATCH (k:CacheKey)-[:DEPENDS_ON*1..5]->(s:KnowledgeSource {id: $source_id}) \
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

    async fn get_affected_cache_keys(&self, source_id: &Uuid) -> Result<Vec<String>> {
        let cypher = "MATCH (k:CacheKey)-[:DEPENDS_ON*1..5]->(s:KnowledgeSource {id: $source_id}) \
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
        for _ in 0..5 {
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

    /// Find all cache keys that must be invalidated when source_id is updated
    pub async fn get_affected_cache_keys(&self, source_id: &Uuid) -> Vec<String> {
        self.store
            .get_affected_cache_keys(source_id)
            .await
            .unwrap_or_default()
    }

    /// Clear recorded dependencies for a source after invalidation
    pub async fn clear_dependencies(&self, source_id: &Uuid) {
        let _ = self.store.clear_dependencies(source_id).await;
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

        let affected = graph.get_affected_cache_keys(&src_id).await;
        assert_eq!(affected.len(), 2);
        assert!(affected.contains(&"key:alpha".to_string()));
        assert!(affected.contains(&"key:beta".to_string()));

        graph.clear_dependencies(&src_id).await;
        let after_clear = graph.get_affected_cache_keys(&src_id).await;
        assert_eq!(after_clear.len(), 0);
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
        let affected = graph.get_affected_cache_keys(&source_id).await;
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
            .any(|(q, _)| q.contains("MATCH (k:CacheKey)-[:DEPENDS_ON*1..5]->")));

        // 5. Clear dependencies triggers DETACH DELETE
        graph.clear_dependencies(&source_id).await;
        let queries_after_clear = mock_store.recorded_queries().await;
        assert!(queries_after_clear
            .iter()
            .any(|(q, _)| q.contains("DETACH DELETE")));

        let affected_after_clear = graph.get_affected_cache_keys(&source_id).await;
        assert_eq!(affected_after_clear.len(), 0);
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
