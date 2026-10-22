use axum::body::Body;
use axum::http::{Request, StatusCode};
use chrono::{Duration, Utc};
use http_body_util::BodyExt;
use sunborn_core::model::{DecisionAction, TenantId};
use sunborn_core::{AppConfig, CachePath, RequestEnvelope, ResponseEnvelope, ReuseCertificate};
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_reuse_certificate_authenticated_and_validated_on_reuse_path() {
    let mut config = AppConfig::default();
    config.cache.exact_cache_enabled = true;
    config.cache.semantic_cache_enabled = false;
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let req_payload1 = RequestEnvelope::new(
        "enterprise-fintech",
        "app-fintech",
        "gpt-4o",
        "Explain deterministic execution reuse in distributed caching",
    );

    // 1. Initial Call: Cache MISS (Admitted to cache with signed ReuseCertificate)
    let resp1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req_payload1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp1.status(), StatusCode::OK);
    let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
    let res_env1: ResponseEnvelope = serde_json::from_slice(&body1).unwrap();
    assert!(!res_env1.cached);
    assert_eq!(res_env1.decision.action, DecisionAction::OriginExecute);

    let cache_key = res_env1.cache_key.expect("Cache key must be generated");

    // Verify the entry in the cache store contains a cryptographically signed ReuseCertificate
    let cached_entry = state
        .cache
        .get(&cache_key)
        .await
        .unwrap()
        .expect("Cache entry must be present in store");

    let cert_val = cached_entry
        .metadata
        .get("certificate")
        .expect("Certificate metadata must exist");
    let cert: ReuseCertificate = serde_json::from_value(cert_val.clone()).unwrap();

    // Verify certificate Merkle root and RFC 2104 HMAC-SHA-256 signature
    assert!(cert.verify_integrity(&state.learning_hmac_secret).is_ok());
    assert_eq!(cert.tenant_id, TenantId::new("enterprise-fintech"));
    assert_eq!(cert.model_name, "gpt-4o");

    // 2. Second Call: Cache HIT (Exact C0/C1) -> Validated through verify_admissibility
    let req_payload2 = RequestEnvelope::new(
        "enterprise-fintech",
        "app-fintech",
        "gpt-4o",
        "Explain deterministic execution reuse in distributed caching",
    );

    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req_payload2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp2.status(), StatusCode::OK);
    let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let res_env2: ResponseEnvelope = serde_json::from_slice(&body2).unwrap();

    assert!(res_env2.cached, "Second request must be served from cache");
    assert_eq!(res_env2.decision.action, DecisionAction::SafeHit);
    assert_eq!(res_env2.decision.cache_path, CachePath::Exact);
    assert!(res_env2.certificate.is_some());

    let returned_cert = res_env2.certificate.unwrap();
    assert_eq!(returned_cert.certificate_id, cert.certificate_id);
    assert_eq!(returned_cert.merkle_root, cert.merkle_root);
}

#[tokio::test]
async fn test_reuse_certificate_tampered_signature_abstains_to_revalidate() {
    let mut config = AppConfig::default();
    config.cache.exact_cache_enabled = true;
    config.cache.semantic_cache_enabled = false;
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let req_payload = RequestEnvelope::new(
        "enterprise-fintech",
        "app-fintech",
        "gpt-4o",
        "Quantum computing error correction thresholds",
    );

    // Initial admission
    let resp1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp1.status(), StatusCode::OK);
    let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
    let res_env1: ResponseEnvelope = serde_json::from_slice(&body1).unwrap();
    let cache_key = res_env1.cache_key.unwrap();

    // Tamper with the cached certificate's HMAC-SHA256 signature in the cache
    let mut cached_entry = state.cache.get(&cache_key).await.unwrap().unwrap();
    let mut cert: ReuseCertificate =
        serde_json::from_value(cached_entry.metadata.get("certificate").unwrap().clone()).unwrap();

    // Corrupt signature bytes
    cert.signature = "deadbeefcafebabe0000111122223333444455556666777788889999aaaabbbb".to_string();
    cached_entry.metadata["certificate"] = serde_json::to_value(cert).unwrap();
    state.cache.set(cached_entry).await.unwrap();

    // Second request: Gateway must detect tampered signature and abstain to Revalidate
    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp2.status(), StatusCode::OK);
    let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let res_env2: ResponseEnvelope = serde_json::from_slice(&body2).unwrap();

    assert!(
        !res_env2.cached,
        "Must not reuse entry with tampered signature"
    );
    assert_eq!(res_env2.decision.action, DecisionAction::OriginExecute);
    assert!(res_env2
        .content
        .contains("[Origin Response from gpt-4o for tenant enterprise-fintech]"));
}

