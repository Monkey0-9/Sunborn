//! Tier defaults at provisioning and the in-place upgrade path.
//!
//! These run against the real router with the in-memory backends. The durable
//! policy write is a no-op without PostgreSQL (documented in
//! `PostgresDatabaseStore::apply_tenant_tier`), so what is asserted here is the
//! admission behaviour a single replica actually applies: the tier's quota is
//! enrolled, identity-gated classes refuse unverified provisioning, and a tier
//! change raises limits on the same tenant without recreating it.

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sunborn_core::{AppConfig, ApplicationId, TenantId};
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

fn state_with_admin() -> AppState {
    let state = AppState::new(AppConfig::default());
    state.api_key_registry.register_key(
        "admin-tier-key",
        TenantId::new("tenant-platform-admin"),
        ApplicationId::new("app-platform"),
        vec!["*".to_string()],
        true,
    );
    state
}

async fn post(app: axum::Router, uri: &str, key: &str, body: Value) -> (StatusCode, Value) {
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("content-type", "application/json")
                .header("x-api-key", key)
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, value)
}

#[tokio::test]
async fn free_tier_defaults_are_returned_and_enrolled() {
    let state = state_with_admin();
    let app = create_router(state.clone());

    let (status, body) = post(
        app,
        "/v1/governance/provision",
        "admin-tier-key",
        json!({
            "tenant_id": "tenant-free-tier",
            "tier": "free",
            "identity_verified": true,
            "invited_by": "tenant-platform-admin"
        }),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "provision failed: {body}");
    assert_eq!(body["tier"], "free");
    assert_eq!(body["daily_requests"], 50);
    assert_eq!(body["monthly_requests"], 1_000);
    assert_eq!(body["retention_days"], 7);
    assert_eq!(body["support_level"], "community");
    assert_eq!(body["invitation_cap"], 5);
    assert_eq!(
        body["allowed_models"],
        json!(["gpt-4o-mini", "claude-3-5-haiku", "gemini-1.5-flash"])
    );

    // 50 admissions, then a refusal on the same window.
    for i in 0..50 {
        assert!(
            state.quota_ledger.reserve("tenant-free-tier").await.is_ok(),
            "admission {i} must be within the free daily limit"
        );
    }
    assert!(
        state
            .quota_ledger
            .reserve("tenant-free-tier")
            .await
            .is_err(),
        "the 51st admission must be refused"
    );
}

#[tokio::test]
async fn identity_gated_tier_refuses_unverified_provisioning() {
    let state = state_with_admin();
    let app = create_router(state);

    let (status, body) = post(
        app,
        "/v1/governance/provision",
        "admin-tier-key",
        json!({ "tenant_id": "tenant-unverified", "tier": "free" }),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN, "unexpected body: {body}");
    assert!(
        body["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("identity_verified"),
        "refusal must name the missing attestation: {body}"
    );
}

#[tokio::test]
async fn an_unknown_tier_is_rejected_instead_of_defaulted() {
    let state = state_with_admin();
    let app = create_router(state);

    let (status, body) = post(
        app,
        "/v1/governance/provision",
        "admin-tier-key",
        json!({ "tenant_id": "tenant-typo", "tier": "platinum" }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "unexpected body: {body}");
    assert!(
        body["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("free, pilot"),
        "the error must list the valid tiers: {body}"
    );
}

#[tokio::test]
async fn provisioning_without_a_tier_keeps_the_generous_pilot_default() {
    let state = state_with_admin();
    let app = create_router(state);

    let (status, body) = post(
        app,
        "/v1/governance/provision",
        "admin-tier-key",
        json!({ "tenant_id": "tenant-pilot-default" }),
    )
    .await;

    assert_eq!(status, StatusCode::OK, "provision failed: {body}");
    assert_eq!(body["tier"], "pilot");
    assert_eq!(body["daily_requests"], 5_000);
    assert_eq!(body["allowed_models"], json!(["*"]));
}

#[tokio::test]
async fn a_tier_only_update_raises_limits_on_the_same_tenant() {
    let state = state_with_admin();
    let app = create_router(state.clone());

    // Enroll a deliberately tiny bespoke envelope.
    let (status, body) = post(
        app.clone(),
        "/v1/governance/tenant-quota",
        "admin-tier-key",
        json!({
            "tenant_id": "tenant-upgrade",
            "tier": "free",
            "daily_requests": 1,
            "monthly_requests": 10
        }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "enrollment failed: {body}");

    assert!(state.quota_ledger.reserve("tenant-upgrade").await.is_ok());
    assert!(
        state.quota_ledger.reserve("tenant-upgrade").await.is_err(),
        "the tiny envelope must be exhausted before the upgrade"
    );

    // Upgrade to the standard policy without recreating the tenant.
    let (status, body) = post(
        app,
        "/v1/governance/tenant-quota",
        "admin-tier-key",
        json!({ "tenant_id": "tenant-upgrade", "tier": "standard" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "upgrade failed: {body}");
    assert_eq!(body["applied"], "tier_policy");
    assert_eq!(body["tier"], "standard");
    assert_eq!(body["daily_requests"], 100_000);
    assert_eq!(body["allowed_models"], json!(["*"]));

    assert!(
        state.quota_ledger.reserve("tenant-upgrade").await.is_ok(),
        "the upgraded tenant must be admitted again on the same identity"
    );
}

#[tokio::test]
async fn a_half_specified_envelope_is_rejected() {
    let state = state_with_admin();
    let app = create_router(state);

    let (status, body) = post(
        app,
        "/v1/governance/tenant-quota",
        "admin-tier-key",
        json!({ "tenant_id": "tenant-half", "tier": "free", "daily_requests": 5 }),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST, "unexpected body: {body}");
    assert!(body["detail"]
        .as_str()
        .unwrap_or_default()
        .contains("provided together"));
}

#[tokio::test]
async fn the_invitation_cap_refuses_the_sixth_free_tenant() {
    let state = state_with_admin();
    let app = create_router(state);

    // The free tier allows five invitations per inviter in a rolling 30 days.
    for i in 0..5 {
        let (status, body) = post(
            app.clone(),
            "/v1/governance/provision",
            "admin-tier-key",
            json!({
                "tenant_id": format!("tenant-invite-{i}"),
                "tier": "free",
                "identity_verified": true,
                "invited_by": "tenant-platform-admin"
            }),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "invite {i} failed: {body}");
    }

    let (status, body) = post(
        app.clone(),
        "/v1/governance/provision",
        "admin-tier-key",
        json!({
            "tenant_id": "tenant-invite-sixth",
            "tier": "free",
            "identity_verified": true,
            "invited_by": "tenant-platform-admin"
        }),
    )
    .await;

    assert_eq!(
        status,
        StatusCode::TOO_MANY_REQUESTS,
        "the sixth invitation must be refused: {body}"
    );
    assert!(
        body["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("invitation cap"),
        "the refusal must name the cap: {body}"
    );

    // The cap is per inviter, not global: a second inviter still has headroom.
    let (status, body) = post(
        app,
        "/v1/governance/provision",
        "admin-tier-key",
        json!({
            "tenant_id": "tenant-invite-other",
            "tier": "free",
            "identity_verified": true,
            "invited_by": "tenant-second-admin"
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "a different inviter must not inherit the exhausted cap: {body}"
    );
}
