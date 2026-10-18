use crate::conformal::{StreamingOODDetector, TaskStratifiedConformalCalibrator, TaskStratum};
use crate::embedding::EmbeddingProvider;
use crate::guardrails::StructuralFingerprint;
use crate::vector_store::VectorStore;
use serde::{Deserialize, Serialize};
use sunborn_core::certificate::SafetyEvidenceRecord;
use sunborn_core::{
    CachePath, DecisionAction, DecisionRecord, RequestEnvelope, Result, ReuseCertificate,
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

/// Stage in the 4-stage sequential verification cascade
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CascadeStage {
    Stage1IdentityAuth,
    Stage2StructuralGuardrails,
    Stage3CausalFreshness,
    Stage4ExecutionOrReuse,
}

/// Outcome of the 4-stage verification cascade
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VerificationCascadeResult {
    pub passed_stage: CascadeStage,
    pub is_eligible_for_reuse: bool,
    pub reason_code: String,
    pub non_conformity_score: f32,
    pub calibrated_risk_bound: f32,
    pub safety_evidence: SafetyEvidenceRecord,
}

/// Multi-tier verification cascade engine
pub struct VerificationCascade {
    pub calibrator: TaskStratifiedConformalCalibrator,
    pub ood_detector: StreamingOODDetector,
}

impl Default for VerificationCascade {
    fn default() -> Self {
        Self::new()
    }
}

impl VerificationCascade {
    pub fn new() -> Self {
        Self {
            calibrator: TaskStratifiedConformalCalibrator::new(),
            ood_detector: StreamingOODDetector::default(),
        }
    }

    /// Evaluates candidate through 4 sequential validation gates:
    /// Stage 1: Identity/Auth Invariant (tenant/app isolation)
    /// Stage 2: Structural Guardrails (negation polarity, numeric literals, prompt risk)
    /// Stage 3: Causal Freshness & Monotonic Epoch Fencing
    /// Stage 4: Conformal Risk Calibration & Streaming OOD Drift
    pub async fn verify(
        &self,
        req: &RequestEnvelope,
        candidate_prompt: &str,
        similarity: f32,
        causal_snapshots_valid: bool,
    ) -> VerificationCascadeResult {
        let risk = DecisionEngine::<
            crate::embedding::DeterministicEmbeddingProvider,
            crate::vector_store::MemoryVectorStore,
        >::assess_risk(&req.prompt);
        let stratum = TaskStratum::from_risk_and_prompt(risk, &req.prompt);
        let calibrated_threshold = self.calibrator.get_threshold(stratum).await as f32;
        let non_conformity = self.calibrator.compute_non_conformity(similarity, risk) as f32;

        // Stage 1: Identity & Authentication Gate
        if req.tenant_id.0.is_empty() || req.application_id.0.is_empty() {
            return VerificationCascadeResult {
                passed_stage: CascadeStage::Stage1IdentityAuth,
                is_eligible_for_reuse: false,
                reason_code: "STAGE1_IDENTITY_AUTH_REJECTED".to_string(),
                non_conformity_score: 1.0,
                calibrated_risk_bound: calibrated_threshold,
                safety_evidence: SafetyEvidenceRecord {
                    structural_hash_match: false,
                    negation_polarity_match: false,
                    numeric_literal_match: false,
                    entity_drift_detected: true,
                    non_conformity_score: 1.0,
                    calibrated_risk_bound: calibrated_threshold,
                    heuristic_risk_score: risk,
                },
            };
        }

        // Stage 2: Structural Guardrails
        let query_fp = StructuralFingerprint::extract(&req.prompt);
        let candidate_fp = StructuralFingerprint::extract(candidate_prompt);

        let structural_match = query_fp.matches(&candidate_fp);
        let negation_match = query_fp.negation_polarity_bit == candidate_fp.negation_polarity_bit;
        let numeric_match =
            query_fp.sorted_numeric_literals == candidate_fp.sorted_numeric_literals;

        let evidence = SafetyEvidenceRecord {
            structural_hash_match: structural_match,
            negation_polarity_match: negation_match,
            numeric_literal_match: numeric_match,
            entity_drift_detected: !structural_match,
            non_conformity_score: non_conformity,
            calibrated_risk_bound: calibrated_threshold,
            heuristic_risk_score: risk,
        };

        if !structural_match || risk > 0.25 {
            return VerificationCascadeResult {
                passed_stage: CascadeStage::Stage2StructuralGuardrails,
                is_eligible_for_reuse: false,
                reason_code: if !structural_match {
                    "STAGE2_STRUCTURAL_GUARDRAIL_VIOLATION".to_string()
                } else {
                    "STAGE2_HIGH_RISK_BUDGET_EXCEEDED".to_string()
                },
                non_conformity_score: non_conformity,
                calibrated_risk_bound: calibrated_threshold,
                safety_evidence: evidence,
            };
        }

        // Streaming OOD drift detection
        let is_ood = self.ood_detector.observe_and_check_drift(similarity as f64);
        if is_ood {
            return VerificationCascadeResult {
                passed_stage: CascadeStage::Stage2StructuralGuardrails,
                is_eligible_for_reuse: false,
                reason_code: "STAGE2_STREAMING_OOD_DRIFT_DETECTED".to_string(),
                non_conformity_score: non_conformity,
                calibrated_risk_bound: calibrated_threshold,
                safety_evidence: evidence,
            };
        }

        // Stage 3: Causal Freshness Gate
        if !causal_snapshots_valid {
            return VerificationCascadeResult {
                passed_stage: CascadeStage::Stage3CausalFreshness,
                is_eligible_for_reuse: false,
                reason_code: "STAGE3_CAUSAL_DEPENDENCY_STALE".to_string(),
                non_conformity_score: non_conformity,
                calibrated_risk_bound: calibrated_threshold,
                safety_evidence: evidence,
            };
        }

        // Stage 4: Conformal Acceptance Gate
        if similarity < calibrated_threshold {
            return VerificationCascadeResult {
                passed_stage: CascadeStage::Stage4ExecutionOrReuse,
                is_eligible_for_reuse: false,
                reason_code: "STAGE4_BELOW_CALIBRATED_SIMILARITY_BOUND".to_string(),
                non_conformity_score: non_conformity,
                calibrated_risk_bound: calibrated_threshold,
                safety_evidence: evidence,
            };
        }

        VerificationCascadeResult {
            passed_stage: CascadeStage::Stage4ExecutionOrReuse,
            is_eligible_for_reuse: true,
            reason_code: "STAGE4_VERIFICATION_CASCADE_PASSED".to_string(),
            non_conformity_score: non_conformity,
            calibrated_risk_bound: calibrated_threshold,
            safety_evidence: evidence,
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecisionOutcome {
    pub decision: DecisionRecord,
    pub matched_content: Option<String>,
    pub matched_id: Option<String>,
    pub matched_certificate: Option<ReuseCertificate>,
    pub safety_evidence: Option<SafetyEvidenceRecord>,
}

pub struct DecisionEngine<E: EmbeddingProvider, V: VectorStore> {
    embedder: E,
    vector_store: V,
    config: DecisionConfig,
    cascade: VerificationCascade,
}

impl<E: EmbeddingProvider, V: VectorStore> DecisionEngine<E, V> {
    pub fn new(embedder: E, vector_store: V, config: DecisionConfig) -> Self {
        Self {
            embedder,
            vector_store,
            config,
            cascade: VerificationCascade::new(),
        }
    }

    /// Evaluates semantic risk for prompt (e.g. presence of mutating commands, PII, financial terms, or security overrides)
    pub fn assess_risk(prompt: &str) -> f32 {
        let lower = prompt.to_lowercase();
        let high_risk_keywords = [
            "delete",
            "drop table",
            "drop database",
            "truncate",
            "password",
            "api_key",
            "secret",
            "credit card",
            "ssn",
            "transfer",
            "wire money",
            "authenticate",
            "execute command",
            "bypass",
            "disable",
            "override",
            "exploit",
            "hack",
            "leak",
            "unauthorized",
            "sql injection",
            "extract",
            "master key",
            "private key",
            "encryption key",
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

    pub async fn evaluate_with_thresholds(
        &self,
        req: &RequestEnvelope,
        min_similarity_threshold: f32,
        max_risk_threshold: f32,
    ) -> Result<DecisionOutcome> {
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
                min_similarity_threshold - self.config.revalidation_band,
            )
            .await?;

        let risk_score = Self::assess_risk(&req.prompt);

        if let Some(best) = candidates.first() {
            let similarity = best.score;
            let confidence =
                Self::compute_confidence(similarity, req.prompt.len(), req.temperature);

            let candidate_cert: Option<ReuseCertificate> = best
                .entry
                .metadata
                .get("certificate")
                .and_then(|v| serde_json::from_value(v.clone()).ok());

            // Execute Verification Cascade
            let cascade_res = self
                .cascade
                .verify(req, &best.entry.prompt, similarity, true)
                .await;

            if !cascade_res.is_eligible_for_reuse {
                return Ok(DecisionOutcome {
                    decision: DecisionRecord {
                        action: DecisionAction::Revalidate,
                        cache_path: CachePath::Semantic,
                        confidence,
                        risk_score,
                        reason_code: cascade_res.reason_code,
                        latency_saved_ms: None,
                        policy_version: None,
                        policy_id: None,
                    },
                    matched_content: Some(best.entry.content.clone()),
                    matched_id: Some(best.entry.id.clone()),
                    matched_certificate: candidate_cert,
                    safety_evidence: Some(cascade_res.safety_evidence),
                });
            }

            // High risk -> Revalidate
            if risk_score > max_risk_threshold {
                return Ok(DecisionOutcome {
                    decision: DecisionRecord {
                        action: DecisionAction::Revalidate,
                        cache_path: CachePath::Semantic,
                        confidence,
                        risk_score,
                        reason_code: "HIGH_RISK_REVALIDATE".to_string(),
                        latency_saved_ms: None,
                        policy_version: None,
                        policy_id: None,
                    },
                    matched_content: Some(best.entry.content.clone()),
                    matched_id: Some(best.entry.id.clone()),
                    matched_certificate: candidate_cert,
                    safety_evidence: Some(cascade_res.safety_evidence),
                });
            }

            // High confidence & similarity >= threshold -> SafeHit C2
            if similarity >= min_similarity_threshold {
                return Ok(DecisionOutcome {
                    decision: DecisionRecord {
                        action: DecisionAction::SafeHit,
                        cache_path: CachePath::Semantic,
                        confidence,
                        risk_score,
                        reason_code: "SEMANTIC_SIMILARITY_MATCH".to_string(),
                        latency_saved_ms: Some(180),
                        policy_version: None,
                        policy_id: None,
                    },
                    matched_content: Some(best.entry.content.clone()),
                    matched_id: Some(best.entry.id.clone()),
                    matched_certificate: candidate_cert,
                    safety_evidence: Some(cascade_res.safety_evidence),
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
                    policy_version: None,
                    policy_id: None,
                },
                matched_content: Some(best.entry.content.clone()),
                matched_id: Some(best.entry.id.clone()),
                matched_certificate: candidate_cert,
                safety_evidence: Some(cascade_res.safety_evidence),
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
                    policy_version: None,
                    policy_id: None,
                },
                matched_content: None,
                matched_id: None,
                matched_certificate: None,
                safety_evidence: None,
            })
        }
    }

    pub async fn evaluate(&self, req: &RequestEnvelope) -> Result<DecisionOutcome> {
        self.evaluate_with_thresholds(
            req,
            self.config.min_similarity_threshold,
            self.config.max_risk_threshold,
        )
        .await
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
    async fn test_verification_cascade_stages() {
        let cascade = VerificationCascade::new();

        // 1. Stage 1 rejection: empty tenant
        let req_empty_tenant = RequestEnvelope::new("", "app1", "gpt-4o", "Hello world");
        let res1 = cascade
            .verify(&req_empty_tenant, "Hello world", 0.99, true)
            .await;
        assert_eq!(res1.passed_stage, CascadeStage::Stage1IdentityAuth);
        assert!(!res1.is_eligible_for_reuse);

        // 2. Stage 2 rejection: negation inversion
        let req_neg = RequestEnvelope::new(
            "t1",
            "app1",
            "gpt-4o",
            "The client has a verified loan approval",
        );
        let res2 = cascade
            .verify(
                &req_neg,
                "The client has no verified loan approval",
                0.95,
                true,
            )
            .await;
        assert_eq!(res2.passed_stage, CascadeStage::Stage2StructuralGuardrails);
        assert!(!res2.is_eligible_for_reuse);

        // 3. Stage 3 rejection: stale causal snapshots
        let req_valid = RequestEnvelope::new(
            "t1",
            "app1",
            "gpt-4o",
            "Explain how semantic caching works in Sunborn",
        );
        let res3 = cascade
            .verify(
                &req_valid,
                "Explain how semantic caching works in Sunborn",
                0.95,
                false, // stale snapshot!
            )
            .await;
        assert_eq!(res3.passed_stage, CascadeStage::Stage3CausalFreshness);
        assert!(!res3.is_eligible_for_reuse);

        // 4. Stage 4 success: all gates pass
        let res4 = cascade
            .verify(
                &req_valid,
                "Explain how semantic caching works in Sunborn",
                0.95,
                true,
            )
            .await;
        assert_eq!(res4.passed_stage, CascadeStage::Stage4ExecutionOrReuse);
        assert!(res4.is_eligible_for_reuse);
    }

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
        assert!(outcome1.safety_evidence.is_some());

        // 2. High-risk mutating prompt -> Revalidate
        let req_risk = RequestEnvelope::new(
            "t1",
            "app1",
            "gpt-4o",
            "Please delete table and drop database",
        );
        let outcome_risk = engine.evaluate(&req_risk).await.unwrap();
        assert!(outcome_risk.decision.risk_score > 0.3);
    }
}
