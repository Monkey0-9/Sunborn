use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sunborn_core::{
    certificate::SourceVersionSnapshot, AppConfig, RequestEnvelope, ResponseEnvelope,
};
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn test_graph_caching_request_admission_and_rtc_invalidation() {
    let mut config = AppConfig::default();
    config.cache.graph_dependency_cache_enabled = true;
    let state = AppState::new(config);
    let app = create_router(state.clone());

    state.api_key_registry.register_key(
        "test-graph-key",
        sunborn_core::TenantId::new("tenant-graph-test-1"),
        sunborn_core::ApplicationId::new("app-graph-test-1"),
        vec!["*".to_string()],
        true,
    );

    let src_1 = Uuid::new_v4();
    let src_2 = Uuid::new_v4();

    // 1. Initial request with knowledge dependencies
    let mut req_env = RequestEnvelope::new(
        "tenant-graph-test-1",
        "app-graph-test-1",
        "gpt-4o",
        "What are the Q3 2026 revenue projections for division Alpha?",
    );
    req_env.dependency_snapshots = vec![
        SourceVersionSnapshot {
            source_id: src_1,
            snapshot_epoch: 1,
            content_hash: "hash_alpha_q3".to_string(),
        },
        SourceVersionSnapshot {
            source_id: src_2,
            snapshot_epoch: 1,
            content_hash: "hash_beta_q3".to_string(),
        },
    ];

    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-graph-key")
                .body(Body::from(serde_json::to_vec(&req_env).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res1.status(), StatusCode::OK);
    let body_bytes1 = res1.into_body().collect().await.unwrap().to_bytes();
    let resp1: ResponseEnvelope = serde_json::from_slice(&body_bytes1).unwrap();
    assert!(!resp1.cached, "Initial request must be an origin miss");

    // 2. Second identical request must be an exact cache hit
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-graph-key")
                .body(Body::from(serde_json::to_vec(&req_env).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res2.status(), StatusCode::OK);
    let body_bytes2 = res2.into_body().collect().await.unwrap().to_bytes();
    let resp2: ResponseEnvelope = serde_json::from_slice(&body_bytes2).unwrap();
    assert!(resp2.cached, "Second request must be a cache hit");

    // Compute expected cache key
    let identity = sunborn_identity::IdentityEngine::compute_identity(&req_env);
    let cache_key = identity.key.clone();

    // 3. Inspect Graph Cache Stats API
    let stats_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/cache/graph/stats")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(stats_res.status(), StatusCode::OK);
    let stats_bytes = stats_res.into_body().collect().await.unwrap().to_bytes();
    let stats_json: Value = serde_json::from_slice(&stats_bytes).unwrap();
    assert_eq!(stats_json["status"], "healthy");
    assert_eq!(stats_json["graph_dependency_cache_enabled"], true);
    assert!(stats_json["total_sources"].as_u64().unwrap() >= 2);
    assert!(stats_json["total_edges"].as_u64().unwrap() >= 2);

    // 4. Query Graph Lineage API for this cache key
    let lineage_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/cache/graph/lineage/{}", cache_key))
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(lineage_res.status(), StatusCode::OK);
    let lineage_bytes = lineage_res.into_body().collect().await.unwrap().to_bytes();
    let lineage_json: Value = serde_json::from_slice(&lineage_bytes).unwrap();
    assert_eq!(lineage_json["is_cached"], true);
    let sources = lineage_json["upstream_sources"].as_array().unwrap();
    let source_strings: Vec<String> = sources
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(source_strings.contains(&src_1.to_string()));
    assert!(source_strings.contains(&src_2.to_string()));

    // 5. Trigger Real-Time Causal (RTC) Graph Invalidation on src_1
    let rtc_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/cache/graph/invalidate")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "source_ids": [src_1]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(rtc_res.status(), StatusCode::OK);
    let rtc_bytes = rtc_res.into_body().collect().await.unwrap().to_bytes();
    let rtc_json: Value = serde_json::from_slice(&rtc_bytes).unwrap();
    assert_eq!(rtc_json["status"], "purged");
    let purged_keys: Vec<String> = rtc_json["purged_cache_keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    assert!(purged_keys.contains(&cache_key));

    // 6. Verify cache entry is now immediately purged (is_cached = false)
    let lineage_res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/v1/cache/graph/lineage/{}", cache_key))
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    let lineage_json2: Value =
        serde_json::from_slice(&lineage_res2.into_body().collect().await.unwrap().to_bytes())
            .unwrap();
    assert_eq!(lineage_json2["is_cached"], false);

    // 7. Third request must now MISS because the graph cache was invalidated in real-time causal mode
    let res3 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", "test-graph-key")
                .body(Body::from(serde_json::to_vec(&req_env).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res3.status(), StatusCode::OK);
    let resp3: ResponseEnvelope =
        serde_json::from_slice(&res3.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(
        !resp3.cached,
        "Third request must MISS after Real-Time Causal graph invalidation"
    );
}

#[tokio::test]
async fn test_graph_caching_roaring_simd_multi_doc_invalidation() {
    let mut config = AppConfig::default();
    config.cache.graph_dependency_cache_enabled = true;
    let state = AppState::new(config);
    let app = create_router(state.clone());

    state.api_key_registry.register_key(
        "test-graph-simd-key",
        sunborn_core::TenantId::new("tenant-simd-1"),
        sunborn_core::ApplicationId::new("app-simd-1"),
        vec!["*".to_string()],
        true,
    );

    let doc_hash_1: u64 = 0xAAAA_BBBB_CCCC_DDDD;
    let doc_hash_2: u64 = 0x1111_2222_3333_4444;

    // Direct link into the active graph memory cache
    if let Some(ref gmc) = state.graph_memory_cache {
        gmc.lineage_graph()
            .link_documents(&[doc_hash_1, doc_hash_2], 5001);
        gmc.lineage_graph().link_documents(&[doc_hash_2], 5002);
    }

    // 1. Verify stats report tracked document hashes
    let stats_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/cache/graph/stats")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(stats_res.status(), StatusCode::OK);
    let stats_json: Value =
        serde_json::from_slice(&stats_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(stats_json["total_doc_hashes"].as_u64().unwrap() >= 2);
    assert!(stats_json["total_doc_associations"].as_u64().unwrap() >= 3);

    // 2. Trigger Roaring SIMD multi-document invalidation
    let rtc_res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/cache/graph/invalidate")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "doc_hashes": [doc_hash_1, doc_hash_2]
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(rtc_res.status(), StatusCode::OK);
    let rtc_json: Value =
        serde_json::from_slice(&rtc_res.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(rtc_json["status"], "purged");
    assert_eq!(rtc_json["affected_doc_records_count"], 2);
}
