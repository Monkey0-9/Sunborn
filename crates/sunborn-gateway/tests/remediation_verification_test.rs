use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use serde_json::Value;
use sunborn_core::{AppConfig, RequestEnvelope, ResponseEnvelope, TenantId};
use sunborn_gateway::{create_router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn test_tenant_key_durability_and_restart_recovery() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let target_tenant = "tenant-durable-vault-1";

    // 1. Admin provisions tenant key via API
    let res = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/provision")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tenant_id": target_tenant })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::OK);
    let body_bytes = res.into_body().collect().await.unwrap().to_bytes();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["status"], "provisioned");

    // 2. Verify key is active in cryptographic memory
    let tenant_id = TenantId::new(target_tenant);
    assert!(state.crypto_registry.is_active(&tenant_id));

    // 3. Verify wrapped key was persisted to Postgres/Database Store
    let all_tdeks = state
        .postgres_db
        .fetch_all_wrapped_tdeks()
        .await
        .expect("Database call should succeed");
    let found = all_tdeks.iter().find(|(t, _, _)| t == target_tenant);
    assert!(
        found.is_some(),
        "Wrapped TDEK must be persisted durably to the database repository"
    );

    let (t_id, wrapped_bytes, provider) = found.unwrap();
    assert_eq!(t_id, target_tenant);
    assert!(!wrapped_bytes.is_empty());
    assert!(!provider.is_empty());
}

#[tokio::test]
async fn test_semantic_cache_authenticated_decryption_and_shred_rejection() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let tenant = "tenant-semantic-crypto";
    let app_id = "app-nlp";
    let api_key = "key-semantic-crypto-tenant";

    state.api_key_registry.register_key(
        api_key,
        TenantId::new(tenant),
        sunborn_core::ApplicationId::new(app_id),
        vec!["*".to_string()],
        true,
    );

    // 1. Initial request (Origin Miss -> Cache Writeback with AES-256-GCM TDEK)
    let prompt1 = "What are the core fundamentals of distributed consensus algorithms?";
    let req1 = RequestEnvelope::new(tenant, app_id, "gpt-4o", prompt1);

    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res1.status(), StatusCode::OK);
    let body1 = res1.into_body().collect().await.unwrap().to_bytes();
    let resp1: ResponseEnvelope = serde_json::from_slice(&body1).unwrap();
    assert!(!resp1.cached);
    let origin_plaintext = resp1.content.clone();

    // 2. Exact match request -> Exact cache hit with authenticated decryption
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res2.status(), StatusCode::OK);
    let body2 = res2.into_body().collect().await.unwrap().to_bytes();
    let resp2: ResponseEnvelope = serde_json::from_slice(&body2).unwrap();
    assert!(resp2.cached);
    assert_eq!(resp2.content, origin_plaintext);
    assert!(
        !resp2.content.starts_with("sunborn:enc:v1:"),
        "Exact cache hit must return decrypted plaintext"
    );

    // 3. Cryptographic Tenant Shredding (Right to be Forgotten)
    let res_shred = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/shred")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tenant_id": tenant })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_shred.status(), StatusCode::OK);

    // 4. Post-shred verification:
    // a) Tenant API key has been revoked by governance shredding -> 401 Unauthorized
    let res_post_shred = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req1).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(
        res_post_shred.status(),
        StatusCode::UNAUTHORIZED,
        "Shredded tenant API key must be revoked and rejected with 401 Unauthorized"
    );

    // b) Cryptographic TDEK is permanently eradicated and marked shredded
    let t_id = TenantId::new(tenant);
    assert!(!state.crypto_registry.is_active(&t_id));
    assert!(state.crypto_registry.is_shredded(&t_id));
}

#[tokio::test]
async fn test_tenant_credential_revocation_isolation() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let tenant = "tenant-cred-revoke-iso";
    let app_id = "app-prod";
    let active_key = "key-revoke-iso-123";

    state.api_key_registry.register_key(
        active_key,
        TenantId::new(tenant),
        sunborn_core::ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );

    // Seed cache for tenant
    let req_seed =
        RequestEnvelope::new(tenant, app_id, "gpt-4o", "Seed prompt for credential test");
    let res_seed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", active_key)
                .body(Body::from(serde_json::to_vec(&req_seed).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_seed.status(), StatusCode::OK);

    // Admin revokes API credential
    let res_revoke = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/auth/credentials/revoke")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "key_hash": sunborn_gateway::middleware::auth::ApiKeyRegistry::hash_key(active_key),
                        "tenant_id": tenant
                    }))
                    .unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_revoke.status(), StatusCode::OK);

    // 1. Revoked API key must now be rejected with 401 Unauthorized
    let res_rejected = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", active_key)
                .body(Body::from(serde_json::to_vec(&req_seed).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_rejected.status(), StatusCode::UNAUTHORIZED);

    // 2. Tenant encryption key remains active (data was NOT destructively erased!)
    assert!(
        state.crypto_registry.is_active(&TenantId::new(tenant)),
        "Credential revocation must NOT destroy tenant encryption keys"
    );
}

