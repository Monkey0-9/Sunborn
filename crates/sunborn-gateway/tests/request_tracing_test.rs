use axum::body::Body;
use http::{Request, StatusCode};
use sunborn_core::AppConfig;
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_request_id_generated_when_missing() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let req_id_header = res.headers().get("x-request-id");
    assert!(
        req_id_header.is_some(),
        "x-request-id header must be injected"
    );
    let req_id = req_id_header.unwrap().to_str().unwrap();
    assert!(!req_id.is_empty());
    // Must be valid UUID format
    assert!(
        uuid::Uuid::parse_str(req_id).is_ok(),
        "Generated request ID must be valid UUIDv4"
    );
}

#[tokio::test]
async fn test_request_id_preserved_when_supplied_by_client() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    let client_id = "client-trace-abc-12345";
    let res = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/health")
                .header("x-request-id", client_id)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let echoed = res.headers().get("x-request-id").unwrap().to_str().unwrap();
    assert_eq!(
        echoed, client_id,
        "Client supplied x-request-id must be preserved"
    );
}

#[tokio::test]
async fn test_request_id_present_on_unauthorized_responses() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state);

    // Call protected endpoint without credentials
    let res = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .body(Body::from(b"{}".to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let req_id_header = res.headers().get("x-request-id");
    assert!(
        req_id_header.is_some(),
        "x-request-id must be returned even on 401 Unauthorized errors"
    );
}
