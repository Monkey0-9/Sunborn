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

        // Collect references and scores without eagerly cloning heavy vectors and metadata
        let mut candidates: Vec<(&VectorEntry, f32)> = entries
            .iter()
            .filter(|e| e.tenant_id == *tenant_id && e.application_id == *application_id)
            .filter_map(|e| {
                let score = cosine_similarity(query_vector, &e.vector);
                if score >= min_score {
                    Some((e, score))
                } else {
                    None
                }
            })
            .collect();

        // Sort descending by score (using total_cmp for IEEE-754 correctness and speed)
        candidates.sort_by(|a, b| b.1.total_cmp(&a.1));
        candidates.truncate(top_k);

        // Only clone the top_k winning entries
        let scored: Vec<ScoredVectorEntry> = candidates
            .into_iter()
            .map(|(e, score)| ScoredVectorEntry {
                entry: e.clone(),
                score,
            })
            .collect();

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

/// Canonical deterministic UUID for Qdrant point from a cache key or record ID (P0-02).
pub fn canonical_qdrant_point_id(id: &str) -> String {
    use sha2::Digest;
    let mut hasher = sha2::Sha256::new();
    hasher.update(id.as_bytes());
    let hash = hex::encode(hasher.finalize());
    format!(
        "{}-{}-{}-{}-{}",
        &hash[0..8],
        &hash[8..12],
        &hash[12..16],
        &hash[16..20],
        &hash[20..32]
    )
}

/// HNSW and storage parameters used when a collection is created.
///
/// Qdrant's defaults are sized for small collections. At the 10^7-vector scale
/// the collection is built with a fixed HNSW graph so query latency does not
/// drift with corpus growth, payload stored on disk so RAM holds only vectors,
/// and optional int8 scalar quantization that cuts the resident vector footprint
/// by ~75% at a small recall cost. Quantization stays opt-in because the recall
/// trade-off has not been measured on this corpus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionTuning {
    pub hnsw_m: u64,
    pub hnsw_ef_construct: u64,
    pub quantization: bool,
}

impl Default for CollectionTuning {
    fn default() -> Self {
        Self {
            hnsw_m: 16,
            hnsw_ef_construct: 128,
            quantization: false,
        }
    }
}

impl CollectionTuning {
    pub fn from_env() -> Self {
        let defaults = Self::default();
        let read_u64 = |key: &str, fallback: u64| {
            std::env::var(key)
                .ok()
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(fallback)
        };
        Self {
            hnsw_m: read_u64("SUNBORN_QDRANT_HNSW_M", defaults.hnsw_m),
            hnsw_ef_construct: read_u64(
                "SUNBORN_QDRANT_HNSW_EF_CONSTRUCT",
                defaults.hnsw_ef_construct,
            ),
            quantization: sunborn_core::config::parse_bool_env(
                "SUNBORN_QDRANT_QUANTIZATION",
                false,
            )
            .unwrap_or(false),
        }
    }