#[tokio::test]
async fn test_reuse_certificate_tampered_merkle_root_abstains_to_revalidate() {
    let mut config = AppConfig::default();
    config.cache.exact_cache_enabled = true;
    config.cache.semantic_cache_enabled = false;
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let req_payload = RequestEnvelope::new(
        "enterprise-fintech",
        "app-fintech",
        "gpt-4o",
        "Post-quantum cryptographic signature standards",
    );

    // Initial admission
    let resp1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp1.status(), StatusCode::OK);
    let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
    let res_env1: ResponseEnvelope = serde_json::from_slice(&body1).unwrap();
    let cache_key = res_env1.cache_key.unwrap();

    // Tamper with the Merkle root in the cached certificate
    let mut cached_entry = state.cache.get(&cache_key).await.unwrap().unwrap();
    let mut cert: ReuseCertificate =
        serde_json::from_value(cached_entry.metadata.get("certificate").unwrap().clone()).unwrap();

    cert.merkle_root =
        "0000000000000000000000000000000000000000000000000000000000000000".to_string();
    cached_entry.metadata["certificate"] = serde_json::to_value(cert).unwrap();
    state.cache.set(cached_entry).await.unwrap();

    // Second request: Gateway must detect tampered Merkle root and abstain
    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp2.status(), StatusCode::OK);
    let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let res_env2: ResponseEnvelope = serde_json::from_slice(&body2).unwrap();

    assert!(
        !res_env2.cached,
        "Must not reuse entry with tampered Merkle root"
    );
    assert_eq!(res_env2.decision.action, DecisionAction::OriginExecute);
    assert!(res_env2
        .content
        .contains("[Origin Response from gpt-4o for tenant enterprise-fintech]"));
}

#[tokio::test]
async fn test_reuse_certificate_epoch_advance_abstains_to_revalidate() {
    let mut config = AppConfig::default();
    config.cache.exact_cache_enabled = true;
    config.cache.semantic_cache_enabled = false;
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let mut req_payload1 = RequestEnvelope::new(
        "enterprise-fintech",
        "app-fintech",
        "gpt-4o",
        "What is the current liquidity ratio for treasury desk 4?",
    );
    req_payload1.knowledge_epoch = Some(1);

    // Initial admission under epoch 1
    let resp1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req_payload1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp1.status(), StatusCode::OK);
    let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
    let res_env1: ResponseEnvelope = serde_json::from_slice(&body1).unwrap();
    let cache_key = res_env1.cache_key.unwrap();

    // Re-sign certificate with lease epoch = 1
    let mut cached_entry = state.cache.get(&cache_key).await.unwrap().unwrap();
    let old_cert: ReuseCertificate =
        serde_json::from_value(cached_entry.metadata.get("certificate").unwrap().clone()).unwrap();

    let restricted_cert = ReuseCertificate::new_signed(
        old_cert.tenant_id,
        old_cert.application_id,
        old_cert.request_identity_hash,
        old_cert.model_name,
        old_cert.prompt_tokens,
        old_cert.source_snapshots,
        old_cert.safety_evidence,
        sunborn_core::ValidityWindow {
            issued_at: Utc::now() - Duration::seconds(10),
            expires_at: Utc::now() + Duration::seconds(60),
            monotonic_epoch_lease: 1, // lease valid only for epoch <= 1
        },
        &state.learning_hmac_secret,
    );
    cached_entry.metadata["certificate"] = serde_json::to_value(restricted_cert).unwrap();
    state.cache.set(cached_entry).await.unwrap();

    // Subsequent request with advanced knowledge epoch = 2
    let mut req_payload2 = req_payload1.clone();
    req_payload2.knowledge_epoch = Some(2);

    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req_payload2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp2.status(), StatusCode::OK);
    let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let res_env2: ResponseEnvelope = serde_json::from_slice(&body2).unwrap();

    assert!(
        !res_env2.cached,
        "Must not reuse certificate across monotonic epoch advance"
    );
    assert_eq!(res_env2.decision.action, DecisionAction::OriginExecute);
    assert!(res_env2
        .content
        .contains("[Origin Response from gpt-4o for tenant enterprise-fintech]"));
}

