use axum::body::Body;
use http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use sunborn_core::{AppConfig, RequestEnvelope, ResponseEnvelope};
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_health_endpoint() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "healthy");
    assert_eq!(json["service"], "sunborn-gateway");
}

#[tokio::test]
async fn test_readiness_endpoint() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/ready")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "ready");
}

#[tokio::test]
async fn test_vs001_vs002_vertical_slice_exact_cache() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    state.api_key_registry.register_key(
        "test-key-tenant-corp-1",
        sunborn_core::TenantId::new("tenant-corp-1"),
        sunborn_core::ApplicationId::new("app-rag"),
        vec!["*".to_string()],
        true,
    );

    let req_payload1 = RequestEnvelope::new(
        "tenant-corp-1",
        "app-rag",
        "gpt-4o",
        "  What is semantic caching?  ",
    );

    // 1. First execution: Origin Miss -> Cache Admission
    let response1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-tenant-corp-1")
                .body(Body::from(serde_json::to_vec(&req_payload1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response1.status(), StatusCode::OK);
    let body1 = response1.into_body().collect().await.unwrap().to_bytes();
    let res_env1: ResponseEnvelope = serde_json::from_slice(&body1).unwrap();
    assert!(!res_env1.cached);
    assert_eq!(
        res_env1.decision.action,
        sunborn_core::DecisionAction::OriginExecute
    );
    assert!(res_env1.cache_key.is_some());
    let cache_key = res_env1.cache_key.unwrap();

    // 2. Second execution: Canonical Match -> Exact Cache HIT
    // Notice differing surrounding whitespace to prove canonicalization
    let req_payload2 = RequestEnvelope::new(
        "tenant-corp-1",
        "app-rag",
        "gpt-4o",
        "What is semantic caching?",
    );

    let response2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-tenant-corp-1")
                .body(Body::from(serde_json::to_vec(&req_payload2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response2.status(), StatusCode::OK);
    let body2 = response2.into_body().collect().await.unwrap().to_bytes();
    let res_env2: ResponseEnvelope = serde_json::from_slice(&body2).unwrap();
    assert!(res_env2.cached);
    assert_eq!(
        res_env2.decision.action,
        sunborn_core::DecisionAction::SafeHit
    );
    assert_eq!(res_env2.decision.cache_path, sunborn_core::CachePath::Exact);
    assert_eq!(res_env2.content, res_env1.content);

    // 3. Cache Invalidation via DELETE /v1/caches/:key
    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/caches/{}", cache_key))
                .header("x-api-key", "test-key-tenant-corp-1")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(delete_response.status(), StatusCode::OK);

    // 4. Third execution: Cache Miss after invalidation
    let response3 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-tenant-corp-1")
                .body(Body::from(serde_json::to_vec(&req_payload2).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response3.status(), StatusCode::OK);
    let body3 = response3.into_body().collect().await.unwrap().to_bytes();
    let res_env3: ResponseEnvelope = serde_json::from_slice(&body3).unwrap();
    assert!(!res_env3.cached);
    assert_eq!(
        res_env3.decision.action,
        sunborn_core::DecisionAction::OriginExecute
    );
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let metrics_text = String::from_utf8(body.to_vec()).unwrap();
    assert!(metrics_text.contains("sunborn_requests_total"));
    assert!(metrics_text.contains("sunborn_cache_hits_total"));
    assert!(metrics_text.contains("sunborn_cache_misses_total"));
}

#[tokio::test]
async fn test_vs003_vs004_semantic_cache_and_invalidation_slice() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    state.api_key_registry.register_key(
        "test-key-tenant-corp-1",
        sunborn_core::TenantId::new("tenant-corp-1"),
        sunborn_core::ApplicationId::new("app-rag"),
        vec!["*".to_string()],
        true,
    );

    let source_id = uuid::Uuid::new_v4();

    // 1. Initial request -> Origin Miss & Cache Admission
    let req_payload1 = RequestEnvelope::new(
        "tenant-corp-1",
        "app-rag",
        "gpt-4o",
        "What are the benefits of semantic caching?",
    );

    let response1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-tenant-corp-1")
                .body(Body::from(serde_json::to_vec(&req_payload1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response1.status(), StatusCode::OK);
    let body1 = response1.into_body().collect().await.unwrap().to_bytes();
    let res_env1: ResponseEnvelope = serde_json::from_slice(&body1).unwrap();
    assert!(!res_env1.cached);
    let cache_key = res_env1.cache_key.unwrap();

    // 2. Link this cache key to knowledge source_id
    let dep_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/dependencies")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-tenant-corp-1")
                .body(Body::from(
                    serde_json::json!({
                        "source_id": source_id,
                        "cache_key": cache_key
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(dep_response.status(), StatusCode::OK);

    // 3. Trigger Invalidation for source_id (document updated)
    let inv_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/invalidation")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-tenant-corp-1")
                .body(Body::from(
                    serde_json::json!({
                        "source_id": source_id,
                        "tenant_id": "tenant-corp-1",
                        "version_tag": "v2.0"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(inv_response.status(), StatusCode::OK);
    let inv_body = inv_response.into_body().collect().await.unwrap().to_bytes();
    let inv_json: Value = serde_json::from_slice(&inv_body).unwrap();
    assert_eq!(inv_json["evicted_cache_count"], 1);

    // 4. Verify request after invalidation is an Origin Miss
    let response2 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-tenant-corp-1")
                .body(Body::from(serde_json::to_vec(&req_payload1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response2.status(), StatusCode::OK);
    let body2 = response2.into_body().collect().await.unwrap().to_bytes();
    let res_env2: ResponseEnvelope = serde_json::from_slice(&body2).unwrap();
    assert!(!res_env2.cached);
}

#[tokio::test]
async fn test_finops_report_endpoint() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/finops/report")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "active");
    assert!(
        json["economics"]["gpt4o_cost_avoided_per_query_usd"]
            .as_f64()
            .unwrap()
            > 0.0
    );
    assert!(
        json["economics"]["break_even_hit_rate_threshold"]
            .as_f64()
            .unwrap()
            < 0.20
    );
}

#[tokio::test]
async fn test_radix_routing_and_affinity_endpoints() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    // 1. Inspect radix router status
    let insp_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/routing/radix/inspect")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(insp_resp.status(), StatusCode::OK);
    let insp_body = insp_resp.into_body().collect().await.unwrap().to_bytes();
    let insp_json: Value = serde_json::from_slice(&insp_body).unwrap();
    assert_eq!(insp_json["status"], "operational");

    // 2. Route prompt with prefix warming
    let route_payload = serde_json::json!({
        "prompt": "You are a senior financial analyst analyzing Q4 EBITDA. Please provide summary.",
        "warm_prefix": "You are a senior financial analyst analyzing",
        "warm_node": "inference-node-2"
    });

    let route_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/routing/radix/route")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(route_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(route_resp.status(), StatusCode::OK);
    let route_body = route_resp.into_body().collect().await.unwrap().to_bytes();
    let route_json: Value = serde_json::from_slice(&route_body).unwrap();
    assert_eq!(route_json["selected_node"], "inference-node-2");
    assert_eq!(route_json["is_kv_affinity_hit"], true);
    assert!(route_json["matched_tokens"].as_u64().unwrap() >= 8);
}

#[tokio::test]
async fn test_cryptographic_shredding_endpoint() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let shred_payload = serde_json::json!({
        "tenant_id": "tenant-alpha"
    });

    let shred_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/shred")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(shred_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(shred_resp.status(), StatusCode::OK);
    let shred_body = shred_resp.into_body().collect().await.unwrap().to_bytes();
    let shred_json: Value = serde_json::from_slice(&shred_body).unwrap();
    assert_eq!(shred_json["status"], "shredded");
    assert_eq!(shred_json["tenant_id"], "tenant-alpha");
    assert!(shred_json["shred_latency_nanos"].as_u64().is_some());
    assert_eq!(shred_json["verified_multi_store_erasure"], true);
    assert_eq!(shred_json["residual_records"]["db"], 0);
    assert_eq!(shred_json["residual_records"]["exact_cache"], 0);
}

#[tokio::test]
async fn test_merkle_attestation_and_proof_verification() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let leaf1 = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string();
    let leaf2 = "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210".to_string();

    let attest_payload = serde_json::json!({
        "leaf_hashes": [leaf1, leaf2]
    });

    // 1. Generate Merkle Attestation
    let attest_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/attest")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(attest_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(attest_resp.status(), StatusCode::OK);
    let attest_body = attest_resp.into_body().collect().await.unwrap().to_bytes();
    let attest_json: Value = serde_json::from_slice(&attest_body).unwrap();
    assert_eq!(attest_json["status"], "attested");
    let proofs = attest_json["proofs"].as_array().unwrap();
    assert_eq!(proofs.len(), 2);

    // 2. Verify Inclusion Proof via /v1/governance/verify-proof
    let verify_payload = serde_json::json!({
        "proof": proofs[0]
    });

    let verify_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/verify-proof")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(verify_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(verify_resp.status(), StatusCode::OK);
    let verify_body = verify_resp.into_body().collect().await.unwrap().to_bytes();
    let verify_json: Value = serde_json::from_slice(&verify_body).unwrap();
    assert_eq!(verify_json["is_valid"], true);
}

#[tokio::test]
async fn test_predictive_speculative_prefetching() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let predict_payload = serde_json::json!({
        "current_action": "search_documents",
        "record_transition_to": "fetch_document_content"
    });

    let predict_resp = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/speculative/predict")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(predict_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(predict_resp.status(), StatusCode::OK);
    let predict_body = predict_resp.into_body().collect().await.unwrap().to_bytes();
    let predict_json: Value = serde_json::from_slice(&predict_body).unwrap();
    assert_eq!(predict_json["current_action"], "search_documents");
    assert_eq!(predict_json["speculative_cache_warmed"], true);
    let preds = predict_json["predictions"].as_array().unwrap();
    assert!(!preds.is_empty());
    assert_eq!(preds[0]["predicted_action"], "fetch_document_content");
}

#[tokio::test]
async fn test_cluster_sync_vector_clock_and_autonomic_status() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    // 1. Verify Cluster Sync Status
    let sync_resp = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/cluster/sync/status")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(sync_resp.status(), StatusCode::OK);
    let sync_body = sync_resp.into_body().collect().await.unwrap().to_bytes();
    let sync_json: Value = serde_json::from_slice(&sync_body).unwrap();
    assert_eq!(sync_json["cluster_state"], "synchronized");
    assert_eq!(sync_json["node_id"], "gateway-primary-node-1");

    // 2. Verify Autonomic Incidents
    let auto_resp = app
        .oneshot(
            Request::builder()
                .uri("/v1/autonomic/incidents")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(auto_resp.status(), StatusCode::OK);
    let auto_body = auto_resp.into_body().collect().await.unwrap().to_bytes();
    let auto_json: Value = serde_json::from_slice(&auto_body).unwrap();
    assert_eq!(auto_json["autonomic_health"], "operational");
}

#[tokio::test]
async fn test_runtime_reuse_certificate_admissibility_and_rejection() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    state.api_key_registry.register_key(
        "test-key-proof",
        sunborn_core::TenantId::new("tenant-proof"),
        sunborn_core::ApplicationId::new("app-verification"),
        vec!["*".to_string()],
        true,
    );

    // 1. Initial Request (Cache Miss -> Origin Executed with Signed Certificate)
    let payload1 = serde_json::json!({
        "tenant_id": "tenant-proof",
        "application_id": "app-verification",
        "model": "gpt-4o",
        "prompt": "What are the core properties of acid transactions?",
        "temperature": 0.0,
        "knowledge_epoch": 1
    });

    let resp1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-proof")
                .body(Body::from(payload1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp1.status(), StatusCode::OK);
    let body1 = resp1.into_body().collect().await.unwrap().to_bytes();
    let json1: Value = serde_json::from_slice(&body1).unwrap();
    assert_eq!(json1["decision"]["action"], "origin_execute");

    // 2. Exact Cache Hit -> Valid Certificate Issued & Admissibility Verified
    let resp2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-proof")
                .body(Body::from(payload1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp2.status(), StatusCode::OK);
    let body2 = resp2.into_body().collect().await.unwrap().to_bytes();
    let json2: Value = serde_json::from_slice(&body2).unwrap();
    assert_eq!(json2["decision"]["action"], "safe_hit");
    assert_eq!(json2["cached"], true);
    assert!(json2["certificate"].is_object());
    assert_eq!(json2["certificate"]["tenant_id"], "tenant-proof");
    assert_eq!(json2["certificate"]["model_name"], "gpt-4o");

    // 3. Stale Knowledge Epoch (Client claims epoch 5, lease was epoch 1)
    // Certificate admissibility fails closed -> Abstains to Revalidate
    let payload_stale = serde_json::json!({
        "tenant_id": "tenant-proof",
        "application_id": "app-verification",
        "model": "gpt-4o",
        "prompt": "What are the core properties of acid transactions?",
        "temperature": 0.0,
        "knowledge_epoch": 5
    });

    let resp3 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-proof")
                .body(Body::from(payload_stale.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp3.status(), StatusCode::OK);
    let body3 = resp3.into_body().collect().await.unwrap().to_bytes();
    let json3: Value = serde_json::from_slice(&body3).unwrap();
    assert_eq!(json3["decision"]["action"], "revalidate");
    assert_eq!(json3["cached"], false);
    assert!(json3["decision"]["reason_code"]
        .as_str()
        .unwrap()
        .contains("CERTIFICATE_ADMISSIBILITY_REJECTED"));

    // 4. Exact Cache Entry with Tampered Certificate Signature -> Fails closed to Revalidate
    let req_env1: RequestEnvelope = serde_json::from_value(payload1.clone()).unwrap();
    let identity = sunborn_identity::IdentityEngine::compute_identity(&req_env1);
    if let Ok(Some(mut cached)) = state.cache.get(&identity.key).await {
        cached.metadata["certificate"]["signature"] =
            serde_json::json!("tampered-signature-0000000000000000");
        state.cache.set(cached).await.unwrap();
    }

    let resp4 = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-key-proof")
                .body(Body::from(payload1.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(resp4.status(), StatusCode::OK);
    let body4 = resp4.into_body().collect().await.unwrap().to_bytes();
    let json4: Value = serde_json::from_slice(&body4).unwrap();
    assert_eq!(json4["decision"]["action"], "revalidate");
    assert_eq!(json4["cached"], false);
    assert!(json4["decision"]["reason_code"]
        .as_str()
        .unwrap()
        .contains("CERTIFICATE_ADMISSIBILITY_REJECTED"));
}
