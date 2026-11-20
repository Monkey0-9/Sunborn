use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sunborn_core::AppConfig;
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_dpdp_act_consent_recording_and_statutory_erasure() {
    let mut config = AppConfig::default();
    config.server.host = "127.0.0.1".to_string();
    config.server.port = 0;
    config.cache.backend = "memory".to_string();
    config.cache.vector_backend = "memory".to_string();

    let state = AppState::new(config);
    let app = create_router(state);

    // 1. Record Data Principal Consent (DPDP Act 2023 Section 6)
    let consent_payload = json!({
        "tenant_id": "enterprise-fintech",
        "data_principal_id": "aadhaar_ref_encrypted_hash_987654",
        "purpose": "Account Aggregator Consent for Instant MSME Credit Evaluation",
        "retention_period_days": 180,
        "consent_notice_version": "v2.1-meity-dpdp",
    });

    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/dpdp/consent")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(consent_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "recorded");
    assert!(body["principal_hash"].as_str().is_some());
    assert_eq!(body["compliance"], "DPDP Act 2023 Section 6(1) Verified");

    // 2. Data Fiduciary Audit Status
    let audit_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/governance/dpdp/audit")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(audit_res.status(), StatusCode::OK);
    let bytes = audit_res.into_body().collect().await.unwrap().to_bytes();
    let audit_body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(audit_body["status"], "compliant");
    assert_eq!(audit_body["fiduciary_jurisdiction"], "Republic of India");

    // 3. Right to Erasure / Right to be Forgotten (DPDP Act 2023 Section 12(3))
    let erasure_payload = json!({
        "tenant_id": "enterprise-fintech",
        "data_principal_id": "aadhaar_ref_encrypted_hash_987654",
        "reason": "Consent Withdrawn by Data Principal",
    });

    let erasure_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/dpdp/erasure")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(erasure_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(erasure_res.status(), StatusCode::OK);
    let bytes = erasure_res.into_body().collect().await.unwrap().to_bytes();
    let erasure_body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(erasure_body["status"], "erased");
    assert!(erasure_body["statute"]
        .as_str()
        .unwrap()
        .contains("Section 12(3)"));
}

#[tokio::test]
async fn test_cert_in_incidents_and_rbi_data_localization() {
    let mut config = AppConfig::default();
    config.server.host = "127.0.0.1".to_string();
    config.server.port = 0;
    config.cache.backend = "memory".to_string();
    config.cache.vector_backend = "memory".to_string();

    let state = AppState::new(config);
    let app = create_router(state);

    // 1. CERT-In 6-Hour Incident Reporting Format
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/governance/cert-in/incidents")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "operational");
    assert_eq!(body["mandatory_reporting_sla_hours"], 6);
    assert!(body["cert_in_statutory_directive"]
        .as_str()
        .unwrap()
        .contains("CERT-In"));

    // 2. RBI Payment Data Localization Verification
    let rbi_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/governance/rbi/localization")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rbi_res.status(), StatusCode::OK);
    let bytes = rbi_res.into_body().collect().await.unwrap().to_bytes();
    let rbi_body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(rbi_body["status"], "sovereign_localized");
    assert_eq!(
        rbi_body["regulatory_authority"],
        "Reserve Bank of India (RBI)"
    );
    assert_eq!(rbi_body["full_end_to_end_localization_verified"], true);
    assert_eq!(rbi_body["gst_sac_code"], "998313");
}
