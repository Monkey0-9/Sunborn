use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sunborn_core::model::TenantId;

use crate::anomaly::EmbeddingAnomalyDetector;
use crate::quarantine::{QuarantineEntry, QuarantinePool};

/// Result of evaluating an entry for cache admission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdmissionCheckResult {
    pub admitted: bool,
    pub flagged_for_quarantine: bool,
    pub threat_category: Option<String>,
    pub anomaly_score: f32,
    pub reason: String,
}

impl AdmissionCheckResult {
    pub fn is_flagged_for_quarantine(&self) -> bool {
        self.flagged_for_quarantine
    }

    pub fn threat_category(&self) -> &str {
        self.threat_category.as_deref().unwrap_or("NONE")
    }

    pub fn admitted_safe() -> Self {
        Self {
            admitted: true,
            flagged_for_quarantine: false,
            threat_category: None,
            anomaly_score: 0.0,
            reason: "Passed all threat checks".into(),
        }
    }

    pub fn quarantined(
        category: impl Into<String>,
        anomaly_score: f32,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            admitted: false,
            flagged_for_quarantine: true,
            threat_category: Some(category.into()),
            anomaly_score,
            reason: reason.into(),
        }
    }
}

/// Detector scanning for adversarial poisoning patterns and prompt injection payloads.
#[derive(Clone)]
pub struct PoisoningDetector {
    injection_regex: Regex,
}

impl Default for PoisoningDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl PoisoningDetector {
    pub fn new() -> Self {
        let pattern = r"(?i)(ignore\s+(all\s+)?previous\s+instructions|system\s+override|jailbreak|pwned|dan\s+mode|disregard\s+prior)";
        let injection_regex = Regex::new(pattern).expect("valid poisoning regex");
        Self { injection_regex }
    }

    pub fn contains_injection_payload(&self, text: &str) -> bool {
        // Hardened: Bound regex evaluation against Slowloris / ReDoS exhaustion on oversized inputs
        let eval_slice = if text.len() > 65536 {
            let mut end = 65536;
            while end > 0 && !text.is_char_boundary(end) {
                end -= 1;
            }
            &text[..end]
        } else {
            text
        };
        self.injection_regex.is_match(eval_slice)
    }
}

/// Core adversarial cache threat intelligence engine.
#[derive(Clone)]
pub struct CacheThreatEngine {
    detector: PoisoningDetector,
    anomaly_detector: EmbeddingAnomalyDetector,
    quarantine_pool: QuarantinePool,
}

impl Default for CacheThreatEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl CacheThreatEngine {
    pub fn new() -> Self {
        Self {
            detector: PoisoningDetector::new(),
            anomaly_detector: EmbeddingAnomalyDetector::new(),
            quarantine_pool: QuarantinePool::new(),
        }
    }

    pub fn quarantine_pool(&self) -> &QuarantinePool {
        &self.quarantine_pool
    }

    /// Evaluates candidate pair for adversarial poisoning before cache admission or reuse.
    pub fn inspect_admission(
        &self,
        original_query: &str,
        candidate_query: &str,
        similarity: f32,
    ) -> AdmissionCheckResult {
        // Step 1: Detect prompt injection or jailbreak payload in candidate
        if self.detector.contains_injection_payload(candidate_query)
            && !self.detector.contains_injection_payload(original_query)
        {
            let result = AdmissionCheckResult::quarantined(
                "PROMPT_INJECTION_DISCREPANCY",
                0.95,
                "Candidate contains prompt injection instructions absent from reference query",
            );

            self.quarantine_pool.quarantine(QuarantineEntry {
                key: uuid::Uuid::new_v4().to_string(),
                original_query: original_query.into(),
                attack_query: candidate_query.into(),
                threat_category: "PROMPT_INJECTION_DISCREPANCY".into(),
                anomaly_score: 0.95,
                quarantined_at: Utc::now(),
            });

            return result;
        }

        // Step 2: Detect embedding length anomaly
        let anomaly_score = self.anomaly_detector.score_discrepancy(
            original_query.len(),
            candidate_query.len(),
            similarity,
        );

        if anomaly_score > 0.6 {
            let result = AdmissionCheckResult::quarantined(
                "EMBEDDING_COLLISION_ANOMALY",
                anomaly_score,
                "High similarity despite severe length and semantic token disparity",
            );

            self.quarantine_pool.quarantine(QuarantineEntry {
                key: uuid::Uuid::new_v4().to_string(),
                original_query: original_query.into(),
                attack_query: candidate_query.into(),
                threat_category: "EMBEDDING_COLLISION_ANOMALY".into(),
                anomaly_score,
                quarantined_at: Utc::now(),
            });

            return result;
        }

        AdmissionCheckResult::admitted_safe()
    }

    /// Write-side poisoning defense: blocks adversarial/injected prompts or poisoned
    /// responses from entering L1/L2 or vector caches on cache-miss writeback.
    pub fn inspect_write_admission(
        &self,
        cache_key: &str,
        prompt: &str,
        response: &str,
    ) -> AdmissionCheckResult {
        if self.detector.contains_injection_payload(prompt)
            || self.detector.contains_injection_payload(response)
        {
            let result = AdmissionCheckResult::quarantined(
                "WRITE_SIDE_POISONING_BLOCKED",
                0.99,
                "Adversarial prompt or response injection payload blocked from cache writeback",
            );
            self.quarantine_pool.quarantine(QuarantineEntry {
                key: cache_key.to_string(),
                original_query: prompt.into(),
                attack_query: response.into(),
                threat_category: "WRITE_SIDE_POISONING_BLOCKED".into(),
                anomaly_score: 0.99,
                quarantined_at: Utc::now(),
            });
            return result;
        }
        AdmissionCheckResult::admitted_safe()
    }

    /// Enforces strict tenant boundary invariant: zero cross-tenant cache access.
    pub fn verify_tenant_boundary(&self, req_tenant: &TenantId, entry_tenant: &TenantId) -> bool {
        req_tenant == entry_tenant
    }
}
