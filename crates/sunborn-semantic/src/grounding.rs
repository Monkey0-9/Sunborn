use crate::guardrails::StructuralFingerprint;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use sunborn_core::certificate::SourceVersionSnapshot;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GroundingOutcome {
    FullySupported,
    PartiallySupported,
    Unsupported,
    Contradicted,
    SourceUnavailable,
    SourceVersionChanged,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEvidence {
    pub source_id: String,
    pub content_hash: String,
    pub excerpt: String,
    #[serde(default)]
    pub chunk_id: Option<String>,
    #[serde(default)]
    pub version: Option<u64>,
    #[serde(default)]
    pub retrieved_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub acl_scope: Option<String>,
    #[serde(default)]
    pub trust_level: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundedClaim {
    pub claim_text: String,
    pub supporting_source_ids: Vec<String>,
    pub support_score: f32,
    pub contradicted: bool,
    pub outcome: GroundingOutcome,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceBundle {
    pub evidence_id: Uuid,
    pub tenant_id: String,
    pub application_id: String,
    pub sources: Vec<SourceEvidence>,
    pub claims: Vec<GroundedClaim>,
    pub overall_outcome: GroundingOutcome,
    pub grounding_policy_version: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroundingAttestation {
    pub source_hashes: Vec<(String, String)>,
    pub response_numbers: Vec<String>,
    pub source_numbers: Vec<String>,
    pub grounding_coverage: f32,
    pub grounded: bool,
    pub reason: String,
    pub outcome: GroundingOutcome,
    pub claims_checked: usize,
    pub contradictions: usize,
}

pub struct SourceGroundingVerifier;

fn has_prohibitive_modality(tokens: &[String], text_lower: &str) -> bool {
    const NEG_WORDS: &[&str] = &[
        "not",
        "never",
        "no",
        "none",
        "prohibit",
        "prohibited",
        "forbid",
        "forbidden",
        "avoid",
        "exclude",
        "excluded",
        "block",
        "blocked",
        "deny",
        "denied",
        "withhold",
        "unsafe",
    ];
    text_lower.contains("do not")
        || text_lower.contains("must not")
        || tokens.iter().any(|t| NEG_WORDS.contains(&t.as_str()))
}

fn has_affirmative_modality(tokens: &[String]) -> bool {
    const POS_WORDS: &[&str] = &[
        "allow",
        "allowed",
        "enable",
        "enabled",
        "approve",
        "approved",
        "permit",
        "permitted",
        "safe",
        "exempt",
        "include",
        "included",
        "continue",
        "eligible",
    ];
    tokens.iter().any(|t| POS_WORDS.contains(&t.as_str()))
}

fn extract_verifiable_sentences(text: &str) -> Vec<String> {
    text.split(['.', '!', '?'])
        .map(str::trim)
        .filter(|s| s.len() >= 8)
        .map(str::to_string)
        .collect()
}

impl SourceGroundingVerifier {
    /// Verifies that request-declared dependency snapshots match the cached certificate's
    /// bound source snapshots in both epoch and `content_hash`.
    pub fn verify_source_snapshots(
        req_snapshots: &[SourceVersionSnapshot],
        cert_snapshots: &[SourceVersionSnapshot],
    ) -> Result<(), String> {
        for req_snap in req_snapshots {
            let matching = cert_snapshots
                .iter()
                .find(|c| c.source_id == req_snap.source_id);
            let Some(cand_snap) = matching else {
                return Err(format!(
                    "SOURCE_SNAPSHOT_MISSING: request source {} is absent from cached provenance",
                    req_snap.source_id
                ));
            };

            if req_snap.snapshot_epoch > cand_snap.snapshot_epoch {
                return Err(format!(
                    "SOURCE_EPOCH_STALE: request epoch {} exceeds cached epoch {}",
                    req_snap.snapshot_epoch, cand_snap.snapshot_epoch
                ));
            }

            if !req_snap.content_hash.is_empty()
                && !cand_snap.content_hash.is_empty()
                && req_snap.content_hash != cand_snap.content_hash
            {
                return Err(format!(
                    "SOURCE_CONTENT_HASH_MISMATCH: source {} hash changed ({} != {})",
                    req_snap.source_id, req_snap.content_hash, cand_snap.content_hash
                ));
            }
        }
        Ok(())
    }

    /// Backwards-compatible wrapper with optional grounding checks.
    pub fn verify_response_against_sources(
        response_text: &str,
        sources: &[SourceEvidence],
    ) -> GroundingAttestation {
        Self::verify_response_with_policy(response_text, sources, None, false)
    }

    /// Evaluates whether a cached response is factually grounded in authorized source excerpts.
    /// Checks ACL scope, contradiction polarity, numeric grounding, claim coverage, and
    /// fail-closed behavior when `require_grounding == true`.
    pub fn verify_response_with_policy(
        response_text: &str,
        sources: &[SourceEvidence],
        requester_scope: Option<&str>,
        require_grounding: bool,
    ) -> GroundingAttestation {
        let (_, att) = Self::build_and_verify_bundle(
            "default",
            "default",
            response_text,
            sources,
            requester_scope,
            require_grounding,
        );
        att
    }

    /// Produces both a structured `EvidenceBundle` with claim-level attribution and a
    /// `GroundingAttestation` decision record.
    pub fn build_and_verify_bundle(
        tenant_id: &str,
        application_id: &str,
        response_text: &str,
        sources: &[SourceEvidence],
        requester_scope: Option<&str>,
        require_grounding: bool,
    ) -> (EvidenceBundle, GroundingAttestation) {
        let source_hashes: Vec<(String, String)> = sources
            .iter()
            .map(|s| (s.source_id.clone(), s.content_hash.clone()))
            .collect();

        if sources.is_empty() {
            let outcome = if require_grounding {
                GroundingOutcome::SourceUnavailable
            } else {
                GroundingOutcome::FullySupported
            };
            let grounded = !require_grounding;
            let reason = if require_grounding {
                "SOURCE_UNAVAILABLE_REQUIRED_EVIDENCE_MISSING".to_string()
            } else {
                "NO_EXTERNAL_SOURCES_ATTACHED".to_string()
            };
            let bundle = EvidenceBundle {
                evidence_id: Uuid::new_v4(),
                tenant_id: tenant_id.to_string(),
                application_id: application_id.to_string(),
                sources: Vec::new(),
                claims: Vec::new(),
                overall_outcome: outcome,
                grounding_policy_version: "v2-claims".to_string(),
                created_at: Utc::now(),
            };
            let att = GroundingAttestation {
                source_hashes,
                response_numbers: Vec::new(),
                source_numbers: Vec::new(),
                grounding_coverage: if require_grounding { 0.0 } else { 1.0 },
                grounded,
                reason,
                outcome,
                claims_checked: 0,
                contradictions: 0,
            };
            return (bundle, att);
        }

        // Enforce per-source ACL scope authorization (§6.3 step 3)
        let mut authorized_sources = Vec::new();
        for s in sources {
            if let Some(ref required_acl) = s.acl_scope {
                if !required_acl.is_empty() && requester_scope != Some(required_acl.as_str()) {
                    let bundle = EvidenceBundle {
                        evidence_id: Uuid::new_v4(),
                        tenant_id: tenant_id.to_string(),
                        application_id: application_id.to_string(),
                        sources: sources.to_vec(),
                        claims: Vec::new(),
                        overall_outcome: GroundingOutcome::SourceUnavailable,
                        grounding_policy_version: "v2-claims".to_string(),
                        created_at: Utc::now(),
                    };
                    let att = GroundingAttestation {
                        source_hashes,
                        response_numbers: Vec::new(),
                        source_numbers: Vec::new(),
                        grounding_coverage: 0.0,
                        grounded: false,
                        reason: format!(
                            "SOURCE_ACL_UNAUTHORIZED: source '{}' requires scope '{}'",
                            s.source_id, required_acl
                        ),
                        outcome: GroundingOutcome::SourceUnavailable,
                        claims_checked: 0,
                        contradictions: 0,
                    };
                    return (bundle, att);
                }
            }
            authorized_sources.push(s);
        }

        let combined_sources = authorized_sources
            .iter()
            .map(|s| s.excerpt.as_str())
            .collect::<Vec<_>>()
            .join(" ");

        let resp_fp = StructuralFingerprint::extract(response_text);
        let src_fp = StructuralFingerprint::extract(&combined_sources);

        fn norm_num(n: &str) -> &str {
            n.trim_end_matches('.')
        }

        let src_num_set: HashSet<&str> = src_fp
            .sorted_numeric_literals
            .iter()
            .map(|n| norm_num(n))
            .collect();
        let ungrounded_numbers: Vec<String> = resp_fp
            .sorted_numeric_literals
            .iter()
            .filter(|n| !src_num_set.contains(norm_num(n)))
            .cloned()
            .collect();

        let src_tokens: HashSet<&String> = src_fp.content_tokens.iter().collect();
        let total_resp_tokens = resp_fp.content_tokens.len();
        let supported_tokens = resp_fp
            .content_tokens
            .iter()
            .filter(|t| src_tokens.contains(*t))
            .count();

        let coverage = if total_resp_tokens == 0 {
            1.0
        } else {
            supported_tokens as f32 / total_resp_tokens as f32
        };

        // Precompute source sentence fingerprints for claim-level contradiction & support scoring
        let mut source_sentences: Vec<(String, String, StructuralFingerprint, String)> = Vec::new();
        for s in &authorized_sources {
            for sent in extract_verifiable_sentences(&s.excerpt) {
                let fp = StructuralFingerprint::extract(&sent);
                source_sentences.push((s.source_id.clone(), sent.clone(), fp, sent.to_lowercase()));
            }
        }

        let mut claims = Vec::new();
        let mut contradictions = 0usize;
        let mut supported_claims = 0usize;
        for claim_sent in extract_verifiable_sentences(response_text) {
            let c_fp = StructuralFingerprint::extract(&claim_sent);
            let c_lower = claim_sent.to_lowercase();
            let c_tok_set: HashSet<&String> = c_fp.content_tokens.iter().collect();
            let c_nums_bad = c_fp
                .sorted_numeric_literals
                .iter()
                .any(|n| !src_num_set.contains(norm_num(n)));

            let mut best_overlap = 0.0f32;
            let mut supporting_ids = Vec::new();
            let mut contradicted = false;

            let claim_neg = has_prohibitive_modality(&c_fp.content_tokens, &c_lower);
            let claim_pos = has_affirmative_modality(&c_fp.content_tokens);

            for (sid, _ssent, sfp, slower) in &source_sentences {
                let s_tok_set: HashSet<&String> = sfp.content_tokens.iter().collect();
                let shared = c_tok_set.intersection(&s_tok_set).count();
                let denom = c_tok_set.len().max(1) as f32;
                let ratio = shared as f32 / denom;
                if shared >= 2 {
                    let src_neg = has_prohibitive_modality(&sfp.content_tokens, slower);
                    let src_pos = has_affirmative_modality(&sfp.content_tokens);
                    if (claim_neg && src_pos && !src_neg) || (claim_pos && src_neg && !claim_neg) {
                        contradicted = true;
                    }
                    if ratio > best_overlap {
                        best_overlap = ratio;
                    }
                    if !supporting_ids.contains(sid) {
                        supporting_ids.push(sid.clone());
                    }
                }
            }

            if contradicted {
                contradictions += 1;
            }
            let c_outcome = if contradicted {
                GroundingOutcome::Contradicted
            } else if c_nums_bad || (supporting_ids.is_empty() && c_tok_set.len() >= 2) {
                GroundingOutcome::Unsupported
            } else if best_overlap < 0.35 && !c_fp.content_tokens.is_empty() {
                GroundingOutcome::PartiallySupported
            } else {
                supported_claims += 1;
                GroundingOutcome::FullySupported
            };

            claims.push(GroundedClaim {
                claim_text: claim_sent,
                supporting_source_ids: supporting_ids,
                support_score: best_overlap,
                contradicted,
                outcome: c_outcome,
            });
        }

        let has_partial = claims
            .iter()
            .any(|c| c.outcome == GroundingOutcome::PartiallySupported);
        let has_unsupported = claims
            .iter()
            .any(|c| c.outcome == GroundingOutcome::Unsupported);

        let (overall_outcome, grounded, reason) = if contradictions > 0 {
            (
                GroundingOutcome::Contradicted,
                false,
                format!("RESPONSE_CONTRADICTS_SOURCE_EVIDENCE ({contradictions} claim(s))"),
            )
        } else if !ungrounded_numbers.is_empty() {
            (
                GroundingOutcome::Unsupported,
                false,
                format!(
                    "RESPONSE_UNGROUNDED_NUMERIC_CLAIM: {:?}",
                    ungrounded_numbers
                ),
            )
        } else if !src_fp.content_tokens.is_empty() && total_resp_tokens > 0 && coverage < 0.15 {
            (
                GroundingOutcome::Unsupported,
                false,
                format!("RESPONSE_LOW_SOURCE_GROUNDING_COVERAGE: {:.2}", coverage),
            )
        } else if has_unsupported && supported_claims == 0 && !claims.is_empty() {
            (
                GroundingOutcome::Unsupported,
                false,
                "RESPONSE_CLAIMS_UNSUPPORTED".to_string(),
            )
        } else if require_grounding && (has_unsupported || has_partial) {
            (
                GroundingOutcome::PartiallySupported,
                false,
                "RESPONSE_PARTIALLY_SUPPORTED_REQUIRES_REVALIDATION".to_string(),
            )
        } else {
            (
                GroundingOutcome::FullySupported,
                true,
                "RESPONSE_SOURCE_GROUNDED".to_string(),
            )
        };

        let claims_checked = claims.len();
        let bundle = EvidenceBundle {
            evidence_id: Uuid::new_v4(),
            tenant_id: tenant_id.to_string(),
            application_id: application_id.to_string(),
            sources: sources.to_vec(),
            claims,
            overall_outcome,
            grounding_policy_version: "v2-claims".to_string(),
            created_at: Utc::now(),
        };
        let att = GroundingAttestation {
            source_hashes,
            response_numbers: resp_fp.sorted_numeric_literals,
            source_numbers: src_fp.sorted_numeric_literals,
            grounding_coverage: coverage,
            grounded,
            reason,
            outcome: overall_outcome,
            claims_checked,
            contradictions,
        };
        (bundle, att)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_source_snapshot_content_hash_mismatch_rejected() {
        let src_id = Uuid::new_v4();
        let cert_snaps = vec![SourceVersionSnapshot {
            source_id: src_id,
            snapshot_epoch: 5,
            content_hash: "sha256:old-doc-hash".to_string(),
        }];
        let req_snaps = vec![SourceVersionSnapshot {
            source_id: src_id,
            snapshot_epoch: 5,
            content_hash: "sha256:new-doc-hash".to_string(),
        }];

        let err = SourceGroundingVerifier::verify_source_snapshots(&req_snaps, &cert_snaps)
            .expect_err("Content hash mismatch must reject reuse");
        assert!(err.contains("SOURCE_CONTENT_HASH_MISMATCH"));
    }

    #[test]
    fn test_response_grounding_detects_ungrounded_numbers_and_low_coverage() {
        let sources = vec![SourceEvidence {
            source_id: "doc-1".to_string(),
            content_hash: "h1".to_string(),
            excerpt:
                "Division Alpha Q3 revenue reached 42 million dollars with 15% operating margin."
                    .to_string(),
            chunk_id: Some("c1".to_string()),
            version: Some(1),
            retrieved_at: None,
            acl_scope: None,
            trust_level: Some("authoritative".to_string()),
        }];

        let grounded_resp = "Division Alpha Q3 revenue was 42 million dollars at a 15% margin.";
        let ok = SourceGroundingVerifier::verify_response_against_sources(grounded_resp, &sources);
        assert!(ok.grounded, "Grounded response should pass: {:?}", ok);

        let fabricated_num_resp =
            "Division Alpha Q3 revenue was 99 million dollars at a 15% margin.";
        let bad_num =
            SourceGroundingVerifier::verify_response_against_sources(fabricated_num_resp, &sources);
        assert!(!bad_num.grounded);
        assert!(bad_num.reason.contains("UNGROUNDED_NUMERIC_CLAIM"));

        let off_topic_resp =
            "Tectonic plates subduct beneath oceanic trenches over geological eons.";
        let bad_cov =
            SourceGroundingVerifier::verify_response_against_sources(off_topic_resp, &sources);
        assert!(!bad_cov.grounded);
        assert!(bad_cov.reason.contains("LOW_SOURCE_GROUNDING_COVERAGE"));
    }
}