#[tokio::test]
async fn test_reuse_certificate_expired_validity_window_abstains_to_revalidate() {
    let mut config = AppConfig::default();
    config.cache.exact_cache_enabled = true;
    config.cache.semantic_cache_enabled = false;
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let req_payload = RequestEnvelope::new(
        "enterprise-fintech",
        "app-fintech",
        "gpt-4o",
        "Real-time FX spot rates USD/EUR",
    );

    // Initial admission
    let resp1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp1.status(), StatusCode::OK);
    let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
    let res_env1: ResponseEnvelope = serde_json::from_slice(&body1).unwrap();
    let cache_key = res_env1.cache_key.unwrap();

    // Re-sign certificate with expired validity window (expired 10 seconds ago)
    let mut cached_entry = state.cache.get(&cache_key).await.unwrap().unwrap();
    let old_cert: ReuseCertificate =
        serde_json::from_value(cached_entry.metadata.get("certificate").unwrap().clone()).unwrap();

    let expired_cert = ReuseCertificate::new_signed(
        old_cert.tenant_id,
        old_cert.application_id,
        old_cert.request_identity_hash,
        old_cert.model_name,
        old_cert.prompt_tokens,
        old_cert.source_snapshots,
        old_cert.safety_evidence,
        sunborn_core::ValidityWindow {
            issued_at: Utc::now() - Duration::seconds(60),
            expires_at: Utc::now() - Duration::seconds(10), // expired in past
            monotonic_epoch_lease: 100,
        },
        &state.learning_hmac_secret,
    );
    cached_entry.metadata["certificate"] = serde_json::to_value(expired_cert).unwrap();
    state.cache.set(cached_entry).await.unwrap();

    // Second request: Gateway must detect lease expiration and abstain
    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp2.status(), StatusCode::OK);
    let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let res_env2: ResponseEnvelope = serde_json::from_slice(&body2).unwrap();

    assert!(!res_env2.cached, "Must not reuse expired certificate");
    assert_eq!(res_env2.decision.action, DecisionAction::OriginExecute);
    assert!(res_env2
        .content
        .contains("[Origin Response from gpt-4o for tenant enterprise-fintech]"));
}

#[tokio::test]
async fn test_reuse_certificate_omitted_dependency_snapshot_abstains_to_revalidate() {
    let mut config = AppConfig::default();
    config.cache.exact_cache_enabled = true;
    config.cache.semantic_cache_enabled = false;
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let doc_id_1 = uuid::Uuid::new_v4();
    let doc_id_2 = uuid::Uuid::new_v4();

    // 1. Initial request with authoritative dependency doc_id_1 only
    let mut req1 = RequestEnvelope::new(
        "enterprise-fintech",
        "app-fintech",
        "gpt-4o",
        "Calculate capital adequacy ratio based on risk-weighted assets",
    );
    req1.dependency_snapshots = vec![sunborn_core::SourceVersionSnapshot {
        source_id: doc_id_1,
        snapshot_epoch: 1,
        content_hash: "hash-doc-1-v1".to_string(),
    }];

    let resp1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp1.status(), StatusCode::OK);
    let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
    let res_env1: ResponseEnvelope = serde_json::from_slice(&body1).unwrap();
    assert!(!res_env1.cached);
    assert_eq!(res_env1.decision.action, DecisionAction::OriginExecute);

    // 2. Second request: Current authoritative graph now requires doc_id_1 AND doc_id_2,
    // but the cached certificate only attested to doc_id_1 (omitted doc_id_2)
    let mut req2 = req1.clone();
    req2.dependency_snapshots = vec![
        sunborn_core::SourceVersionSnapshot {
            source_id: doc_id_1,
            snapshot_epoch: 1,
            content_hash: "hash-doc-1-v1".to_string(),
        },
        sunborn_core::SourceVersionSnapshot {
            source_id: doc_id_2,
            snapshot_epoch: 1,
            content_hash: "hash-doc-2-v1".to_string(),
        },
    ];

    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp2.status(), StatusCode::OK);
    let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let res_env2: ResponseEnvelope = serde_json::from_slice(&body2).unwrap();

    // Must strictly reject reuse because certificate omitted required dependency doc_id_2
    assert!(
        !res_env2.cached,
        "Cache reuse must be rejected when certificate omits a required source dependency"
    );
    assert_eq!(res_env2.decision.action, DecisionAction::OriginExecute);
    assert!(res_env2
        .content
        .contains("[Origin Response from gpt-4o for tenant enterprise-fintech]"));
}