    /// Request body for `PUT /collections/{name}`.
    pub fn payload(&self, dim: usize) -> serde_json::Value {
        let mut payload = serde_json::json!({
            "vectors": {
                "size": dim,
                "distance": "Cosine"
            },
            "hnsw_config": {
                "m": self.hnsw_m,
                "ef_construct": self.hnsw_ef_construct
            },
            "on_disk_payload": true
        });
        if self.quantization {
            payload["quantization_config"] = serde_json::json!({
                "scalar": {
                    "type": "int8",
                    "always_ram": true
                }
            });
        }
        payload
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
    pub tuning: CollectionTuning,
}

impl QdrantVectorStore {
    pub fn new(endpoint_url: impl Into<String>, collection_name: impl Into<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .connect_timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap_or_default();
        let is_prod = sunborn_core::is_production_mode();
        let strict = match sunborn_core::config::parse_bool_env("SUNBORN_VECTOR_STRICT", false) {
            Ok(v) => {
                v || is_prod
                    || std::env::var("SUNBORN_VECTOR_BACKEND")
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
            tuning: CollectionTuning::from_env(),
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
            tuning: CollectionTuning::from_env(),
        }
    }

    /// Overrides the collection tuning, primarily so tests can pin it without
    /// mutating process-wide environment variables.
    pub fn with_tuning(mut self, tuning: CollectionTuning) -> Self {
        self.tuning = tuning;
        self
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
        let payload = self.tuning.payload(dim);
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

        // Deterministic canonical UUID for Qdrant point from entry.id (P0-02)
        let point_uuid = canonical_qdrant_point_id(&entry.id);

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
        let url = format!(
            "{}/collections/{}/points/delete",
            self.endpoint_url.trim_end_matches('/'),
            self.collection_name
        );
        // Qdrant point ids are UUIDs; reuse the exact canonical deterministic UUID derived at
        // insert time so invalidation actually deletes the indexed point (P0-02).
        let point_uuid = canonical_qdrant_point_id(id);
        let delete_payload = serde_json::json!({
            "points": [point_uuid]
        });
        match self.client.post(&url).json(&delete_payload).send().await {
            Ok(resp) if resp.status().is_success() => Ok(true),
            Ok(resp) => {
                if self.strict_mode {
                    Err(sunborn_core::SunbornError::vector_store(format!(
                        "Qdrant point delete failed with status {}",
                        resp.status()
                    )))
                } else {
                    Ok(true)
                }
            }
            Err(err) => {
                if self.strict_mode {
                    Err(sunborn_core::SunbornError::vector_store(format!(
                        "Qdrant point delete connection failed: {err}"
                    )))
                } else {
                    Ok(true)
                }
            }
        }
    }

    async fn count(&self, tenant_id: &TenantId) -> Result<usize> {
        let url = format!(
            "{}/collections/{}/points/count",
            self.endpoint_url.trim_end_matches('/'),
            self.collection_name
        );
        let payload = serde_json::json!({
            "filter": {
                "must": [
                    { "key": "tenant_id", "match": { "value": tenant_id.0 } }
                ]
            },
            "exact": true
        });
        match self.client.post(&url).json(&payload).send().await {
            // A fresh deployment has no collection yet; that means zero points,
            // not an unavailable vector store. Other HTTP failures still fail
            // closed in strict mode.
            Ok(resp) if resp.status().as_u16() == 404 => Ok(0),
            Ok(resp) if resp.status().is_success() => {
                if let Ok(data) = resp.json::<serde_json::Value>().await {
                    if let Some(count) = data
                        .get("result")
                        .and_then(|r| r.get("count"))
                        .and_then(|c| c.as_u64())
                    {
                        return Ok(count as usize);
                    }
                }
                Ok(0)
            }
            Ok(resp) => {
                if self.strict_mode {
                    Err(sunborn_core::SunbornError::vector_store(format!(
                        "Qdrant count failed with status {}",
                        resp.status()
                    )))
                } else {
                    self.fallback_store.count(tenant_id).await
                }
            }
            Err(err) => {
                if self.strict_mode {
                    Err(sunborn_core::SunbornError::vector_store(format!(
                        "Qdrant count connection failed: {err}"
                    )))
                } else {
                    self.fallback_store.count(tenant_id).await
                }
            }
        }
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

    #[test]
    fn test_canonical_qdrant_point_id_determinism_and_format() {
        let key = "sunborn:tenant-alpha:app-1:chat:sha256:abcd1234efgh5678";
        let id1 = canonical_qdrant_point_id(key);
        let id2 = canonical_qdrant_point_id(key);
        assert_eq!(
            id1, id2,
            "Canonical Qdrant point IDs must be strictly deterministic"
        );

        // Verify valid UUID string length (36 chars with 4 hyphens)
        assert_eq!(id1.len(), 36);
        assert_eq!(id1.matches('-').count(), 4);

        // Verify distinct keys produce distinct point IDs
        let id3 = canonical_qdrant_point_id("sunborn:tenant-beta:app-2:chat");
        assert_ne!(id1, id3);
    }
}

#[cfg(test)]
mod collection_tuning_tests {
    use super::*;

    #[test]
    fn payload_sets_hnsw_graph_and_disk_payload() {
        let tuning = CollectionTuning::default();
        let payload = tuning.payload(384);

        assert_eq!(payload["vectors"]["size"], 384);
        assert_eq!(payload["vectors"]["distance"], "Cosine");
        assert_eq!(payload["hnsw_config"]["m"], 16);
        assert_eq!(payload["hnsw_config"]["ef_construct"], 128);
        assert_eq!(
            payload["on_disk_payload"], true,
            "payload must live on disk so RAM holds vectors only"
        );
    }

    #[test]
    fn quantization_is_absent_unless_enabled() {
        let off = CollectionTuning::default();
        assert!(
            off.payload(384).get("quantization_config").is_none(),
            "an unmeasured recall trade-off must not be enabled implicitly"
        );

        let on = CollectionTuning {
            quantization: true,
            ..CollectionTuning::default()
        };
        let payload = on.payload(384);
        assert_eq!(payload["quantization_config"]["scalar"]["type"], "int8");
        assert_eq!(payload["quantization_config"]["scalar"]["always_ram"], true);
    }

    #[test]
    fn tuning_is_overridable_without_touching_the_environment() {
        let tuned =
            QdrantVectorStore::new("http://localhost:6333", "c").with_tuning(CollectionTuning {
                hnsw_m: 32,
                hnsw_ef_construct: 256,
                quantization: true,
            });
        assert_eq!(tuned.tuning.hnsw_m, 32);
        assert_eq!(tuned.tuning.hnsw_ef_construct, 256);
        assert!(tuned.tuning.payload(8)["quantization_config"].is_object());
    }
}
