use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sunborn_core::{AppConfig, ApplicationId, TenantId};
use sunborn_db::TenantRecord;
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

fn extract_error_detail(json: &Value) -> String {
    if let Some(detail) = json["detail"].as_str() {
        return detail.to_string();
    }
    if let Some(msg) = json["message"].as_str() {
        return msg.to_string();
    }
    if let Some(msg) = json["error"]["message"].as_str() {
        return msg.to_string();
    }
    json.to_string()
}

#[tokio::test]
async fn test_five_enterprise_kill_switches_under_load() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let admin_key = "sb_live_admin_primary_test_key";
    let quorum_key = "sb_live_admin_secondary_test_key";
    let tenant_id = "tenant-killswitch-test";
    let app_id = "app-killswitch-1";
    let client_key = "sb_live_client_active_key_test";

    // Setup admin identities
    state.api_key_registry.register_key(
        admin_key,
        TenantId::new("system-admin"),
        ApplicationId::new("admin-ops"),
        vec!["*".to_string()],
        true,
    );
    state.api_key_registry.register_key(
        quorum_key,
        TenantId::new("system-admin-sec"),
        ApplicationId::new("admin-ops-sec"),
        vec!["*".to_string()],
        true,
    );

    // Provision active tenant in DB
    state
        .db
        .insert_tenant(TenantRecord {
            id: tenant_id.to_string(),
            name: "Kill Switch Test Tenant".to_string(),
            status: "active".to_string(),
            rate_limit_rps: 100,
            created_at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    // Register initial active client key
    state.api_key_registry.register_key(
        client_key,
        TenantId::new(tenant_id),
        ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );

    let chat_payload = serde_json::json!({
        "model": "gpt-4o",
        "messages": [
            {"role": "user", "content": "Ping test for enterprise kill switch validation"}
        ]
    });

    // 0. Baseline verification: Request succeeds (200 OK)
    let res0 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("x-api-key", client_key)
                .body(Body::from(chat_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res0.status(), StatusCode::OK);

    // =========================================================================
    // KILL SWITCH 1: Suspend Tenant Status in DB -> Immediate 403 Forbidden
    // =========================================================================
    state
        .db
        .insert_tenant(TenantRecord {
            id: tenant_id.to_string(),
            name: "Kill Switch Test Tenant".to_string(),
            status: "suspended".to_string(),
            rate_limit_rps: 100,
            created_at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    let res_ks1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("x-api-key", client_key)
                .body(Body::from(chat_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_ks1.status(), StatusCode::FORBIDDEN);
    let body_bytes = res_ks1.into_body().collect().await.unwrap().to_bytes();
    let json_ks1: Value = serde_json::from_slice(&body_bytes).unwrap();
    let detail_ks1 = extract_error_detail(&json_ks1);
    assert!(
        detail_ks1.contains("is suspended"),
        "Expected suspension detail, got: {}",
        detail_ks1
    );

    // Restore tenant status to active
    state
        .db
        .insert_tenant(TenantRecord {
            id: tenant_id.to_string(),
            name: "Kill Switch Test Tenant".to_string(),
            status: "active".to_string(),
            rate_limit_rps: 100,
            created_at: chrono::Utc::now(),
        })
        .await
        .unwrap();

    // =========================================================================
    // KILL SWITCH 2: Empty / Disallowed Model Allowlist -> 401 Unauthorized Model
    // =========================================================================
    let restricted_key = "sb_live_restricted_model_key";
    state.api_key_registry.register_key(
        restricted_key,
        TenantId::new(tenant_id),
        ApplicationId::new(app_id),
        vec!["claude-3-haiku".to_string()], // Does not allow gpt-4o
        false,
    );

    let res_ks2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("x-api-key", restricted_key)
                .body(Body::from(chat_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_ks2.status(), StatusCode::UNAUTHORIZED);
    let body_bytes = res_ks2.into_body().collect().await.unwrap().to_bytes();
    let json_ks2: Value = serde_json::from_slice(&body_bytes).unwrap();
    let detail_ks2 = extract_error_detail(&json_ks2);
    assert!(
        detail_ks2.contains("not authorized"),
        "Expected unauthorized model detail, got: {}",
        detail_ks2
    );

    // =========================================================================
    // KILL SWITCH 3: Lower Tenant Spend Ceiling to $0.00 -> 429 Too Many Requests
    // =========================================================================
    let ceiling_payload = serde_json::json!({
        "tenant_id": tenant_id,
        "ceiling_usd": 0.00
    });
    let res_ceiling = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/spend-ceiling")
                .header("content-type", "application/json")
                .header("x-api-key", admin_key)
                .body(Body::from(ceiling_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_ceiling.status(), StatusCode::OK);

    let res_ks3 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("x-api-key", client_key)
                .body(Body::from(chat_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_ks3.status(), StatusCode::TOO_MANY_REQUESTS);
    let body_bytes = res_ks3.into_body().collect().await.unwrap().to_bytes();
    let json_ks3: Value = serde_json::from_slice(&body_bytes).unwrap();
    let detail_ks3 = extract_error_detail(&json_ks3);
    assert!(
        detail_ks3.contains("Spend velocity ceiling"),
        "Expected spend ceiling detail, got: {}",
        detail_ks3
    );

    // Reset spend ceiling to standard $100.00
    let reset_ceiling_payload = serde_json::json!({
        "tenant_id": tenant_id,
        "ceiling_usd": 100.00
    });
    let _ = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/spend-ceiling")
                .header("content-type", "application/json")
                .header("x-api-key", admin_key)
                .body(Body::from(reset_ceiling_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // =========================================================================
    // KILL SWITCH 4: Set Enrolled Tenant Quota to 0 -> 429 Too Many Requests
    // =========================================================================
    let quota_payload = serde_json::json!({
        "tenant_id": tenant_id,
        "daily_requests": 0,
        "monthly_requests": 0
    });
    let res_quota = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/tenant-quota")
                .header("content-type", "application/json")
                .header("x-api-key", admin_key)
                .body(Body::from(quota_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_quota.status(), StatusCode::OK);

    let res_ks4 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("x-api-key", client_key)
                .body(Body::from(chat_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_ks4.status(), StatusCode::TOO_MANY_REQUESTS);
    let body_bytes = res_ks4.into_body().collect().await.unwrap().to_bytes();
    let json_ks4: Value = serde_json::from_slice(&body_bytes).unwrap();
    let detail_ks4 = extract_error_detail(&json_ks4);
    assert!(
        detail_ks4.contains("quota exceeded"),
        "Expected quota exceeded detail, got: {}",
        detail_ks4
    );

    // =========================================================================
    // KILL SWITCH 5: Revoke Tenant Keys -> Immediate 401 Unauthorized
    // =========================================================================
    let revoke_payload = serde_json::json!({
        "tenant_id": tenant_id
    });
    let res_revoke = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/credentials/revoke")
                .header("content-type", "application/json")
                .header("x-api-key", admin_key)
                .header("x-quorum-admin-key", quorum_key)
                .body(Body::from(revoke_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_revoke.status(), StatusCode::OK);

    let res_ks5 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("content-type", "application/json")
                .header("x-api-key", client_key)
                .body(Body::from(chat_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_ks5.status(), StatusCode::UNAUTHORIZED);
}