#[tokio::test]
async fn test_reuse_certificate_duplicate_dependency_in_request_rejected() {
    let mut config = AppConfig::default();
    config.cache.exact_cache_enabled = true;
    config.cache.semantic_cache_enabled = false;
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let doc_id = uuid::Uuid::new_v4();

    // Initial request admitted
    let mut req1 = RequestEnvelope::new(
        "enterprise-fintech",
        "app-fintech",
        "gpt-4o",
        "Explain Basel III leverage ratio requirements",
    );
    req1.dependency_snapshots = vec![sunborn_core::SourceVersionSnapshot {
        source_id: doc_id,
        snapshot_epoch: 1,
        content_hash: "hash-doc-base".to_string(),
    }];

    let resp1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp1.status(), StatusCode::OK);

    // Second request with duplicate dependency entries
    let mut req2 = req1.clone();
    req2.dependency_snapshots = vec![
        sunborn_core::SourceVersionSnapshot {
            source_id: doc_id,
            snapshot_epoch: 1,
            content_hash: "hash-doc-base".to_string(),
        },
        sunborn_core::SourceVersionSnapshot {
            source_id: doc_id,
            snapshot_epoch: 1,
            content_hash: "hash-doc-base".to_string(),
        },
    ];

    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp2.status(), StatusCode::OK);
    let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let res_env2: ResponseEnvelope = serde_json::from_slice(&body2).unwrap();

    assert!(!res_env2.cached);
    assert_eq!(res_env2.decision.action, DecisionAction::OriginExecute);
    assert!(res_env2
        .content
        .contains("[Origin Response from gpt-4o for tenant enterprise-fintech]"));
}

#[tokio::test]
async fn test_semantic_cache_missing_certificate_falls_through_to_origin() {
    let mut config = AppConfig::default();
    config.cache.exact_cache_enabled = false;
    config.cache.semantic_cache_enabled = true;
    let state = AppState::new(config);
    let app = create_router(state.clone());

    let prompt = "Tell me about macroeconomic policy stability";
    let vector = state.selected_embedder.embed(prompt).await.unwrap();

    let tenant_id = TenantId::new("enterprise-fintech");
    let app_id = sunborn_core::model::ApplicationId::new("app-fintech");
    let entry = sunborn_semantic::VectorEntry {
        id: "vector-no-cert-test".to_string(),
        tenant_id: tenant_id.clone(),
        application_id: app_id.clone(),
        prompt: prompt.to_string(),
        content: "Unverified cached response without certificate".to_string(),
        vector,
        metadata: serde_json::json!({}),
        created_at: Utc::now(),
    };
    state.vector_store.insert(entry).await.unwrap();

    let req_payload = RequestEnvelope::new("enterprise-fintech", "app-fintech", "gpt-4o", prompt);

    let resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "key-enterprise-fintech")
                .body(Body::from(serde_json::to_vec(&req_payload).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp.status(), StatusCode::OK);
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let res_env: ResponseEnvelope = serde_json::from_slice(&body).unwrap();

    // Must NOT return the unverified content; must execute origin
    assert!(!res_env.cached);
    assert_eq!(res_env.decision.action, DecisionAction::OriginExecute);
    assert!(res_env
        .content
        .contains("[Origin Response from gpt-4o for tenant enterprise-fintech]"));
    assert!(!res_env
        .content
        .contains("Unverified cached response without certificate"));
}
