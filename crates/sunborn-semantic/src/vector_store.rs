use crate::embedding::{cosine_similarity, Vector};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use sunborn_core::{ApplicationId, Result, TenantId};
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VectorEntry {
    pub id: String,
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub prompt: String,
    pub content: String,
    pub vector: Vector,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ScoredVectorEntry {
    pub entry: VectorEntry,
    pub score: f32,
}

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn insert(&self, entry: VectorEntry) -> Result<()>;
    async fn search(
        &self,
        tenant_id: &TenantId,
        application_id: &ApplicationId,
        query_vector: &[f32],
        top_k: usize,
        min_score: f32,
    ) -> Result<Vec<ScoredVectorEntry>>;
    async fn delete(&self, id: &str) -> Result<bool>;
    async fn count(&self, tenant_id: &TenantId) -> Result<usize>;
    async fn delete_by_tenant(&self, tenant_id: &TenantId) -> Result<usize>;
    async fn all_entries(&self) -> Result<Vec<VectorEntry>>;
}

#[derive(Clone, Default)]
pub struct MemoryVectorStore {
    entries: Arc<RwLock<Vec<VectorEntry>>>,
}

impl MemoryVectorStore {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

#[async_trait]
impl VectorStore for MemoryVectorStore {
    async fn insert(&self, entry: VectorEntry) -> Result<()> {
        let mut entries = self.entries.write().await;
        // Replace existing entry if id matches
        if let Some(pos) = entries.iter().position(|e| e.id == entry.id) {
            entries[pos] = entry;
        } else {
            entries.push(entry);
        }
        Ok(())
    }

    async fn search(
        &self,
        tenant_id: &TenantId,
        application_id: &ApplicationId,
        query_vector: &[f32],
        top_k: usize,
        min_score: f32,
    ) -> Result<Vec<ScoredVectorEntry>> {
        let entries = self.entries.read().await;

        let mut scored: Vec<ScoredVectorEntry> = entries
            .iter()
            .filter(|e| e.tenant_id == *tenant_id && e.application_id == *application_id)
            .map(|e| {
                let score = cosine_similarity(query_vector, &e.vector);
                ScoredVectorEntry {
                    entry: e.clone(),
                    score,
                }
            })
            .filter(|s| s.score >= min_score)
            .collect();

        // Sort descending by score
        scored.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        scored.truncate(top_k);

        Ok(scored)
    }

    async fn delete(&self, id: &str) -> Result<bool> {
        let mut entries = self.entries.write().await;
        if let Some(pos) = entries.iter().position(|e| e.id == id) {
            entries.remove(pos);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    async fn count(&self, tenant_id: &TenantId) -> Result<usize> {
        let entries = self.entries.read().await;
        let count = entries.iter().filter(|e| e.tenant_id == *tenant_id).count();
        Ok(count)
    }

    async fn delete_by_tenant(&self, tenant_id: &TenantId) -> Result<usize> {
        let mut entries = self.entries.write().await;
        let before = entries.len();
        entries.retain(|e| e.tenant_id != *tenant_id);
        Ok(before - entries.len())
    }

    async fn all_entries(&self) -> Result<Vec<VectorEntry>> {
        let entries = self.entries.read().await;
        Ok(entries.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::{DeterministicEmbeddingProvider, EmbeddingProvider};

    #[tokio::test]
    async fn test_vector_search_with_tenant_isolation() {
        let provider = DeterministicEmbeddingProvider::new();
        let store = MemoryVectorStore::new();

        let v1 = provider.embed("What is semantic caching?").await.unwrap();
        let v2 = provider.embed("What is semantic caching?").await.unwrap();

        // Insert for Tenant A
        store
            .insert(VectorEntry {
                id: "entry-1".into(),
                tenant_id: TenantId::new("tenant-a"),
                application_id: ApplicationId::new("app-1"),
                prompt: "What is semantic caching?".into(),
                content: "Semantic caching reuses LLM responses.".into(),
                vector: v1,
                metadata: serde_json::json!({}),
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        // Query for Tenant A -> Hit
        let results_a = store
            .search(
                &TenantId::new("tenant-a"),
                &ApplicationId::new("app-1"),
                &v2,
                5,
                0.8,
            )
            .await
            .unwrap();
        assert_eq!(results_a.len(), 1);
        assert_eq!(
            results_a[0].entry.content,
            "Semantic caching reuses LLM responses."
        );

        // Query for Tenant B with identical vector -> Strictly Isolated (0 hits)
        let results_b = store
            .search(
                &TenantId::new("tenant-b"),
                &ApplicationId::new("app-1"),
                &v2,
                5,
                0.8,
            )
            .await
            .unwrap();
        assert_eq!(
            results_b.len(),
            0,
            "Cross-tenant vector search must yield 0 results"
        );
    }
}
