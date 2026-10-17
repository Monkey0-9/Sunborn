use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::error::{Result, SunbornError};
use crate::model::{ApplicationId, TenantId};

/// Snapshot of an authoritative knowledge source dependency at the time of computation
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceVersionSnapshot {
    pub source_id: Uuid,
    pub snapshot_epoch: u64,
    pub content_hash: String,
}

/// Evidentiary safety record demonstrating which guardrail and risk checks passed
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SafetyEvidenceRecord {
    pub structural_hash_match: bool,
    pub negation_polarity_match: bool,
    pub numeric_literal_match: bool,
    pub entity_drift_detected: bool,
    pub non_conformity_score: f32,
    pub calibrated_risk_bound: f32,
    pub heuristic_risk_score: f32,
}

/// Point-in-time temporal validity window
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidityWindow {
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub monotonic_epoch_lease: u64,
}

/// Tamper-evident, machine-verifiable proof of reusable execution state
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReuseCertificate {
    pub certificate_id: Uuid,
    pub certificate_version: u32,
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub request_identity_hash: String,
    pub model_name: String,
    pub prompt_tokens: u32,
    pub source_snapshots: Vec<SourceVersionSnapshot>,
    pub safety_evidence: SafetyEvidenceRecord,
    pub validity_window: ValidityWindow,
    pub merkle_root: String,
    pub signature: String,
}

impl ReuseCertificate {
    pub const CURRENT_VERSION: u32 = 1;

    /// Computes canonical Merkle root over all certificate fields
    #[allow(clippy::too_many_arguments)]
    pub fn compute_merkle_root(
        certificate_id: &Uuid,
        certificate_version: u32,
        tenant_id: &TenantId,
        application_id: &ApplicationId,
        request_identity_hash: &str,
        model_name: &str,
        prompt_tokens: u32,
        source_snapshots: &[SourceVersionSnapshot],
        safety_evidence: &SafetyEvidenceRecord,
        validity_window: &ValidityWindow,
    ) -> String {
        // Leaf 1: Tenant & Identity Scope
        let mut h1 = Sha256::new();
        h1.update(certificate_id.as_bytes());
        h1.update(certificate_version.to_be_bytes());
        h1.update(tenant_id.0.as_bytes());
        h1.update(application_id.0.as_bytes());
        h1.update(request_identity_hash.as_bytes());
        let leaf1 = h1.finalize();

        // Leaf 2: Model & Execution Spec
        let mut h2 = Sha256::new();
        h2.update(model_name.as_bytes());
        h2.update(prompt_tokens.to_be_bytes());
        let leaf2 = h2.finalize();

        // Leaf 3: Knowledge Sources Snapshot
        let mut h3 = Sha256::new();
        for s in source_snapshots {
            h3.update(s.source_id.as_bytes());
            h3.update(s.snapshot_epoch.to_be_bytes());
            h3.update(s.content_hash.as_bytes());
        }
        let leaf3 = h3.finalize();

        // Leaf 4: Safety & Risk Bound Evidence
        let mut h4 = Sha256::new();
        h4.update([
            safety_evidence.structural_hash_match as u8,
            safety_evidence.negation_polarity_match as u8,
            safety_evidence.numeric_literal_match as u8,
            safety_evidence.entity_drift_detected as u8,
        ]);
        h4.update(safety_evidence.non_conformity_score.to_be_bytes());
        h4.update(safety_evidence.calibrated_risk_bound.to_be_bytes());
        h4.update(safety_evidence.heuristic_risk_score.to_be_bytes());
        let leaf4 = h4.finalize();

        // Leaf 5: Validity Window
        let mut h5 = Sha256::new();
        h5.update(validity_window.issued_at.timestamp_millis().to_be_bytes());
        h5.update(validity_window.expires_at.timestamp_millis().to_be_bytes());
        h5.update(validity_window.monotonic_epoch_lease.to_be_bytes());
        let leaf5 = h5.finalize();

        // Intermediate Tree Nodes
        let mut node12 = Sha256::new();
        node12.update(leaf1);
        node12.update(leaf2);
        let n12 = node12.finalize();

        let mut node34 = Sha256::new();
        node34.update(leaf3);
        node34.update(leaf4);
        let n34 = node34.finalize();

        // Root combining intermediate nodes and leaf5
        let mut root = Sha256::new();
        root.update(n12);
        root.update(n34);
        root.update(leaf5);
        hex::encode(root.finalize())
    }

