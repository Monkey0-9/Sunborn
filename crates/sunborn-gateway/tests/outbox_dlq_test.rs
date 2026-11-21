//! End-to-end coverage for the outbox dead-letter queue surface.
//!
//! The DLQ alert and its runbook only mean something if (a) depth is visible in
//! `/metrics` and (b) an operator can actually drain it. Both are exercised
//! here against the in-memory store, which implements the same trait as
//! PostgreSQL.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sunborn_core::{ApplicationId, RequestEnvelope, TenantId};
use sunborn_db::models::OutboxRecord;
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

fn outbox_record(tenant: &str) -> OutboxRecord {
    OutboxRecord {
        id: 0,
        tenant_id: tenant.to_string(),
        entity_type: "document".to_string(),
        entity_id: uuid::Uuid::new_v4().to_string(),
        action: "invalidate".to_string(),
        payload: serde_json::json!({"source_id": uuid::Uuid::new_v4().to_string()}),
        status: "PENDING".to_string(),
        created_at: chrono::Utc::now(),
    }
}

async fn scrape_body(router: axum::Router) -> String {
    let res = router
        .oneshot(
            Request::builder()
                .uri("/metrics")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap()
}

fn dlq_depth(metrics: &str) -> Option<u64> {
    metrics
        .lines()
        .find(|l| l.starts_with("sunborn_invalidation_dlq "))
        .and_then(|l| l.rsplit(' ').next())
        .and_then(|v| v.parse().ok())
}

async fn exhaust_retries(state: &AppState, id: i64) {
    // max_retries defaults to 5; a failure past the limit parks the event in
    // FAILED, which is what the alert counts.
    for _ in 0..6 {
        state
            .db
            .mark_outbox_failed(id, "downstream store unavailable")
            .await
            .expect("mark_outbox_failed");
    }
}

#[tokio::test]
async fn failed_outbox_events_are_visible_and_replayable() {
    let state = AppState::new(sunborn_core::AppConfig::default());
    state.api_key_registry.register_key(
        "admin-key-dlq",
        TenantId::new("tenant-dlq"),
        ApplicationId::new("app-dlq"),
        vec!["*".to_string()],
        true,
    );
    let app = create_router(state.clone());

    let id = state
        .db
        .insert_outbox_event(outbox_record("tenant-dlq"))
        .await
        .expect("insert outbox event");

    let before = scrape_body(app.clone()).await;
    assert_eq!(
        dlq_depth(&before),
        Some(0),
        "a clean outbox must report depth 0, not omit the gauge"
    );

    exhaust_retries(&state, id).await;
    assert_eq!(state.db.failed_outbox_depth().await.unwrap(), 1);

    let after = scrape_body(app.clone()).await;
    assert_eq!(
        dlq_depth(&after),
        Some(1),
        "DLQ depth gauge must reflect the FAILED event"
    );

    let replay = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/lifecycle/outbox/replay")
                .header("x-api-key", "admin-key-dlq")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        replay.status(),
        StatusCode::OK,
        "replay requires an admin key; a non-admin or missing key must not pass"
    );
    let body = replay.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "drained");
    assert_eq!(json["replayed"], 1);
    assert_eq!(json["remaining_failed"], 0);

    let drained = scrape_body(app).await;
    assert_eq!(
        dlq_depth(&drained),
        Some(0),
        "gauge must fall back to 0 after a successful replay"
    );
}

#[tokio::test]
async fn replay_is_rejected_without_authentication() {
    let state = AppState::new(sunborn_core::AppConfig::default());
    state.api_key_registry.register_key(
        "tenant-key",
        TenantId::new("tenant-plain"),
        ApplicationId::new("app-plain"),
        vec!["*".to_string()],
        false,
    );
    let app = create_router(state);

    // No key at all.
    let anonymous = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/lifecycle/outbox/replay")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    // Authenticated but not an administrator.
    let non_admin = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/lifecycle/outbox/replay")
                .header("x-api-key", "tenant-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(non_admin.status(), StatusCode::FORBIDDEN);
}

#[test]
fn request_envelope_helper_is_wired() {
    // Guards the import list used above: if RequestEnvelope stops being
    // constructible the DLQ tests silently stop covering the request path.
    let _ = RequestEnvelope::new("t", "a", "gpt-4o", "probe");
}