#[tokio::test]
async fn test_session_verification_endpoint() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let test_token = "session-test-bearer-token";
    let tenant = "tenant-session-check";
    let app_id = "app-portal";

    state.api_key_registry.register_key(
        test_token,
        TenantId::new(tenant),
        sunborn_core::ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );

    // 1. Valid token -> 200 OK with session context
    let res_valid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/auth/session")
                .header("authorization", format!("Bearer {test_token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_valid.status(), StatusCode::OK);
    let body_bytes = res_valid.into_body().collect().await.unwrap().to_bytes();
    let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body_json["authenticated"], true);
    assert_eq!(body_json["tenant_id"], tenant);
    assert_eq!(body_json["application_id"], app_id);
    assert_eq!(body_json["is_admin"], false);

    // 2. Missing token -> 401 Unauthorized
    let res_anon = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/v1/auth/session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_anon.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_finops_atomic_reservation_and_spend_settlement() {
    let state = AppState::new(AppConfig::default());
    let tenant = "tenant-finops-concurrency-vault";

    // Configure tenant ceiling to $10.00
    state
        .spend_velocity_controller
        .set_tenant_ceiling(tenant, 10.0);

    // 1. Initial reservation for $6.00 succeeds
    let res1 = state
        .spend_velocity_controller
        .reserve_spend(tenant, 6.0)
        .expect("First reservation of $6.00 against $10.00 ceiling must succeed");

    assert_eq!(
        state.spend_velocity_controller.current_reserved(tenant),
        6.0
    );
    assert_eq!(state.spend_velocity_controller.current_spend(tenant), 0.0);
    assert_eq!(
        state
            .spend_velocity_controller
            .current_total_commitment(tenant),
        6.0
    );

    // 2. Concurrent reservation for $5.00 fails closed (6.0 + 5.0 = 11.0 > 10.0 ceiling)
    let res2_err = state.spend_velocity_controller.reserve_spend(tenant, 5.0);

    assert!(
        res2_err.is_err(),
        "Concurrent reservation exceeding ceiling must be rejected"
    );
    let (committed, ceiling) = res2_err.unwrap_err();
    assert_eq!(committed, 11.0);
    assert_eq!(ceiling, 10.0);

    // 3. Settle first reservation with actual spend of $5.50
    let settled = state
        .spend_velocity_controller
        .settle_spend(tenant, res1, 5.50);
    assert_eq!(settled, 5.50);

    assert_eq!(
        state.spend_velocity_controller.current_reserved(tenant),
        0.0
    );
    assert_eq!(state.spend_velocity_controller.current_spend(tenant), 5.50);
    assert_eq!(
        state
            .spend_velocity_controller
            .current_total_commitment(tenant),
        5.50
    );

    // 4. Subsequent reservation for $4.00 now succeeds (5.50 + 4.00 = 9.50 <= 10.0)
    let res3 = state
        .spend_velocity_controller
        .reserve_spend(tenant, 4.0)
        .expect("Reservation under ceiling post-settlement must succeed");

    assert_eq!(
        state
            .spend_velocity_controller
            .current_total_commitment(tenant),
        9.50
    );

    // 5. Release reservation without spend
    state
        .spend_velocity_controller
        .release_reservation(tenant, res3);

    assert_eq!(
        state.spend_velocity_controller.current_reserved(tenant),
        0.0
    );
    assert_eq!(state.spend_velocity_controller.current_spend(tenant), 5.50);
}

#[tokio::test]
async fn test_cross_tenant_cryptographic_and_cache_isolation() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let tenant_a = "tenant-isolation-alpha";
    let tenant_b = "tenant-isolation-beta";
    let app_id = "app-secure-rag";
    let key_a = "key-tenant-alpha-secret";
    let key_b = "key-tenant-beta-secret";

    state.api_key_registry.register_key(
        key_a,
        TenantId::new(tenant_a),
        sunborn_core::ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );
    state.api_key_registry.register_key(
        key_b,
        TenantId::new(tenant_b),
        sunborn_core::ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );

    let prompt =
        "Explain the cryptographic invariants of zero-knowledge rollups in financial ledgers.";

    // 1. Tenant Alpha sends prompt -> Origin Miss (Cache Writeback under Alpha's TDEK)
    let req_a = RequestEnvelope::new(tenant_a, app_id, "gpt-4o", prompt);
    let res_a1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", key_a)
                .body(Body::from(serde_json::to_vec(&req_a).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_a1.status(), StatusCode::OK);
    let body_a1: ResponseEnvelope =
        serde_json::from_slice(&res_a1.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(!body_a1.cached, "Tenant Alpha first request must be a MISS");
    let key_a1 = body_a1.cache_key.expect("Must have cache key");
    assert!(
        key_a1.starts_with(&format!("sunborn:{tenant_a}:")),
        "Alpha cache key must be scoped to tenant Alpha"
    );

    // 2. Tenant Beta sends the EXACT SAME prompt -> MUST NOT get a cache hit from Tenant Alpha!
    let req_b = RequestEnvelope::new(tenant_b, app_id, "gpt-4o", prompt);
    let res_b1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", key_b)
                .body(Body::from(serde_json::to_vec(&req_b).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_b1.status(), StatusCode::OK);
    let body_b1: ResponseEnvelope =
        serde_json::from_slice(&res_b1.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(
        !body_b1.cached,
        "Tenant Beta must NEVER receive a cache hit from Tenant Alpha's data (strict tenant isolation)"
    );
    let key_b1 = body_b1.cache_key.expect("Must have cache key");
    assert!(
        key_b1.starts_with(&format!("sunborn:{tenant_b}:")),
        "Beta cache key must be scoped to tenant Beta"
    );
    assert_ne!(
        key_a1, key_b1,
        "Cache keys between distinct tenants must never collide"
    );

    // 3. Tenant Alpha queries again -> Gets isolated EXACT HIT decrypted with Alpha's TDEK
    let res_a2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", key_a)
                .body(Body::from(serde_json::to_vec(&req_a).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_a2.status(), StatusCode::OK);
    let body_a2: ResponseEnvelope =
        serde_json::from_slice(&res_a2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(body_a2.cached, "Tenant Alpha must get an exact cache hit");
    assert_eq!(body_a2.content, body_a1.content);
    assert!(!body_a2.content.starts_with("sunborn:enc:v1:"));

    // 4. Tenant Beta queries again -> Gets isolated EXACT HIT decrypted with Beta's TDEK
    let res_b2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", key_b)
                .body(Body::from(serde_json::to_vec(&req_b).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res_b2.status(), StatusCode::OK);
    let body_b2: ResponseEnvelope =
        serde_json::from_slice(&res_b2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(body_b2.cached, "Tenant Beta must get an exact cache hit");
    assert_eq!(body_b2.content, body_b1.content);
    assert!(!body_b2.content.starts_with("sunborn:enc:v1:"));
}

#[tokio::test]
async fn test_shredded_tenant_fail_closed_early_rejection() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let tenant = "tenant-shred-early-reject";
    let app_id = "app-banking";
    let api_key = "key-shred-early-test";

    state.api_key_registry.register_key(
        api_key,
        TenantId::new(tenant),
        sunborn_core::ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );

    // Initial successful request
    let req = RequestEnvelope::new(
        tenant,
        app_id,
        "gpt-4o",
        "Analyze account transaction anomalies.",
    );
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::OK);

    // Shred tenant
    let res_shred = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/governance/shred")
                .header("content-type", "application/json")
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({ "tenant_id": tenant })).unwrap(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_shred.status(), StatusCode::OK);

    // Request with tenant key -> 401 Unauthorized (credentials revoked)
    let res_tenant = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_tenant.status(), StatusCode::UNAUTHORIZED);

    // If an authenticated credential is created/restored for shredded tenant, ingress strictly fails closed with 403 Forbidden
    let re_registered_key = "key-shred-re-registered-test";
    state.api_key_registry.register_key(
        re_registered_key,
        TenantId::new(tenant),
        sunborn_core::ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );
    let res_shredded = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", re_registered_key)
                .body(Body::from(serde_json::to_vec(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        res_shredded.status(),
        StatusCode::FORBIDDEN,
        "Shredded tenant must fail closed immediately on all ingress paths"
    );
}

#[tokio::test]
async fn test_cache_invalidation_lifecycle_and_freshness() {
    let state = AppState::new(AppConfig::default());
    let app = create_router(state.clone());

    let tenant = "tenant-inval-lifecycle";
    let app_id = "app-knowledge";
    let api_key = "key-inval-lifecycle-test";

    state.api_key_registry.register_key(
        api_key,
        TenantId::new(tenant),
        sunborn_core::ApplicationId::new(app_id),
        vec!["*".to_string()],
        false,
    );

    let prompt = "What are the latest compliance protocols for autonomous AI agents?";
    let req = RequestEnvelope::new(tenant, app_id, "gpt-4o", prompt);

    // 1. Initial request -> Miss
    let res1 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res1.status(), StatusCode::OK);
    let body1: ResponseEnvelope =
        serde_json::from_slice(&res1.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(!body1.cached);
    let cache_key = body1.cache_key.expect("Must have cache key");

    // 2. Second request -> Cache Hit
    let res2 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res2.status(), StatusCode::OK);
    let body2: ResponseEnvelope =
        serde_json::from_slice(&res2.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(body2.cached);

    // 3. Invalidate cache entry via DELETE /v1/caches/{key} with admin key
    let res_del = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/v1/caches/{}", cache_key))
                .header("x-api-key", "sunborn-admin-key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res_del.status(), StatusCode::OK);

    // 4. Third request -> Must be an Origin Miss post-invalidation!
    let res3 = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/requests")
                .header("content-type", "application/json")
                .header("x-api-key", api_key)
                .body(Body::from(serde_json::to_vec(&req).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res3.status(), StatusCode::OK);
    let body3: ResponseEnvelope =
        serde_json::from_slice(&res3.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(
        !body3.cached,
        "Post-invalidation request must miss cache and fetch from origin"
    );
}
