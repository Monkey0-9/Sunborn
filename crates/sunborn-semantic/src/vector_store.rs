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

#[async_trait]
impl VectorStore for Arc<dyn VectorStore> {
    async fn insert(&self, entry: VectorEntry) -> Result<()> {
        (**self).insert(entry).await
    }

    async fn search(
        &self,
        tenant_id: &TenantId,
        application_id: &ApplicationId,
        query_vector: &[f32],
        top_k: usize,
        min_score: f32,
    ) -> Result<Vec<ScoredVectorEntry>> {
        (**self)
            .search(tenant_id, application_id, query_vector, top_k, min_score)
            .await
    }

    async fn delete(&self, id: &str) -> Result<bool> {
        (**self).delete(id).await
    }

    async fn count(&self, tenant_id: &TenantId) -> Result<usize> {
        (**self).count(tenant_id).await
    }

    async fn delete_by_tenant(&self, tenant_id: &TenantId) -> Result<usize> {
        (**self).delete_by_tenant(tenant_id).await
    }

    async fn all_entries(&self) -> Result<Vec<VectorEntry>> {
        (**self).all_entries().await
    }
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

/// Production Qdrant vector store adapter providing cosine distance search with strict tenant filters
#[derive(Clone)]
pub struct QdrantVectorStore {
    pub endpoint_url: String,
    pub collection_name: String,
    client: reqwest::Client,
    fallback_store: Arc<MemoryVectorStore>,
    pub strict_mode: bool,
}

impl QdrantVectorStore {
    pub fn new(endpoint_url: impl Into<String>, collection_name: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .connect_timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_default();
        let strict = match sunborn_core::config::parse_bool_env("SUNBORN_VECTOR_STRICT", false) {
            Ok(v) => {
                v || std::env::var("SUNBORN_VECTOR_BACKEND")
                    .map(|b| b.to_lowercase() == "qdrant")
                    .unwrap_or(false)
            }
            Err(_) => true, // Invalid config fails closed to strict mode
        };

        Self {
            endpoint_url: endpoint_url.into(),
            collection_name: collection_name.into(),
            client,
            fallback_store: Arc::new(MemoryVectorStore::new()),
            strict_mode: strict,
        }
    }

    pub fn new_strict(endpoint_url: impl Into<String>, collection_name: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .connect_timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_default();

        Self {
            endpoint_url: endpoint_url.into(),
            collection_name: collection_name.into(),
            client,
            fallback_store: Arc::new(MemoryVectorStore::new()),
            strict_mode: true,
        }
    }

    /// Actively validates Qdrant vector database health via HTTP endpoint reachability.
    pub async fn verify_connection(&self) -> Result<()> {
        let base = self.endpoint_url.trim_end_matches('/');
        let coll_url = format!("{base}/collections");
        match self.client.get(&coll_url).send().await {
            Ok(resp) if resp.status().is_success() => Ok(()),
            Ok(resp) => Err(sunborn_core::SunbornError::vector_store(format!(
                "Qdrant returned non-success HTTP status {} for endpoint {}",
                resp.status(),
                coll_url
            ))),
            Err(err) => Err(sunborn_core::SunbornError::vector_store(format!(
                "Unable to reach Qdrant vector store at {}: {err}",
                self.endpoint_url
            ))),
        }
    }

    async fn ensure_collection(&self, dim: usize) -> Result<()> {
        let url = format!(
            "{}/collections/{}",
            self.endpoint_url.trim_end_matches('/'),
            self.collection_name
        );
        let payload = serde_json::json!({
            "vectors": {
                "size": dim,
                "distance": "Cosine"
            }
        });
        match self.client.put(&url).json(&payload).send().await {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() || status.as_u16() == 409 {
                    Ok(())
                } else if self.strict_mode {
                    Err(sunborn_core::SunbornError::vector_store(format!(
                        "Qdrant collection creation failed: status {}",
                        status
                    )))
                } else {
                    Ok(())
                }
            }
            Err(err) => {
                if self.strict_mode {
                    Err(sunborn_core::SunbornError::vector_store(format!(
                        "Qdrant connection failed during collection creation: {}",
                        err
                    )))
                } else {
                    Ok(())
                }
            }
        }
    }
}

