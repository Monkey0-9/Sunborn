use crate::embedding::EmbeddingProvider;
use crate::vector_store::VectorStore;
use sunborn_core::{
    CachePath, DecisionAction, DecisionRecord, RequestEnvelope, Result,
};

#[derive(Debug, Clone)]
pub struct DecisionConfig {
    pub min_similarity_threshold: f32,
    pub max_risk_threshold: f32,
    pub revalidation_band: f32,
}

impl Default for DecisionConfig {
    fn default() -> Self {
        Self {
            min_similarity_threshold: 0.88,
            max_risk_threshold: 0.25,
            revalidation_band: 0.05,
        }
    }
}

pub struct DecisionOutcome {
    pub decision: DecisionRecord,
    pub matched_content: Option<String>,
    pub matched_id: Option<String>,
}

pub struct DecisionEngine<E: EmbeddingProvider, V: VectorStore> {
    embedder: E,
    vector_store: V,
    config: DecisionConfig,
}

impl<E: EmbeddingProvider, V: VectorStore> DecisionEngine<E, V> {
    pub fn new(embedder: E, vector_store: V, config: DecisionConfig) -> Self {
        Self {
            embedder,
            vector_store,
            config,
        }
    }

    /// Evaluates semantic risk for prompt (e.g. presence of mutating commands, PII, financial terms)
    pub fn assess_risk(prompt: &str) -> f32 {
        let lower = prompt.to_lowercase();
        let high_risk_keywords = [
            "delete", "drop table", "password", "api_key", "secret", "credit card",
            "ssn", "transfer", "wire money", "authenticate", "execute command",
        ];

        let mut risk: f32 = 0.0;
        for kw in high_risk_keywords {
            if lower.contains(kw) {
                risk += 0.35;
            }
        }
        risk.min(1.0)
    }

    /// Calibrates confidence from similarity, prompt length, and temperature
    pub fn compute_confidence(similarity: f32, prompt_len: usize, temperature: Option<f32>) -> f32 {
        let temp_penalty = temperature.unwrap_or(0.7) * 0.05;
        let len_bonus = if prompt_len > 20 { 0.03 } else { 0.0 };
        (similarity + len_bonus - temp_penalty).clamp(0.0, 1.0)
    }

    pub async fn evaluate(&self, req: &RequestEnvelope) -> Result<DecisionOutcome> {
        // 1. Generate query embedding
        let query_vec = self.embedder.embed(&req.prompt).await?;

        // 2. Query vector candidates for the specific tenant and application
        let candidates = self
            .vector_store
            .search(
                &req.tenant_id,
                &req.application_id,
                &query_vec,
                1,
                self.config.min_similarity_threshold - self.config.revalidation_band,
            )
            .await?;

        let risk_score = Self::assess_risk(&req.prompt);

        if let Some(best) = candidates.first() {
            let similarity = best.score;
            let confidence = Self::compute_confidence(similarity, req.prompt.len(), req.temperature);

            // High risk -> Revalidate
            if risk_score > self.config.max_risk_threshold {
                return Ok(DecisionOutcome {
                    decision: DecisionRecord {
                        action: DecisionAction::Revalidate,
                        cache_path: CachePath::Semantic,
                        confidence,
                        risk_score,
                        reason_code: "HIGH_RISK_REVALIDATE".to_string(),
                        latency_saved_ms: None,
                    },
                    matched_content: Some(best.entry.content.clone()),
                    matched_id: Some(best.entry.id.clone()),
                });
            }

            // High confidence & similarity >= threshold -> SafeHit C2
            if similarity >= self.config.min_similarity_threshold {
                return Ok(DecisionOutcome {
                    decision: DecisionRecord {
                        action: DecisionAction::SafeHit,
                        cache_path: CachePath::Semantic,
                        confidence,
                        risk_score,
                        reason_code: "SEMANTIC_SIMILARITY_MATCH".to_string(),
                        latency_saved_ms: Some(180),
                    },
                    matched_content: Some(best.entry.content.clone()),
                    matched_id: Some(best.entry.id.clone()),
                });
            }

            // In revalidation band
            Ok(DecisionOutcome {
                decision: DecisionRecord {
                    action: DecisionAction::Revalidate,
                    cache_path: CachePath::Semantic,
                    confidence,
                    risk_score,
                    reason_code: "BORDERLINE_SIMILARITY_REVALIDATE".to_string(),
                    latency_saved_ms: None,
                },
                matched_content: Some(best.entry.content.clone()),
                matched_id: Some(best.entry.id.clone()),
            })
        } else {
            // Semantic miss -> Execute origin
            Ok(DecisionOutcome {
                decision: DecisionRecord {
                    action: DecisionAction::OriginExecute,
                    cache_path: CachePath::Miss,
                    confidence: 0.0,
                    risk_score,
                    reason_code: "NO_SEMANTIC_CANDIDATE_FOUND".to_string(),
                    latency_saved_ms: None,
                },
                matched_content: None,
                matched_id: None,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding::DeterministicEmbeddingProvider;
    use crate::vector_store::{MemoryVectorStore, VectorEntry};
    use chrono::Utc;
    use sunborn_core::{ApplicationId, TenantId};

    #[tokio::test]
    async fn test_decision_engine_safe_hit_and_risk() {
        let embedder = DeterministicEmbeddingProvider::new();
        let store = MemoryVectorStore::new();

        let prompt = "How does Sunborn achieve low latency?";
        let vec = embedder.embed(prompt).await.unwrap();

        store
            .insert(VectorEntry {
                id: "doc-1".into(),
                tenant_id: TenantId::new("t1"),
                application_id: ApplicationId::new("app1"),
                prompt: prompt.into(),
                content: "Sunborn uses multi-tier caching (C0-C5) and exact hash lookups.".into(),
                vector: vec,
                metadata: serde_json::json!({}),
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        let engine = DecisionEngine::new(embedder, store, DecisionConfig::default());

        // 1. Identical query -> SafeHit
        let req1 = RequestEnvelope::new("t1", "app1", "gpt-4o", prompt);
        let outcome1 = engine.evaluate(&req1).await.unwrap();
        assert_eq!(outcome1.decision.action, DecisionAction::SafeHit);
        assert_eq!(outcome1.decision.cache_path, CachePath::Semantic);
        assert!(outcome1.matched_content.is_some());

        // 2. High-risk mutating prompt -> Revalidate
        let req_risk = RequestEnvelope::new("t1", "app1", "gpt-4o", "Please delete table and drop database");
        let outcome_risk = engine.evaluate(&req_risk).await.unwrap();
        assert!(outcome_risk.decision.risk_score > 0.3);
    }
}
