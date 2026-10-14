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
    let app = create_router(state);

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
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert!(text.contains("sunborn_requests_total"));
    assert!(text.contains("sunborn_cache_hits_total"));
}

#[tokio::test]
async fn test_vs003_vs004_semantic_cache_and_invalidation_slice() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

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
