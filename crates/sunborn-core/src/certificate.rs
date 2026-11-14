use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::error::{Result, SunbornError};
use crate::model::{ApplicationId, TenantId};

type HmacSha256 = Hmac<Sha256>;

/// Cryptographic algorithm used to attest certificate provenance and non-repudiation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignatureAlgorithm {
    #[serde(rename = "ed25519")]
    #[default]
    Ed25519,
    #[serde(rename = "hmac-sha256")]
    HmacSha256,
}

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
    /// Canonical JSON of the issuing prompt's structural fingerprint. Empty for
    /// legacy certificates. When present, reuse is re-verified against the
    /// candidate prompt instead of trusting the issuance-time boolean flags.
    #[serde(default)]
    pub reference_fingerprint: String,
    pub merkle_root: String,
    pub signature: String,
    #[serde(default)]
    pub signature_algorithm: SignatureAlgorithm,
    #[serde(default)]
    pub public_key_id: String,
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
        reference_fingerprint: &str,
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
        h4.update((reference_fingerprint.len() as u32).to_be_bytes());
        h4.update(reference_fingerprint.as_bytes());
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
            "",
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
            reference_fingerprint: String::new(),
            merkle_root,
            signature,
            signature_algorithm: SignatureAlgorithm::HmacSha256,
            public_key_id: String::new(),
        }
    }

    /// Creates and signs a new ReuseCertificate using asymmetric Ed25519
    #[allow(clippy::too_many_arguments)]
    pub fn new_signed_ed25519(
        tenant_id: TenantId,
        application_id: ApplicationId,
        request_identity_hash: impl Into<String>,
        model_name: impl Into<String>,
        prompt_tokens: u32,
        source_snapshots: Vec<SourceVersionSnapshot>,
        safety_evidence: SafetyEvidenceRecord,
        validity_window: ValidityWindow,
        signing_key: &SigningKey,
        key_id: impl Into<String>,
    ) -> Self {
        let certificate_id = Uuid::new_v4();
        let certificate_version = Self::CURRENT_VERSION;
        let req_id_hash = request_identity_hash.into();
        let model = model_name.into();
        let public_key_id = key_id.into();

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
            "",
        );

        let signature = Self::compute_signature_ed25519(&merkle_root, signing_key);

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
            reference_fingerprint: String::new(),
            merkle_root,
            signature,
            signature_algorithm: SignatureAlgorithm::Ed25519,
            public_key_id,
        }
    }

    /// Computes standard Ed25519 signature over the Merkle root with domain separation
    pub fn compute_signature_ed25519(merkle_root: &str, signing_key: &SigningKey) -> String {
        let mut msg = Vec::with_capacity(32 + merkle_root.len());
        msg.extend_from_slice(b"sunborn:cert:v1:ed25519:sig:");
        msg.extend_from_slice(merkle_root.as_bytes());
        let sig: Signature = signing_key.sign(&msg);
        hex::encode(sig.to_bytes())
    }

    /// Binds reference fingerprint and signs with Ed25519
    pub fn attach_reference_fingerprint_ed25519(
        &mut self,
        reference_fingerprint: impl Into<String>,
        signing_key: &SigningKey,
    ) {
        self.reference_fingerprint = reference_fingerprint.into();
        self.merkle_root = Self::compute_merkle_root(
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
            &self.reference_fingerprint,
        );
        self.signature = Self::compute_signature_ed25519(&self.merkle_root, signing_key);
        self.signature_algorithm = SignatureAlgorithm::Ed25519;
    }

    /// Verifies Ed25519 signature over the Merkle root using only a public VerifyingKey
    pub fn verify_signature_ed25519(&self, verifying_key: &VerifyingKey) -> Result<()> {
        let sig_bytes = hex::decode(&self.signature)
            .map_err(|e| SunbornError::validation_error(format!("Invalid hex signature: {e}")))?;
        if sig_bytes.len() != 64 {
            return Err(SunbornError::validation_error(
                "Ed25519 signature must be exactly 64 bytes",
            ));
        }
        let mut arr = [0u8; 64];
        arr.copy_from_slice(&sig_bytes);
        let sig = Signature::from_bytes(&arr);

        let mut msg = Vec::with_capacity(32 + self.merkle_root.len());
        msg.extend_from_slice(b"sunborn:cert:v1:ed25519:sig:");
        msg.extend_from_slice(self.merkle_root.as_bytes());

        verifying_key.verify(&msg, &sig).map_err(|_| {
            SunbornError::validation_error("Certificate Ed25519 public key verification failed")
        })?;
        Ok(())
    }

    /// Binds the canonical structural fingerprint of the issuing prompt to the
    /// certificate and re-signs it. Reuse can then be re-verified against the
    /// candidate prompt at read time.
    pub fn attach_reference_fingerprint(
        &mut self,
        reference_fingerprint: impl Into<String>,
        signing_key: &[u8],
    ) {
        self.reference_fingerprint = reference_fingerprint.into();
        self.merkle_root = Self::compute_merkle_root(
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
            &self.reference_fingerprint,
        );
        if self.signature_algorithm == SignatureAlgorithm::Ed25519 && signing_key.len() == 32 {
            let mut seed = [0u8; 32];
            seed.copy_from_slice(signing_key);
            let sk = SigningKey::from_bytes(&seed);
            self.signature = Self::compute_signature_ed25519(&self.merkle_root, &sk);
        } else {
            self.signature = Self::compute_signature(&self.merkle_root, signing_key);
        }
    }

    /// Verifies cryptographic integrity of Merkle root and signature in constant time
    pub fn verify_integrity(&self, key: &[u8]) -> Result<()> {
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
            &self.reference_fingerprint,
        );

        // Constant-time check for Merkle root
        if !bool::from(self.merkle_root.as_bytes().ct_eq(expected_root.as_bytes())) {
            return Err(SunbornError::validation_error(
                "Certificate Merkle root mismatch: content was altered or corrupted",
            ));
        }

        match self.signature_algorithm {
            SignatureAlgorithm::Ed25519 => {
                if key.len() == 32 {
                    let mut vk_bytes = [0u8; 32];
                    vk_bytes.copy_from_slice(key);
                    if let Ok(verifying_key) = VerifyingKey::from_bytes(&vk_bytes) {
                        if self.verify_signature_ed25519(&verifying_key).is_ok() {
                            return Ok(());
                        }
                    }
                }

                // Also allow verification if caller provided the secret signing key/seed
                let mut hasher = Sha256::new();
                hasher.update(b"sunborn-derived-ed25519-seed-v1:");
                hasher.update(key);
                let derived_seed: [u8; 32] = hasher.finalize().into();
                let sk = SigningKey::from_bytes(&derived_seed);
                let vk = sk.verifying_key();
                if self.verify_signature_ed25519(&vk).is_ok() {
                    return Ok(());
                }

                // Also try direct 32-byte signing key seed
                if key.len() == 32 {
                    let mut seed = [0u8; 32];
                    seed.copy_from_slice(key);
                    let direct_sk = SigningKey::from_bytes(&seed);
                    let direct_vk = direct_sk.verifying_key();
                    if self.verify_signature_ed25519(&direct_vk).is_ok() {
                        return Ok(());
                    }
                }

                Err(SunbornError::validation_error(
                    "Certificate Ed25519 signature verification failed with provided key",
                ))
            }
            SignatureAlgorithm::HmacSha256 => {
                let expected_sig = Self::compute_signature(&self.merkle_root, key);
                if !bool::from(self.signature.as_bytes().ct_eq(expected_sig.as_bytes())) {
                    return Err(SunbornError::validation_error(
                        "Certificate cryptographic HMAC signature verification failed",
                    ));
                }
                Ok(())
            }
        }
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

    /// Verifies the signature and freshness, then applies caller-measured safety
    /// evidence instead of the issuance-time flags. This is the measured path:
    /// the gateway recomputes structural/negation/numeric/entity results against
    /// the candidate prompt and passes them here.
    #[allow(clippy::too_many_arguments)]
    pub fn verify_admissibility_measured(
        &self,
        signing_key: &[u8],
        expected_tenant: &TenantId,
        expected_app: &ApplicationId,
        expected_model: &str,
        expected_identity_hash: Option<&str>,
        current_time: DateTime<Utc>,
        current_knowledge_epoch: u64,
        current_source_epochs: &[(Uuid, u64)],
        measured: &SafetyEvidenceRecord,
    ) -> Result<()> {
        self.verify_integrity(signing_key)?;

        let mut effective = self.clone();
        effective.safety_evidence = measured.clone();

        let req_ctx = ReuseValidationContext {
            tenant_id: expected_tenant,
            application_id: expected_app,
            model_name: expected_model,
            expected_identity_hash,
            current_time,
            current_knowledge_epoch,
            current_source_epochs,
        };

        effective.validate_for_reuse(&req_ctx)
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
    fn test_measured_evidence_is_enforced_over_issuance_flags() {
        let key = b"measured-evidence-key-32-bytes!!";
        let tenant = TenantId::new("tenant-measured");
        let app = ApplicationId::new("app-measured");
        let doc_id = Uuid::new_v4();
        let good = SafetyEvidenceRecord {
            structural_hash_match: true,
            negation_polarity_match: true,
            numeric_literal_match: true,
            entity_drift_detected: false,
            non_conformity_score: 0.0,
            calibrated_risk_bound: 0.25,
            heuristic_risk_score: 0.0,
        };
        let mut cert = ReuseCertificate::new_signed(
            tenant.clone(),
            app.clone(),
            "identity-hash",
            "gpt-4o",
            10,
            vec![SourceVersionSnapshot {
                source_id: doc_id,
                snapshot_epoch: 1,
                content_hash: "h".to_string(),
            }],
            good.clone(),
            ValidityWindow {
                issued_at: Utc::now(),
                expires_at: Utc::now() + Duration::hours(1),
                monotonic_epoch_lease: 1,
            },
            key,
        );
        cert.attach_reference_fingerprint("{\"reference\":true}", key);

        // Measured evidence that passes is accepted.
        assert!(cert
            .verify_admissibility_measured(
                key,
                &tenant,
                &app,
                "gpt-4o",
                None,
                Utc::now(),
                1,
                &[(doc_id, 1)],
                &good
            )
            .is_ok());

        // Even though issuance recorded "true", measured evidence that fails is rejected.
        let bad = SafetyEvidenceRecord {
            structural_hash_match: false,
            ..good
        };
        assert!(cert
            .verify_admissibility_measured(
                key,
                &tenant,
                &app,
                "gpt-4o",
                None,
                Utc::now(),
                1,
                &[(doc_id, 1)],
                &bad
            )
            .is_err());
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

    #[test]
    fn test_ed25519_certificate_signing_and_zero_trust_public_verification() {
        let seed = [42u8; 32];
        let signing_key = SigningKey::from_bytes(&seed);
        let verifying_key = signing_key.verifying_key();

        let tenant = TenantId::new("tenant-ed25519");
        let app = ApplicationId::new("app-ed25519");
        let doc_id = Uuid::new_v4();

        let mut cert = ReuseCertificate::new_signed_ed25519(
            tenant.clone(),
            app.clone(),
            "req-hash-ed25519",
            "claude-3-5-sonnet-20241022",
            250,
            vec![SourceVersionSnapshot {
                source_id: doc_id,
                snapshot_epoch: 1,
                content_hash: "hash-doc-ed25519".to_string(),
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
                expires_at: Utc::now() + Duration::minutes(15),
                monotonic_epoch_lease: 1,
            },
            &signing_key,
            "key-ed25519-v1",
        );

        assert_eq!(cert.signature_algorithm, SignatureAlgorithm::Ed25519);
        assert_eq!(cert.public_key_id, "key-ed25519-v1");

        // 1. Verify with VerifyingKey directly (offline zero-trust verification)
        assert!(cert.verify_signature_ed25519(&verifying_key).is_ok());

        // 2. Verify with verify_integrity using verifying key bytes (32 bytes)
        assert!(cert.verify_integrity(verifying_key.as_bytes()).is_ok());

        // 3. Attach reference fingerprint and re-verify
        cert.attach_reference_fingerprint_ed25519("{\"fingerprint\": \"abc\"}", &signing_key);
        assert!(cert.verify_signature_ed25519(&verifying_key).is_ok());

        // 4. Tamper detection: modifying Merkle root fails verification
        let mut tampered_cert = cert.clone();
        tampered_cert.merkle_root =
            "0000000000000000000000000000000000000000000000000000000000000000".to_string();
        assert!(tampered_cert
            .verify_integrity(verifying_key.as_bytes())
            .is_err());

        // 5. Tamper detection: verifying with wrong public key fails
        let wrong_seed = [99u8; 32];
        let wrong_signing_key = SigningKey::from_bytes(&wrong_seed);
        let wrong_verifying_key = wrong_signing_key.verifying_key();
        assert!(cert.verify_signature_ed25519(&wrong_verifying_key).is_err());
        assert!(cert
            .verify_integrity(wrong_verifying_key.as_bytes())
            .is_err());
    }
}
