use chrono::{DateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
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

#[inline]
fn canonical_f32_bits(v: f32) -> u32 {
    if v == 0.0 {
        0.0_f32.to_bits()
    } else {
        v.to_bits()
    }
}

#[inline]
fn canonical_f64_bits(v: f64) -> u64 {
    if v == 0.0 {
        0.0_f64.to_bits()
    } else {
        v.to_bits()
    }
}

#[inline]
fn push_len_prefixed(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(&(s.len() as u32).to_be_bytes());
    buf.extend_from_slice(s.as_bytes());
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerifiedOutcome {
    VerifiedCorrect,
    VerifiedFalseHit,
    VerifiedStale,
    GroundTruthMismatch,
    UnverifiedOpinion,
}

impl VerifiedOutcome {
    pub fn as_canonical_str(&self) -> &'static str {
        match self {
            Self::VerifiedCorrect => "VerifiedCorrect",
            Self::VerifiedFalseHit => "VerifiedFalseHit",
            Self::VerifiedStale => "VerifiedStale",
            Self::GroundTruthMismatch => "GroundTruthMismatch",
            Self::UnverifiedOpinion => "UnverifiedOpinion",
        }
    }

    /// True only when the outcome explicitly confirms a correct, fresh cache reuse.
    pub fn is_admissible_hit(&self) -> bool {
        matches!(self, Self::VerifiedCorrect)
    }

    /// True when the outcome indicates an unsafe reuse (false semantic match,
    /// stale data, or ground-truth contradiction).
    pub fn is_unsafe_reuse(&self) -> bool {
        matches!(
            self,
            Self::VerifiedFalseHit | Self::VerifiedStale | Self::GroundTruthMismatch
        )
    }
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
    /// Compute length-prefixed, domain-separated canonical signing payload covering
    /// EVERY authenticated field of the feedback event (zero unsigned metadata).
    #[allow(clippy::too_many_arguments)]
    pub fn signing_payload(
        event_id: &str,
        tenant_id: &TenantId,
        application_id: &ApplicationId,
        cache_key: &str,
        decision_type: &str,
        prompt_hash: &str,
        similarity_score: Option<f32>,
        conformal_risk_score: Option<f32>,
        latency_ms: f64,
        tokens_saved: u64,
        cost_saved_usd: f64,
        outcome: &VerifiedOutcome,
        verification_source: &str,
        timestamp: &DateTime<Utc>,
    ) -> Vec<u8> {
        let mut buf = Vec::with_capacity(256);
        buf.extend_from_slice(b"sunborn:feedback:v2:");
        push_len_prefixed(&mut buf, event_id);
        push_len_prefixed(&mut buf, &tenant_id.0);
        push_len_prefixed(&mut buf, &application_id.0);
        push_len_prefixed(&mut buf, cache_key);
        push_len_prefixed(&mut buf, decision_type);
        push_len_prefixed(&mut buf, prompt_hash);

        match similarity_score {
            Some(s) => {
                buf.push(1);
                buf.extend_from_slice(&canonical_f32_bits(s).to_be_bytes());
            }
            None => buf.push(0),
        }

        match conformal_risk_score {
            Some(r) => {
                buf.push(1);
                buf.extend_from_slice(&canonical_f32_bits(r).to_be_bytes());
            }
            None => buf.push(0),
        }

        buf.extend_from_slice(&canonical_f64_bits(latency_ms).to_be_bytes());
        buf.extend_from_slice(&tokens_saved.to_be_bytes());
        buf.extend_from_slice(&canonical_f64_bits(cost_saved_usd).to_be_bytes());
        push_len_prefixed(&mut buf, outcome.as_canonical_str());
        push_len_prefixed(&mut buf, verification_source);
        let ts_str = timestamp.to_rfc3339_opts(SecondsFormat::Nanos, true);
        push_len_prefixed(&mut buf, &ts_str);
        buf
    }

    /// Compute canonical signing payload for this event instance.
    pub fn canonical_payload(&self) -> Vec<u8> {
        Self::signing_payload(
            &self.event_id,
            &self.tenant_id,
            &self.application_id,
            &self.cache_key,
            &self.decision_type,
            &self.prompt_hash,
            self.similarity_score,
            self.conformal_risk_score,
            self.latency_ms,
            self.tokens_saved,
            self.cost_saved_usd,
            &self.verified_outcome,
            &self.verification_source,
            &self.timestamp,
        )
    }

    /// Sign all authenticated fields of a feedback event with a secret key.
    #[allow(clippy::too_many_arguments)]
    pub fn sign(
        event_id: &str,
        tenant_id: &TenantId,
        application_id: &ApplicationId,
        cache_key: &str,
        decision_type: &str,
        prompt_hash: &str,
        similarity_score: Option<f32>,
        conformal_risk_score: Option<f32>,
        latency_ms: f64,
        tokens_saved: u64,
        cost_saved_usd: f64,
        outcome: &VerifiedOutcome,
        verification_source: &str,
        timestamp: &DateTime<Utc>,
        secret: &[u8],
    ) -> String {
        let payload = Self::signing_payload(
            event_id,
            tenant_id,
            application_id,
            cache_key,
            decision_type,
            prompt_hash,
            similarity_score,
            conformal_risk_score,
            latency_ms,
            tokens_saved,
            cost_saved_usd,
            outcome,
            verification_source,
            timestamp,
        );
        compute_hmac_sha256(secret, &payload)
    }

    /// Sign this event struct in place and return the HMAC signature.
    pub fn sign_event(&mut self, secret: &[u8]) -> String {
        let payload = self.canonical_payload();
        let sig = compute_hmac_sha256(secret, &payload);
        self.hmac_signature = sig.clone();
        sig
    }

    /// Verify HMAC signature in constant time and validate numeric finiteness.
    pub fn verify_signature(&self, secret: &[u8]) -> Result<(), SunbornError> {
        if !self.latency_ms.is_finite()
            || self.latency_ms < 0.0
            || !self.cost_saved_usd.is_finite()
            || self.cost_saved_usd < 0.0
            || self
                .similarity_score
                .is_some_and(|s| !s.is_finite() || !(0.0..=1.0).contains(&s))
            || self
                .conformal_risk_score
                .is_some_and(|r| !r.is_finite() || !(0.0..=1.0).contains(&r))
        {
            return Err(SunbornError::validation_error(
                "Feedback numeric fields contain non-finite or out-of-range values",
            ));
        }

        let expected = compute_hmac_sha256(secret, &self.canonical_payload());
        let expected_bytes = hex::decode(&expected).unwrap_or_default();
        let actual_bytes = hex::decode(self.hmac_signature.trim()).unwrap_or_default();

        if !expected_bytes.is_empty()
            && actual_bytes.len() == expected_bytes.len()
            && bool::from(actual_bytes.ct_eq(&expected_bytes))
        {
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
