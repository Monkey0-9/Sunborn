use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use sunborn_core::{Result, TenantId};
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

/// Dependency graph tracking which cache keys depend on which document/source versions
#[derive(Clone, Default)]
pub struct DependencyGraph {
    // source_id -> set of cache keys
    dependencies: Arc<RwLock<HashMap<Uuid, HashSet<String>>>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            dependencies: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Record a dependency: cache_key depends on knowledge source_id
    pub async fn record_dependency(
        &self,
        source_id: Uuid,
        cache_key: impl Into<String>,
    ) -> Result<()> {
        let mut deps = self.dependencies.write().await;
        deps.entry(source_id).or_default().insert(cache_key.into());
        Ok(())
    }

    /// Find all cache keys that must be invalidated when source_id is updated
    pub async fn get_affected_cache_keys(&self, source_id: &Uuid) -> Vec<String> {
        let deps = self.dependencies.read().await;
        deps.get(source_id)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Clear recorded dependencies for a source after invalidation
    pub async fn clear_dependencies(&self, source_id: &Uuid) {
        let mut deps = self.dependencies.write().await;
        deps.remove(source_id);
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
}
