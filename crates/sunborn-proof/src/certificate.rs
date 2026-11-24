use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sunborn_core::error::{Result, SunbornError};
use sunborn_core::model::{ApplicationId, TenantId};
use uuid::Uuid;

/// Tamper-evident, machine-verifiable Proof-Carrying Semantic Reuse (PCSR) certificate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofCertificate {
    pub certificate_id: Uuid,
    pub certificate_version: u32,
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub model_fingerprint: String,
    pub prompt_digest: String,
    pub context_digest: String,
    pub dependency_root: String,
    pub policy_version: String,
    pub freshness_evidence: String,
    pub execution_digest: String,
    pub decision_evidence: String,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub merkle_root: String,
    pub signature: String,
}

impl ProofCertificate {
    pub const CURRENT_VERSION: u32 = 1;

    /// Computes canonical length-prefixed Merkle root over all PCSR fields with domain separation.
    pub fn compute_merkle_root(
        certificate_id: &Uuid,
        certificate_version: u32,
        tenant_id: &TenantId,
        application_id: &ApplicationId,
        model_fingerprint: &str,
        prompt_digest: &str,
        context_digest: &str,
        dependency_root: &str,
        policy_version: &str,
        freshness_evidence: &str,
        execution_digest: &str,
        decision_evidence: &str,
        issued_at: &DateTime<Utc>,
        expires_at: &DateTime<Utc>,
    ) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"SUNBORN_PCSR_V1_MERKLE_DOMAIN\0");

        let issued_str = issued_at.to_rfc3339();
        let expires_str = expires_at.to_rfc3339();

        let fields: [&[u8]; 14] = [
            certificate_id.as_bytes(),
            &certificate_version.to_le_bytes(),
            tenant_id.0.as_bytes(),
            application_id.0.as_bytes(),
            model_fingerprint.as_bytes(),
            prompt_digest.as_bytes(),
            context_digest.as_bytes(),
            dependency_root.as_bytes(),
            policy_version.as_bytes(),
            freshness_evidence.as_bytes(),
            execution_digest.as_bytes(),
            decision_evidence.as_bytes(),
            issued_str.as_bytes(),
            expires_str.as_bytes(),
        ];

        for field in fields {
            let len = field.len() as u32;
            hasher.update(len.to_le_bytes());
            hasher.update(field);
        }

        hex::encode(hasher.finalize())
    }

    /// Re-evaluates Merkle root against certificate payload.
    pub fn recompute_merkle_root(&self) -> String {
        Self::compute_merkle_root(
            &self.certificate_id,
            self.certificate_version,
            &self.tenant_id,
            &self.application_id,
            &self.model_fingerprint,
            &self.prompt_digest,
            &self.context_digest,
            &self.dependency_root,
            &self.policy_version,
            &self.freshness_evidence,
            &self.execution_digest,
            &self.decision_evidence,
            &self.issued_at,
            &self.expires_at,
        )
    }

    /// Verifies the Ed25519 cryptographic signature.
    pub fn verify_signature(&self, public_key: &VerifyingKey) -> bool {
        let sig_bytes = match hex::decode(&self.signature) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        let sig = match Signature::from_slice(&sig_bytes) {
            Ok(sig) => sig,
            Err(_) => return false,
        };

        let merkle_bytes = match hex::decode(&self.merkle_root) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };

        public_key.verify(&merkle_bytes, &sig).is_ok()
    }
}

/// Fluent builder for PCSR Certificates.
pub struct ProofCertificateBuilder {
    certificate_id: Uuid,
    certificate_version: u32,
    tenant_id: TenantId,
    application_id: ApplicationId,
    model_fingerprint: String,
    prompt_digest: String,
    context_digest: String,
    dependency_root: String,
    policy_version: String,
    freshness_evidence: String,
    execution_digest: String,
    decision_evidence: String,
    issued_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
}

impl ProofCertificateBuilder {
    pub fn new(
        tenant_id: TenantId,
        application_id: ApplicationId,
        model_fingerprint: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            certificate_id: Uuid::new_v4(),
            certificate_version: ProofCertificate::CURRENT_VERSION,
            tenant_id,
            application_id,
            model_fingerprint: model_fingerprint.into(),
            prompt_digest: String::new(),
            context_digest: String::new(),
            dependency_root: String::new(),
            policy_version: "v1.0.0".into(),
            freshness_evidence: String::new(),
            execution_digest: String::new(),
            decision_evidence: String::new(),
            issued_at: now,
            expires_at: now + chrono::Duration::hours(24),
        }
    }

    pub fn with_prompt_digest(mut self, digest: impl Into<String>) -> Self {
        self.prompt_digest = digest.into();
        self
    }

    pub fn with_context_digest(mut self, digest: impl Into<String>) -> Self {
        self.context_digest = digest.into();
        self
    }

    pub fn with_dependency_root(mut self, root: impl Into<String>) -> Self {
        self.dependency_root = root.into();
        self
    }

    pub fn with_policy_version(mut self, version: impl Into<String>) -> Self {
        self.policy_version = version.into();
        self
    }

    pub fn with_freshness_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.freshness_evidence = evidence.into();
        self
    }

    pub fn with_execution_digest(mut self, digest: impl Into<String>) -> Self {
        self.execution_digest = digest.into();
        self
    }

    pub fn with_decision_evidence(mut self, evidence: impl Into<String>) -> Self {
        self.decision_evidence = evidence.into();
        self
    }

    pub fn with_validity(mut self, issued_at: DateTime<Utc>, expires_at: DateTime<Utc>) -> Self {
        self.issued_at = issued_at;
        self.expires_at = expires_at;
        self
    }

    /// Signs the computed Merkle root with an Ed25519 key, producing a verified ProofCertificate.
    pub fn sign(self, signing_key: &SigningKey) -> Result<ProofCertificate> {
        let merkle_root = ProofCertificate::compute_merkle_root(
            &self.certificate_id,
            self.certificate_version,
            &self.tenant_id,
            &self.application_id,
            &self.model_fingerprint,
            &self.prompt_digest,
            &self.context_digest,
            &self.dependency_root,
            &self.policy_version,
            &self.freshness_evidence,
            &self.execution_digest,
            &self.decision_evidence,
            &self.issued_at,
            &self.expires_at,
        );

        let merkle_bytes = hex::decode(&merkle_root).map_err(|e| {
            SunbornError::validation_error(format!("Failed to decode merkle root hex: {e}"))
        })?;

        let signature = signing_key.sign(&merkle_bytes);
        let signature_hex = hex::encode(signature.to_bytes());

        Ok(ProofCertificate {
            certificate_id: self.certificate_id,
            certificate_version: self.certificate_version,
            tenant_id: self.tenant_id,
            application_id: self.application_id,
            model_fingerprint: self.model_fingerprint,
            prompt_digest: self.prompt_digest,
            context_digest: self.context_digest,
            dependency_root: self.dependency_root,
            policy_version: self.policy_version,
            freshness_evidence: self.freshness_evidence,
            execution_digest: self.execution_digest,
            decision_evidence: self.decision_evidence,
            issued_at: self.issued_at,
            expires_at: self.expires_at,
            merkle_root,
            signature: signature_hex,
        })
    }
}
