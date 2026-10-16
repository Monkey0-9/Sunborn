use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sunborn_core::{ApplicationId, SunbornError, TenantId};

fn compute_hmac_sha256(key: &[u8], data: &[u8]) -> String {
    let mut padded_key = [0u8; 64];
    if key.len() > 64 {
        let mut hasher = Sha256::new();
        hasher.update(key);
        let h = hasher.finalize();
        padded_key[..32].copy_from_slice(&h);
    } else {
        padded_key[..key.len()].copy_from_slice(key);
    }

    let mut o_key_pad = [0x5cu8; 64];
    let mut i_key_pad = [0x36u8; 64];
    for i in 0..64 {
        o_key_pad[i] ^= padded_key[i];
        i_key_pad[i] ^= padded_key[i];
    }

    let mut inner = Sha256::new();
    inner.update(i_key_pad);
    inner.update(data);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(o_key_pad);
    outer.update(inner_hash);
    hex::encode(outer.finalize())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerifiedOutcome {
    VerifiedCorrect,
    VerifiedFalseHit,
    VerifiedStale,
    GroundTruthMismatch,
    UnverifiedOpinion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedFeedbackEvent {
    pub event_id: String,
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub cache_key: String,
    pub decision_type: String,
    pub prompt_hash: String,
    pub similarity_score: Option<f32>,
    pub conformal_risk_score: Option<f32>,
    pub latency_ms: f64,
    pub tokens_saved: u64,
    pub cost_saved_usd: f64,
    pub verified_outcome: VerifiedOutcome,
    pub verification_source: String,
    pub hmac_signature: String,
    pub timestamp: DateTime<Utc>,
}

impl VerifiedFeedbackEvent {
    /// Compute deterministic signing payload
    pub fn signing_payload(
        event_id: &str,
        tenant_id: &TenantId,
        cache_key: &str,
        outcome: &VerifiedOutcome,
        timestamp: &DateTime<Utc>,
    ) -> String {
        format!(
            "{}:{}:{}:{:?}:{}",
            event_id,
            tenant_id.0,
            cache_key,
            outcome,
            timestamp.to_rfc3339()
        )
    }

    /// Sign the feedback event with a secret key
    pub fn sign(
        event_id: &str,
        tenant_id: &TenantId,
        cache_key: &str,
        outcome: &VerifiedOutcome,
        timestamp: &DateTime<Utc>,
        secret: &[u8],
    ) -> String {
        let payload = Self::signing_payload(event_id, tenant_id, cache_key, outcome, timestamp);
        compute_hmac_sha256(secret, payload.as_bytes())
    }

    /// Verify HMAC signature
    pub fn verify_signature(&self, secret: &[u8]) -> Result<(), SunbornError> {
        let expected = Self::sign(
            &self.event_id,
            &self.tenant_id,
            &self.cache_key,
            &self.verified_outcome,
            &self.timestamp,
            secret,
        );
        if self.hmac_signature.eq_ignore_ascii_case(&expected) {
            Ok(())
        } else {
            Err(SunbornError::unauthorized(
                "Feedback HMAC signature verification failed; possible tampering",
            ))
        }
    }

    /// Is this feedback from a verified, objective source?
    pub fn is_trustworthy(&self) -> bool {
        self.verified_outcome != VerifiedOutcome::UnverifiedOpinion
            && (self.verification_source == "origin_execution"
                || self.verification_source == "human_audit"
                || self.verification_source == "shadow_eval")
    }
}