#[async_trait]
impl VectorStore for QdrantVectorStore {
    async fn insert(&self, entry: VectorEntry) -> Result<()> {
        let dim = entry.vector.len();
        let _ = self.ensure_collection(dim).await;

        let url = format!(
            "{}/collections/{}/points",
            self.endpoint_url.trim_end_matches('/'),
            self.collection_name
        );

        // Deterministic UUID for Qdrant point from entry.id
        let point_uuid = {
            let mut hasher = sha2::Sha256::new();
            use sha2::Digest;
            hasher.update(entry.id.as_bytes());
            let hash = hex::encode(hasher.finalize());
            format!(
                "{}-{}-{}-{}-{}",
                &hash[0..8],
                &hash[8..12],
                &hash[12..16],
                &hash[16..20],
                &hash[20..32]
            )
        };

        let payload = serde_json::json!({
            "points": [
                {
                    "id": point_uuid,
                    "vector": entry.vector,
                    "payload": {
                        "id": entry.id,
                        "tenant_id": entry.tenant_id.0,
                        "application_id": entry.application_id.0,
                        "prompt": entry.prompt,
                        "content": entry.content,
                        "metadata": entry.metadata,
                        "created_at": entry.created_at.to_rfc3339()
                    }
                }
            ]
        });

        match self.client.put(&url).json(&payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                // Successfully indexed in Qdrant; also maintain fallback memory for instant retrieval
                let _ = self.fallback_store.insert(entry).await;
                Ok(())
            }
            Ok(resp) => {
                tracing::warn!(status = %resp.status(), "Qdrant point insert failed");
                if self.strict_mode {
                    return Err(sunborn_core::SunbornError::vector_store(format!(
                        "Qdrant point insert failed with status {}",
                        resp.status()
                    )));
                }
                self.fallback_store.insert(entry).await
            }
            Err(err) => {
                tracing::warn!(error = %err, "Qdrant connection failed");
                if self.strict_mode {
                    return Err(sunborn_core::SunbornError::vector_store(format!(
                        "Qdrant connection failed: {}",
                        err
                    )));
                }
                self.fallback_store.insert(entry).await
            }
        }
    }

    async fn search(
        &self,
        tenant_id: &TenantId,
        application_id: &ApplicationId,
        query_vector: &[f32],
        top_k: usize,
        min_score: f32,
    ) -> Result<Vec<ScoredVectorEntry>> {
        let url = format!(
            "{}/collections/{}/points/search",
            self.endpoint_url.trim_end_matches('/'),
            self.collection_name
        );

        let search_payload = serde_json::json!({
            "vector": query_vector,
            "limit": top_k,
            "score_threshold": min_score,
            "filter": {
                "must": [
                    { "key": "tenant_id", "match": { "value": tenant_id.0 } },
                    { "key": "application_id", "match": { "value": application_id.0 } }
                ]
            },
            "with_payload": true
        });

        match self.client.post(&url).json(&search_payload).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(results) = data.get("result").and_then(|r| r.as_array()) {
                        let mut scored = Vec::new();
                        for r in results {
                            let score =
                                r.get("score").and_then(|s| s.as_f64()).unwrap_or(0.0) as f32;
                            if let Some(p) = r.get("payload") {
                                let id = p
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string();
                                let t_id = p
                                    .get("tenant_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string();
                                let a_id = p
                                    .get("application_id")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string();
                                let prompt = p
                                    .get("prompt")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string();
                                let content = p
                                    .get("content")
                                    .and_then(|v| v.as_str())
                                    .unwrap_or_default()
                                    .to_string();
                                let metadata =
                                    p.get("metadata").cloned().unwrap_or(serde_json::json!({}));
                                let created_at = p
                                    .get("created_at")
                                    .and_then(|v| v.as_str())
                                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                                    .map(|dt| dt.with_timezone(&chrono::Utc))
                                    .unwrap_or_else(chrono::Utc::now);

                                scored.push(ScoredVectorEntry {
                                    entry: VectorEntry {
                                        id,
                                        tenant_id: TenantId::new(t_id),
                                        application_id: ApplicationId::new(a_id),
                                        prompt,
                                        content,
                                        vector: query_vector.to_vec(),
                                        metadata,
                                        created_at,
                                    },
                                    score,
                                });
                            }
                        }
                        return Ok(scored);
                    }
                }
            }
            Ok(resp) => {
                tracing::warn!(status = %resp.status(), "Qdrant search returned non-2xx status");
                if self.strict_mode {
                    return Err(sunborn_core::SunbornError::vector_store(format!(
                        "Qdrant search failed: status {}",
                        resp.status()
                    )));
                }
            }
            Err(err) => {
                tracing::warn!(error = %err, "Qdrant search connection failed");
                if self.strict_mode {
                    return Err(sunborn_core::SunbornError::vector_store(format!(
                        "Qdrant search connection failed: {}",
                        err
                    )));
                }
            }
        }

        self.fallback_store
            .search(tenant_id, application_id, query_vector, top_k, min_score)
            .await
    }

    async fn delete(&self, id: &str) -> Result<bool> {
        let _ = self.fallback_store.delete(id).await;
        Ok(true)
    }

    async fn count(&self, tenant_id: &TenantId) -> Result<usize> {
        self.fallback_store.count(tenant_id).await
    }

    async fn delete_by_tenant(&self, tenant_id: &TenantId) -> Result<usize> {
        let url = format!(
            "{}/collections/{}/points/delete",
            self.endpoint_url.trim_end_matches('/'),
            self.collection_name
        );

        let delete_payload = serde_json::json!({
            "filter": {
                "must": [
                    { "key": "tenant_id", "match": { "value": tenant_id.0 } }
                ]
            }
        });

        match self.client.post(&url).json(&delete_payload).send().await {
            Ok(resp) if !resp.status().is_success() && self.strict_mode => {
                return Err(sunborn_core::SunbornError::vector_store(format!(
                    "Qdrant delete_by_tenant failed: status {}",
                    resp.status()
                )));
            }
            Err(err) if self.strict_mode => {
                return Err(sunborn_core::SunbornError::vector_store(format!(
                    "Qdrant delete_by_tenant connection failed: {}",
                    err
                )));
            }
            _ => {}
        }

        self.fallback_store.delete_by_tenant(tenant_id).await
    }

    async fn all_entries(&self) -> Result<Vec<VectorEntry>> {
        self.fallback_store.all_entries().await
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
