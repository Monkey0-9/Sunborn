use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::error::{Result, SunbornError};
use crate::model::{ApplicationId, TenantId};

type HmacSha256 = Hmac<Sha256>;

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

/// Request context required to authorize reuse of a cached result
#[derive(Debug, Clone)]
pub struct ReuseValidationContext<'a> {
    pub tenant_id: &'a TenantId,
    pub application_id: &'a ApplicationId,
    pub model_name: &'a str,
    pub expected_identity_hash: Option<&'a str>,
    pub current_time: DateTime<Utc>,
    pub current_knowledge_epoch: u64,
    pub current_source_epochs: &'a [(Uuid, u64)],
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

    /// Computes canonical length-prefixed Merkle root over all certificate fields with domain separation
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
        // Leaf 1: Tenant & Identity Scope (with domain separation & explicit length prefixes)
        let mut h1 = Sha256::new();
        h1.update(b"sunborn:cert:v1:leaf1:identity:");
        h1.update(certificate_id.as_bytes());
        h1.update(certificate_version.to_be_bytes());
        h1.update((tenant_id.0.len() as u32).to_be_bytes());
        h1.update(tenant_id.0.as_bytes());
        h1.update((application_id.0.len() as u32).to_be_bytes());
        h1.update(application_id.0.as_bytes());
        h1.update((request_identity_hash.len() as u32).to_be_bytes());
        h1.update(request_identity_hash.as_bytes());
        let leaf1 = h1.finalize();

        // Leaf 2: Model & Execution Spec
        let mut h2 = Sha256::new();
        h2.update(b"sunborn:cert:v1:leaf2:model:");
        h2.update((model_name.len() as u32).to_be_bytes());
        h2.update(model_name.as_bytes());
        h2.update(prompt_tokens.to_be_bytes());
        let leaf2 = h2.finalize();

        // Leaf 3: Knowledge Sources Snapshot
        let mut h3 = Sha256::new();
        h3.update(b"sunborn:cert:v1:leaf3:sources:");
        h3.update((source_snapshots.len() as u32).to_be_bytes());
        for s in source_snapshots {
            h3.update(s.source_id.as_bytes());
            h3.update(s.snapshot_epoch.to_be_bytes());
            h3.update((s.content_hash.len() as u32).to_be_bytes());
            h3.update(s.content_hash.as_bytes());
        }
        let leaf3 = h3.finalize();

        // Leaf 4: Safety & Risk Bound Evidence
        let mut h4 = Sha256::new();
        h4.update(b"sunborn:cert:v1:leaf4:safety:");
        let flags: u8 = (safety_evidence.structural_hash_match as u8)
            | ((safety_evidence.negation_polarity_match as u8) << 1)
            | ((safety_evidence.numeric_literal_match as u8) << 2)
            | ((safety_evidence.entity_drift_detected as u8) << 3);
        h4.update([flags]);
        h4.update(safety_evidence.non_conformity_score.to_be_bytes());
        h4.update(safety_evidence.calibrated_risk_bound.to_be_bytes());
        h4.update(safety_evidence.heuristic_risk_score.to_be_bytes());
        let leaf4 = h4.finalize();

        // Leaf 5: Validity Window
        let mut h5 = Sha256::new();
        h5.update(b"sunborn:cert:v1:leaf5:validity:");
        h5.update(validity_window.issued_at.timestamp_millis().to_be_bytes());
        h5.update(validity_window.expires_at.timestamp_millis().to_be_bytes());
        h5.update(validity_window.monotonic_epoch_lease.to_be_bytes());
        let leaf5 = h5.finalize();

        // Intermediate Tree Nodes with domain separation
        let mut node12 = Sha256::new();
        node12.update(b"sunborn:cert:v1:node:12:");
        node12.update(leaf1);
        node12.update(leaf2);
        let n12 = node12.finalize();

        let mut node34 = Sha256::new();
        node34.update(b"sunborn:cert:v1:node:34:");
        node34.update(leaf3);
        node34.update(leaf4);
        let n34 = node34.finalize();

        // Root combining intermediate nodes and leaf5
        let mut root = Sha256::new();
        root.update(b"sunborn:cert:v1:root:");
        root.update(n12);
        root.update(n34);
        root.update(leaf5);
        hex::encode(root.finalize())
    }

    /// Computes standard HMAC-SHA-256 signature over the Merkle root
    pub fn compute_signature(merkle_root: &str, signing_key: &[u8]) -> String {
        let mut mac =
            HmacSha256::new_from_slice(signing_key).expect("HMAC can take key of any size");
        mac.update(b"sunborn:cert:v1:signature:");
        mac.update(merkle_root.as_bytes());
        hex::encode(mac.finalize().into_bytes())
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

    /// Verifies cryptographic integrity of Merkle root and HMAC-SHA-256 signature in constant time
    pub fn verify_integrity(&self, signing_key: &[u8]) -> Result<()> {
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

        // Constant-time check for Merkle root
        if !bool::from(self.merkle_root.as_bytes().ct_eq(expected_root.as_bytes())) {
            return Err(SunbornError::validation_error(
                "Certificate Merkle root mismatch: content was altered or corrupted",
            ));
        }

        // Standard HMAC-SHA-256 verification in constant time
        let expected_sig = Self::compute_signature(&self.merkle_root, signing_key);
        if !bool::from(self.signature.as_bytes().ct_eq(expected_sig.as_bytes())) {
            return Err(SunbornError::validation_error(
                "Certificate cryptographic HMAC signature verification failed",
            ));
        }

        Ok(())
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

        if self.source_snapshots.len() != current_source_epochs.len() {
            return false;
        }

        for (source_id, current_epoch) in current_source_epochs {
            match self
                .source_snapshots
                .iter()
                .find(|s| s.source_id == *source_id)
            {
                Some(snap) if snap.snapshot_epoch == *current_epoch => {}
                _ => return false,
            }
        }

        true
    }

    /// Validates all semantic, temporal, and safety invariants authorizing cache reuse
    pub fn validate_for_reuse(&self, req_ctx: &ReuseValidationContext) -> Result<()> {
        // 1. Version gate (reject unknown or deprecated certificate versions)
        if self.certificate_version != Self::CURRENT_VERSION {
            return Err(SunbornError::validation_error(format!(
                "Admissibility failed: unsupported certificate version {} (supported: {})",
                self.certificate_version,
                Self::CURRENT_VERSION
            )));
        }

        // 2. Certificate ID non-nil
        if self.certificate_id.is_nil() {
            return Err(SunbornError::validation_error(
                "Admissibility failed: certificate ID cannot be nil",
            ));
        }

        // 3. Tenant Boundary Invariant (Strict multi-tenant isolation)
        if self.tenant_id != *req_ctx.tenant_id {
            return Err(SunbornError::forbidden(format!(
                "Admissibility failed: tenant mismatch (cert tenant: {}, expected: {})",
                self.tenant_id.0, req_ctx.tenant_id.0
            )));
        }

        // 4. Application Boundary Invariant
        if self.application_id != *req_ctx.application_id {
            return Err(SunbornError::forbidden(format!(
                "Admissibility failed: application mismatch (cert app: {}, expected: {})",
                self.application_id.0, req_ctx.application_id.0
            )));
        }

        // 5. Model Compatibility
        if self.model_name != req_ctx.model_name {
            return Err(SunbornError::validation_error(format!(
                "Admissibility failed: model mismatch (cert model: {}, expected: {})",
                self.model_name, req_ctx.model_name
            )));
        }

        // 6. Identity Hash Match (enforced when exact matching is evaluated)
        if let Some(expected_hash) = req_ctx.expected_identity_hash {
            if self.request_identity_hash != expected_hash {
                return Err(SunbornError::validation_error(
                    "Admissibility failed: request identity canonical hash mismatch",
                ));
            }
        }

        // 7. Validity Window Sanity & Lease Expiration
        if self.validity_window.expires_at <= self.validity_window.issued_at {
            return Err(SunbornError::validation_error(
                "Admissibility failed: malformed validity window (expires_at <= issued_at)",
            ));
        }
        if req_ctx.current_time > self.validity_window.expires_at {
            return Err(SunbornError::validation_error(
                "Admissibility failed: certificate validity lease has expired",
            ));
        }
        if req_ctx.current_time < self.validity_window.issued_at - chrono::Duration::seconds(5) {
            return Err(SunbornError::validation_error(
                "Admissibility failed: certificate issued in the future (clock skew)",
            ));
        }

        // 8. Monotonic Epoch Lease Fence (Prevents stale writeback and read-repair races)
        if req_ctx.current_knowledge_epoch > self.validity_window.monotonic_epoch_lease {
            return Err(SunbornError::validation_error(format!(
                "Admissibility failed: knowledge epoch advanced (current: {}, cert lease: {})",
                req_ctx.current_knowledge_epoch, self.validity_window.monotonic_epoch_lease
            )));
        }

        // 9. Source Version Snapshot Completeness, Freshness, and Inconsistency Detection
        // (a) Reject duplicate snapshots in the certificate
        let mut seen_cert_sources = std::collections::HashSet::new();
        for snap in &self.source_snapshots {
            if !seen_cert_sources.insert(snap.source_id) {
                return Err(SunbornError::validation_error(format!(
                    "Admissibility failed: duplicate source snapshot in certificate for source {}",
                    snap.source_id
                )));
            }
        }

        // (b) Reject duplicate dependencies in request context
        let mut seen_req_sources = std::collections::HashSet::new();
        for (source_id, _) in req_ctx.current_source_epochs {
            if !seen_req_sources.insert(*source_id) {
                return Err(SunbornError::validation_error(format!(
                    "Admissibility failed: duplicate source dependency in request context for source {}",
                    source_id
                )));
            }
        }

        // (c) Dependency completeness: every authoritative source dependency required by current request
        // MUST have a corresponding snapshot in the certificate
        for (source_id, current_epoch) in req_ctx.current_source_epochs {
            match self
                .source_snapshots
                .iter()
                .find(|s| s.source_id == *source_id)
            {
                None => {
                    return Err(SunbornError::validation_error(format!(
                        "Admissibility failed: missing certificate snapshot for required source dependency {}",
                        source_id
                    )));
                }
                Some(snap) => {
                    if snap.snapshot_epoch != *current_epoch {
                        return Err(SunbornError::validation_error(format!(
                            "Admissibility failed: source dependency {} epoch mutated (cert: {}, current: {})",
                            source_id, snap.snapshot_epoch, current_epoch
                        )));
                    }
                }
            }
        }

        // (d) Snapshot consistency: certificate cannot contain orphaned or extra dependencies
        // not present in the authoritative request dependency graph
        for snap in &self.source_snapshots {
            if !req_ctx
                .current_source_epochs
                .iter()
                .any(|(id, _)| *id == snap.source_id)
            {
                return Err(SunbornError::validation_error(format!(
                    "Admissibility failed: certificate contains snapshot for source {} not in current dependency graph",
                    snap.source_id
                )));
            }
        }

        // 10. Safety Evidence Conformance & Mathematical Bounds
        if !self.safety_evidence.non_conformity_score.is_finite()
            || self.safety_evidence.non_conformity_score < 0.0
        {
            return Err(SunbornError::validation_error(
                "Admissibility failed: non-finite or negative non-conformity score",
            ));
        }
        if !self.safety_evidence.calibrated_risk_bound.is_finite()
            || self.safety_evidence.calibrated_risk_bound < 0.0
        {
            return Err(SunbornError::validation_error(
                "Admissibility failed: non-finite or negative calibrated risk bound",
            ));
        }
        if !self.safety_evidence.heuristic_risk_score.is_finite()
            || self.safety_evidence.heuristic_risk_score < 0.0
        {
            return Err(SunbornError::validation_error(
                "Admissibility failed: non-finite or negative heuristic risk score",
            ));
        }

        // Risk threshold strict bound
        if self.safety_evidence.non_conformity_score
            > self.safety_evidence.calibrated_risk_bound + 1e-6
        {
            return Err(SunbornError::validation_error(format!(
                "Admissibility failed: non-conformity score ({}) exceeds calibrated risk bound ({})",
                self.safety_evidence.non_conformity_score, self.safety_evidence.calibrated_risk_bound
            )));
        }

        if !self.safety_evidence.structural_hash_match
            || !self.safety_evidence.negation_polarity_match
            || !self.safety_evidence.numeric_literal_match
            || self.safety_evidence.entity_drift_detected
        {
            return Err(SunbornError::validation_error(
                "Admissibility failed: safety evidence indicates structural violation or entity drift",
            ));
        }

        Ok(())
    }

    /// Evaluates both integrity and semantic authorization for cache reuse
    #[allow(clippy::too_many_arguments)]
    pub fn verify_admissibility(
        &self,
        signing_key: &[u8],
        expected_tenant: &TenantId,
        expected_app: &ApplicationId,
        expected_model: &str,
        expected_identity_hash: Option<&str>,
        current_time: DateTime<Utc>,
        current_knowledge_epoch: u64,
        current_source_epochs: &[(Uuid, u64)],
    ) -> Result<()> {
        self.verify_integrity(signing_key)?;

        let req_ctx = ReuseValidationContext {
            tenant_id: expected_tenant,
            application_id: expected_app,
            model_name: expected_model,
            expected_identity_hash,
            current_time,
            current_knowledge_epoch,
            current_source_epochs,
        };

        self.validate_for_reuse(&req_ctx)
    }

    /// Legacy compatibility alias for verify_integrity
    pub fn verify_certificate_integrity(&self, signing_key: &[u8]) -> Result<bool> {
        self.verify_integrity(signing_key).map(|_| true)
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
                calibrated_risk_bound: 0.10,
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
        assert!(cert.verify_integrity(key).is_ok());

        // 2. Tampered field fails Merkle root verification
        let mut tampered = cert.clone();
        tampered.model_name = "gpt-3.5-turbo".to_string();
        assert!(tampered.verify_integrity(key).is_err());

        // 3. Forged signature fails HMAC verification
        let mut forged_sig = cert.clone();
        forged_sig.signature = "bad-sig-1234".to_string();
        assert!(forged_sig.verify_integrity(key).is_err());

        // 4. Freshness evaluation
        let now = Utc::now();
        assert!(cert.is_fresh_at(now, &[(doc_id, 1)]));
        // Stale epoch (source mutated to epoch=2)
        assert!(!cert.is_fresh_at(now, &[(doc_id, 2)]));
        // Expired window
        let future = Utc::now() + Duration::hours(2);
        assert!(!cert.is_fresh_at(future, &[(doc_id, 1)]));
    }

    #[test]
    fn test_certificate_admissibility_enforcement_and_rejections() {
        let key = b"admissibility-key-32-bytes-long!";
        let tenant = TenantId::new("tenant-safe");
        let app = ApplicationId::new("app-analytics");
        let doc_id = Uuid::new_v4();

        let cert = ReuseCertificate::new_signed(
            tenant.clone(),
            app.clone(),
            "req-identity-hash-abc",
            "claude-3-5-sonnet",
            200,
            vec![SourceVersionSnapshot {
                source_id: doc_id,
                snapshot_epoch: 5,
                content_hash: "hash-doc-v5".to_string(),
            }],
            SafetyEvidenceRecord {
                structural_hash_match: true,
                negation_polarity_match: true,
                numeric_literal_match: true,
                entity_drift_detected: false,
                non_conformity_score: 0.02,
                calibrated_risk_bound: 0.05,
                heuristic_risk_score: 0.0,
            },
            ValidityWindow {
                issued_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(30),
                monotonic_epoch_lease: 5,
            },
            key,
        );

        let now = Utc::now();

        // 1. Valid request matches perfectly
        assert!(cert
            .verify_admissibility(
                key,
                &tenant,
                &app,
                "claude-3-5-sonnet",
                Some("req-identity-hash-abc"),
                now,
                5,
                &[(doc_id, 5)],
            )
            .is_ok());

        // 2. Cross-tenant substitution rejected
        let other_tenant = TenantId::new("tenant-adversary");
        let err = cert
            .verify_admissibility(
                key,
                &other_tenant,
                &app,
                "claude-3-5-sonnet",
                Some("req-identity-hash-abc"),
                now,
                5,
                &[(doc_id, 5)],
            )
            .unwrap_err();
        assert!(err.to_string().contains("tenant mismatch"));

        // 3. Model mismatch rejected
        let err = cert
            .verify_admissibility(
                key,
                &tenant,
                &app,
                "gpt-4o",
                Some("req-identity-hash-abc"),
                now,
                5,
                &[(doc_id, 5)],
            )
            .unwrap_err();
        assert!(err.to_string().contains("model mismatch"));

        // 4. Identity hash mismatch rejected
        let err = cert
            .verify_admissibility(
                key,
                &tenant,
                &app,
                "claude-3-5-sonnet",
                Some("forged-identity-hash"),
                now,
                5,
                &[(doc_id, 5)],
            )
            .unwrap_err();
        assert!(err.to_string().contains("identity canonical hash mismatch"));

        // 5. Expired lease rejected
        let future_time = Utc::now() + Duration::hours(1);
        let err = cert
            .verify_admissibility(
                key,
                &tenant,
                &app,
                "claude-3-5-sonnet",
                Some("req-identity-hash-abc"),
                future_time,
                5,
                &[(doc_id, 5)],
            )
            .unwrap_err();
        assert!(err.to_string().contains("validity lease has expired"));

        // 6. Monotonic epoch advancement fence (epoch 6 > lease 5)
        let err = cert
            .verify_admissibility(
                key,
                &tenant,
                &app,
                "claude-3-5-sonnet",
                Some("req-identity-hash-abc"),
                now,
                6,
                &[(doc_id, 5)],
            )
            .unwrap_err();
        assert!(err.to_string().contains("knowledge epoch advanced"));

        // 7. Source snapshot mutated (source epoch mutated from 5 to 6)
        let err = cert
            .verify_admissibility(
                key,
                &tenant,
                &app,
                "claude-3-5-sonnet",
                Some("req-identity-hash-abc"),
                now,
                5,
                &[(doc_id, 6)],
            )
            .unwrap_err();
        assert!(err.to_string().contains("epoch mutated"));

        // 8. Key rotation / forged signature rejected
        let bad_key = b"wrong-key-32-bytes-long-bad!!!!";
        let err = cert
            .verify_admissibility(
                bad_key,
                &tenant,
                &app,
                "claude-3-5-sonnet",
                Some("req-identity-hash-abc"),
                now,
                5,
                &[(doc_id, 5)],
            )
            .unwrap_err();
        assert!(err.to_string().contains("signature verification failed"));
    }

    #[test]
    fn test_certificate_rejects_unknown_version_and_non_finite_scores() {
        let key = b"key-validation-32-bytes-long!!!!";
        let tenant = TenantId::new("tenant-strict");
        let app = ApplicationId::new("app-strict");

        let mut cert = ReuseCertificate::new_signed(
            tenant.clone(),
            app.clone(),
            "req-hash-strict",
            "gpt-4o",
            100,
            vec![],
            SafetyEvidenceRecord {
                structural_hash_match: true,
                negation_polarity_match: true,
                numeric_literal_match: true,
                entity_drift_detected: false,
                non_conformity_score: 0.01,
                calibrated_risk_bound: 0.05,
                heuristic_risk_score: 0.0,
            },
            ValidityWindow {
                issued_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(10),
                monotonic_epoch_lease: 1,
            },
            key,
        );

        let ctx = ReuseValidationContext {
            tenant_id: &tenant,
            application_id: &app,
            model_name: "gpt-4o",
            expected_identity_hash: Some("req-hash-strict"),
            current_time: Utc::now(),
            current_knowledge_epoch: 1,
            current_source_epochs: &[],
        };

        // Normal succeeds
        assert!(cert.validate_for_reuse(&ctx).is_ok());

        // Reject unknown version
        cert.certificate_version = 99;
        assert!(cert
            .validate_for_reuse(&ctx)
            .unwrap_err()
            .to_string()
            .contains("unsupported certificate version"));
        cert.certificate_version = ReuseCertificate::CURRENT_VERSION;

        // Reject non-finite score (NaN)
        cert.safety_evidence.non_conformity_score = f32::NAN;
        assert!(cert
            .validate_for_reuse(&ctx)
            .unwrap_err()
            .to_string()
            .contains("non-finite"));
        cert.safety_evidence.non_conformity_score = 0.01;

        // Reject score exceeding calibrated risk bound
        cert.safety_evidence.non_conformity_score = 0.50;
        cert.safety_evidence.calibrated_risk_bound = 0.10;
        assert!(cert
            .validate_for_reuse(&ctx)
            .unwrap_err()
            .to_string()
            .contains("exceeds calibrated risk bound"));
    }

    #[test]
    fn test_certificate_source_dependency_completeness_and_rejections() {
        let key = b"key-source-deps-32-bytes-long!!!";
        let tenant = TenantId::new("tenant-deps");
        let app = ApplicationId::new("app-deps");
        let doc_1 = Uuid::new_v4();
        let doc_2 = Uuid::new_v4();

        let cert = ReuseCertificate::new_signed(
            tenant.clone(),
            app.clone(),
            "req-hash-deps",
            "gpt-4o",
            100,
            vec![SourceVersionSnapshot {
                source_id: doc_1,
                snapshot_epoch: 1,
                content_hash: "hash-doc-1".to_string(),
            }],
            SafetyEvidenceRecord {
                structural_hash_match: true,
                negation_polarity_match: true,
                numeric_literal_match: true,
                entity_drift_detected: false,
                non_conformity_score: 0.01,
                calibrated_risk_bound: 0.05,
                heuristic_risk_score: 0.0,
            },
            ValidityWindow {
                issued_at: Utc::now(),
                expires_at: Utc::now() + Duration::minutes(10),
                monotonic_epoch_lease: 1,
            },
            key,
        );

        let now = Utc::now();

        // 1. Success when dependencies match exactly
        let valid_ctx = ReuseValidationContext {
            tenant_id: &tenant,
            application_id: &app,
            model_name: "gpt-4o",
            expected_identity_hash: Some("req-hash-deps"),
            current_time: now,
            current_knowledge_epoch: 1,
            current_source_epochs: &[(doc_1, 1)],
        };
        assert!(cert.validate_for_reuse(&valid_ctx).is_ok());

        // 2. Reject when request requires doc_2 but certificate omitted doc_2
        let missing_dep_ctx = ReuseValidationContext {
            tenant_id: &tenant,
            application_id: &app,
            model_name: "gpt-4o",
            expected_identity_hash: Some("req-hash-deps"),
            current_time: now,
            current_knowledge_epoch: 1,
            current_source_epochs: &[(doc_1, 1), (doc_2, 1)],
        };
        let err = cert.validate_for_reuse(&missing_dep_ctx).unwrap_err();
        assert!(err
            .to_string()
            .contains("missing certificate snapshot for required source dependency"));

        // 3. Reject when certificate contains extra snapshot not in request dependencies
        let empty_dep_ctx = ReuseValidationContext {
            tenant_id: &tenant,
            application_id: &app,
            model_name: "gpt-4o",
            expected_identity_hash: Some("req-hash-deps"),
            current_time: now,
            current_knowledge_epoch: 1,
            current_source_epochs: &[],
        };
        let err = cert.validate_for_reuse(&empty_dep_ctx).unwrap_err();
        assert!(err
            .to_string()
            .contains("certificate contains snapshot for source"));

        // 4. Reject duplicate dependencies in request context
        let duplicate_req_ctx = ReuseValidationContext {
            tenant_id: &tenant,
            application_id: &app,
            model_name: "gpt-4o",
            expected_identity_hash: Some("req-hash-deps"),
            current_time: now,
            current_knowledge_epoch: 1,
            current_source_epochs: &[(doc_1, 1), (doc_1, 1)],
        };
        let err = cert.validate_for_reuse(&duplicate_req_ctx).unwrap_err();
        assert!(err
            .to_string()
            .contains("duplicate source dependency in request context"));

        // 5. Reject duplicate snapshots in certificate
        let mut duplicate_cert = cert.clone();
        duplicate_cert.source_snapshots.push(SourceVersionSnapshot {
            source_id: doc_1,
            snapshot_epoch: 1,
            content_hash: "hash-doc-1".to_string(),
        });
        let err = duplicate_cert.validate_for_reuse(&valid_ctx).unwrap_err();
        assert!(err
            .to_string()
            .contains("duplicate source snapshot in certificate"));
    }
}
