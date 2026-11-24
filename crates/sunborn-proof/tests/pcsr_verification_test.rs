use chrono::{Duration, Utc};
use ed25519_dalek::SigningKey;
use rand_core::OsRng;
use sunborn_core::model::{ApplicationId, TenantId};
use sunborn_proof::certificate::ProofCertificateBuilder;
use sunborn_proof::verifier::{IndependentVerifier, VerificationOutcome};

#[test]
fn test_pcsr_certificate_verification_and_tamper_detection() {
    let mut csprng = OsRng;
    let signing_key = SigningKey::generate(&mut csprng);
    let verifying_key = signing_key.verifying_key();

    let cert = ProofCertificateBuilder::new(
        TenantId("tenant_acme".into()),
        ApplicationId("app_rag".into()),
        "gpt-4o",
    )
    .with_context_digest("ctx_digest_abc123")
    .with_dependency_root("dep_root_def456")
    .with_policy_version("v1.2.0")
    .with_execution_digest("exec_digest_789")
    .sign(&signing_key)
    .expect("certificate signing failed");

    let verifier = IndependentVerifier::new(verifying_key);
    assert_eq!(verifier.verify(&cert), VerificationOutcome::Verified);

    // Tamper with context digest -> TamperedPayload
    let mut tampered = cert.clone();
    tampered.context_digest = "ctx_tampered".into();
    assert_eq!(
        verifier.verify(&tampered),
        VerificationOutcome::TamperedPayload
    );

    // Context mismatch check
    assert_eq!(
        verifier.verify_with_context(&cert, "ctx_digest_abc123"),
        VerificationOutcome::Verified
    );
    assert_eq!(
        verifier.verify_with_context(&cert, "different_context"),
        VerificationOutcome::ContextMismatch
    );

    // Signature mismatch (different verifying key)
    let other_key = SigningKey::generate(&mut csprng).verifying_key();
    let other_verifier = IndependentVerifier::new(other_key);
    assert_eq!(
        other_verifier.verify(&cert),
        VerificationOutcome::SignatureMismatch
    );

    // Expired certificate
    let past = Utc::now() - Duration::hours(48);
    let expired_cert = ProofCertificateBuilder::new(
        TenantId("tenant_acme".into()),
        ApplicationId("app_rag".into()),
        "gpt-4o",
    )
    .with_validity(past, past + Duration::hours(1))
    .sign(&signing_key)
    .expect("signing expired cert failed");

    assert_eq!(
        verifier.verify(&expired_cert),
        VerificationOutcome::ExpiredEpoch
    );
}
