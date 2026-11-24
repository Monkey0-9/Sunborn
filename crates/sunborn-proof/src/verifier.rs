use chrono::Utc;
use ed25519_dalek::VerifyingKey;
use serde::{Deserialize, Serialize};

use crate::certificate::ProofCertificate;

/// Independent outcome of verifying a Proof-Carrying Semantic Reuse certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VerificationOutcome {
    Verified,
    TamperedPayload,
    SignatureMismatch,
    ExpiredEpoch,
    ContextMismatch,
}

/// Zero-trust verifier that validates proof certificates independently from the cache server.
pub struct IndependentVerifier {
    public_key: VerifyingKey,
}

impl IndependentVerifier {
    pub fn new(public_key: VerifyingKey) -> Self {
        Self { public_key }
    }

    /// Evaluates certificate validity, cryptographic authenticity, and tamper integrity.
    pub fn verify(&self, cert: &ProofCertificate) -> VerificationOutcome {
        // Step 1: Detect tampering in any certificate field by recomputing length-prefixed Merkle root
        let recomputed = cert.recompute_merkle_root();
        if recomputed != cert.merkle_root {
            return VerificationOutcome::TamperedPayload;
        }

        // Step 2: Validate Ed25519 signature over Merkle root
        if !cert.verify_signature(&self.public_key) {
            return VerificationOutcome::SignatureMismatch;
        }

        // Step 3: Check temporal expiry window
        if Utc::now() > cert.expires_at {
            return VerificationOutcome::ExpiredEpoch;
        }

        VerificationOutcome::Verified
    }

    /// Verifies the certificate and asserts that it matches the caller's expected context digest.
    pub fn verify_with_context(
        &self,
        cert: &ProofCertificate,
        expected_context: &str,
    ) -> VerificationOutcome {
        let base_outcome = self.verify(cert);
        if base_outcome != VerificationOutcome::Verified {
            return base_outcome;
        }

        if cert.context_digest != expected_context {
            return VerificationOutcome::ContextMismatch;
        }

        VerificationOutcome::Verified
    }
}
