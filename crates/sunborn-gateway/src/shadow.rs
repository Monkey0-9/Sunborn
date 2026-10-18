use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use sunborn_cache::ExactCacheStore;
use sunborn_core::error::SunbornError;
use sunborn_core::model::{ApplicationId, TenantId};
use sunborn_semantic::{
    cosine_similarity, DeterministicEmbeddingProvider, EmbeddingProvider, VectorStore,
};

pub static SHADOW_EVALUATIONS_TOTAL: AtomicU64 = AtomicU64::new(0);
pub static SHADOW_DRIFT_DETECTED_TOTAL: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowEvaluationRequest {
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub cache_key: String,
    pub cached_content: String,
    pub origin_content: String,
    #[serde(default)]
    pub similarity_threshold: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowEvaluationResult {
    pub cache_key: String,
    pub similarity_score: f32,
    pub is_drift_detected: bool,
    pub threshold: f32,
    pub status: String,
}

pub struct ShadowEvaluator {
    embedder: Arc<dyn EmbeddingProvider>,
    calibrator: Arc<std::sync::RwLock<sunborn_semantic::NonExchangeableConformalCalibrator>>,
}

impl ShadowEvaluator {
    pub fn new() -> Self {
        Self {
            embedder: Arc::new(DeterministicEmbeddingProvider::new()),
            calibrator: Arc::new(std::sync::RwLock::new(
                sunborn_semantic::NonExchangeableConformalCalibrator::new(1000, 0.995, 0.005),
            )),
        }
    }

    pub fn with_embedder(embedder: Arc<dyn EmbeddingProvider>) -> Self {
        Self {
            embedder,
            calibrator: Arc::new(std::sync::RwLock::new(
                sunborn_semantic::NonExchangeableConformalCalibrator::new(1000, 0.995, 0.005),
            )),
        }
    }

    pub fn calibrated_threshold(&self) -> f32 {
        self.calibrator.read().unwrap().calibrated_threshold() as f32
    }

    pub fn record_observation(&self, similarity: f64, semantic_loss: f64) {
        self.calibrator
            .write()
            .unwrap()
            .record_shadow_evaluation(similarity, semantic_loss);
    }

    /// Evaluates semantic divergence between cached content and origin execution.
    /// If drift is detected below the threshold, auto-evicts from cache and vector stores.
    pub async fn evaluate_drift<C: ExactCacheStore + ?Sized, V: VectorStore + ?Sized>(
        &self,
        req: &ShadowEvaluationRequest,
        cache_store: &Arc<C>,
        vector_store: &Arc<V>,
    ) -> Result<ShadowEvaluationResult, SunbornError> {
        SHADOW_EVALUATIONS_TOTAL.fetch_add(1, Ordering::Relaxed);
        let threshold = req.similarity_threshold.unwrap_or(0.85);

        let cached_vec = self.embedder.embed(&req.cached_content).await?;
        let origin_vec = self.embedder.embed(&req.origin_content).await?;

        let similarity = cosine_similarity(&cached_vec, &origin_vec);
        let is_drift_detected = similarity < threshold;

        // Feed non-exchangeable conformal calibrator
        let loss = if is_drift_detected { 1.0 } else { 0.0 };
        self.record_observation(similarity as f64, loss);

        let status = if is_drift_detected {
            SHADOW_DRIFT_DETECTED_TOTAL.fetch_add(1, Ordering::Relaxed);
            tracing::warn!(
                tenant = %req.tenant_id.0,
                cache_key = %req.cache_key,
                similarity = %similarity,
                threshold = %threshold,
                "SHADOW_DRIFT_DETECTED: Semantic divergence exceeded threshold. Evicting stale entry."
            );

            // Auto-evict stale cache entry
            let _ = cache_store.delete(&req.cache_key).await;
            let _ = vector_store.delete(&req.cache_key).await;

            "EVICTED_STALE_CACHE".to_string()
        } else {
            tracing::info!(
                tenant = %req.tenant_id.0,
                cache_key = %req.cache_key,
                similarity = %similarity,
                "SHADOW_EVALUATION_PASSED: Cache content verified against origin."
            );
            "VERIFIED_CONSISTENT".to_string()
        };

        Ok(ShadowEvaluationResult {
            cache_key: req.cache_key.clone(),
            similarity_score: similarity,
            is_drift_detected,
            threshold,
            status,
        })
    }
}

impl Default for ShadowEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sunborn_cache::MemoryExactCache;
    use sunborn_semantic::MemoryVectorStore;

    #[tokio::test]
    async fn test_shadow_evaluation_consistent_content() {
        let evaluator = ShadowEvaluator::new();
        let cache = Arc::new(MemoryExactCache::new());
        let vectors = Arc::new(MemoryVectorStore::new());

        let req = ShadowEvaluationRequest {
            tenant_id: TenantId::new("t1"),
            application_id: ApplicationId::new("app1"),
            cache_key: "key1".into(),
            cached_content: "Rust is a fast and memory-safe language.".into(),
            origin_content: "Rust is a fast and memory-safe programming language.".into(),
            similarity_threshold: Some(0.80),
        };

        let res = evaluator
            .evaluate_drift(&req, &cache, &vectors)
            .await
            .unwrap();
        assert!(!res.is_drift_detected);
        assert_eq!(res.status, "VERIFIED_CONSISTENT");
    }

    #[tokio::test]
    async fn test_shadow_evaluation_detects_drift_and_evicts() {
        let evaluator = ShadowEvaluator::new();
        let cache = Arc::new(MemoryExactCache::new());
        let vectors = Arc::new(MemoryVectorStore::new());

        // Populate cache
        cache
            .set(sunborn_cache::CacheEntry::new(
                "stale_key",
                "Quarterly revenue was 10M",
                None,
            ))
            .await
            .unwrap();

        let req = ShadowEvaluationRequest {
            tenant_id: TenantId::new("t1"),
            application_id: ApplicationId::new("app1"),
            cache_key: "stale_key".into(),
            cached_content: "Quarterly revenue was 10M".into(),
            origin_content: "Completely unrelated statement regarding geology and tectonic plates."
                .into(),
            similarity_threshold: Some(0.90),
        };

        let res = evaluator
            .evaluate_drift(&req, &cache, &vectors)
            .await
            .unwrap();
        assert!(res.is_drift_detected);
        assert_eq!(res.status, "EVICTED_STALE_CACHE");

        // Verify that stale entry was evicted
        let entry = cache.get("stale_key").await.unwrap();
        assert!(
            entry.is_none(),
            "Stale entry must be evicted on drift detection"
        );
    }
}