    /// Creates and signs a new ReuseCertificate
    #[allow(clippy::too_many_arguments)]
    pub fn new_signed(
        tenant_id: TenantId,
        application_id: ApplicationId,
        request_identity_hash: impl Into<String>,
        model_name: impl Into<String>,
        prompt_tokens: u32,
        source_snapshots: Vec<SourceVersionSnapshot>,
        safety_evidence: SafetyEvidenceRecord,
        validity_window: ValidityWindow,
        signing_key: &[u8],
    ) -> Self {
        let certificate_id = Uuid::new_v4();
        let certificate_version = Self::CURRENT_VERSION;
        let req_id_hash = request_identity_hash.into();
        let model = model_name.into();

        let merkle_root = Self::compute_merkle_root(
            &certificate_id,
            certificate_version,
            &tenant_id,
            &application_id,
            &req_id_hash,
            &model,
            prompt_tokens,
            &source_snapshots,
            &safety_evidence,
            &validity_window,
        );

        let signature = Self::compute_signature(&merkle_root, signing_key);

        Self {
            certificate_id,
            certificate_version,
            tenant_id,
            application_id,
            request_identity_hash: req_id_hash,
            model_name: model,
            prompt_tokens,
            source_snapshots,
            safety_evidence,
            validity_window,
            merkle_root,
            signature,
        }
    }

    pub fn compute_signature(merkle_root: &str, signing_key: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(signing_key);
        hasher.update(b"|REUSE_CERT|");
        hasher.update(merkle_root.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Verifies that the certificate Merkle root matches fields and signature is authentic
    pub fn verify_certificate_integrity(&self, signing_key: &[u8]) -> Result<bool> {
        let expected_root = Self::compute_merkle_root(
            &self.certificate_id,
            self.certificate_version,
            &self.tenant_id,
            &self.application_id,
            &self.request_identity_hash,
            &self.model_name,
            self.prompt_tokens,
            &self.source_snapshots,
            &self.safety_evidence,
            &self.validity_window,
        );

        if self.merkle_root != expected_root {
            return Err(SunbornError::validation_error(
                "Certificate Merkle root mismatch: content was altered or corrupted",
            ));
        }

        let expected_sig = Self::compute_signature(&self.merkle_root, signing_key);
        if self.signature != expected_sig {
            return Err(SunbornError::validation_error(
                "Certificate cryptographic signature verification failed",
            ));
        }

        Ok(true)
    }

    /// Evaluates if certificate remains fresh at a given time against authoritative epochs
    pub fn is_fresh_at(
        &self,
        current_time: DateTime<Utc>,
        current_source_epochs: &[(Uuid, u64)],
    ) -> bool {
        if current_time > self.validity_window.expires_at {
            return false;
        }

        for (source_id, current_epoch) in current_source_epochs {
            if let Some(snap) = self
                .source_snapshots
                .iter()
                .find(|s| s.source_id == *source_id)
            {
                if snap.snapshot_epoch != *current_epoch {
                    return false;
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_certificate_creation_integrity_and_tamper_detection() {
        let key = b"test-secret-key-32-bytes-long!!";
        let tenant = TenantId::new("tenant-alpha");
        let app = ApplicationId::new("app-fintech");
        let doc_id = Uuid::new_v4();

        let cert = ReuseCertificate::new_signed(
            tenant.clone(),
            app.clone(),
            "req-identity-hash-1234",
            "gpt-4o",
            120,
            vec![SourceVersionSnapshot {
                source_id: doc_id,
                snapshot_epoch: 1,
                content_hash: "hash-doc-v1".to_string(),
            }],
            SafetyEvidenceRecord {
                structural_hash_match: true,
                negation_polarity_match: true,
                numeric_literal_match: true,
                entity_drift_detected: false,
                non_conformity_score: 0.05,
                calibrated_risk_bound: 0.01,
                heuristic_risk_score: 0.0,
            },
            ValidityWindow {
                issued_at: Utc::now(),
                expires_at: Utc::now() + Duration::hours(1),
                monotonic_epoch_lease: 1,
            },
            key,
        );

        // 1. Untampered certificate verifies
        assert!(cert.verify_certificate_integrity(key).unwrap());

        // 2. Tampered field fails Merkle root verification
        let mut tampered = cert.clone();
        tampered.model_name = "gpt-3.5-turbo".to_string();
        assert!(tampered.verify_certificate_integrity(key).is_err());

        // 3. Forged signature fails signature verification
        let mut forged_sig = cert.clone();
        forged_sig.signature = "bad-sig-1234".to_string();
        assert!(forged_sig.verify_certificate_integrity(key).is_err());

        // 4. Freshness evaluation
        let now = Utc::now();
        assert!(cert.is_fresh_at(now, &[(doc_id, 1)]));
        // Stale epoch (source mutated to epoch=2)
        assert!(!cert.is_fresh_at(now, &[(doc_id, 2)]));
        // Expired window
        let future = Utc::now() + Duration::hours(2);
        assert!(!cert.is_fresh_at(future, &[(doc_id, 1)]));
    }
}
