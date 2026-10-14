use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use sunborn_core::{ApplicationId, TenantId};

#[derive(Clone, Debug)]
pub struct SecurityContext {
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub api_key: String,
}

pub async fn auth_middleware(
    headers: HeaderMap,
    mut req: Request,
    next: Next,
) -> Response {
    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "))
        });

    let tenant = headers
        .get("x-tenant-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("default-tenant");

    let app = headers
        .get("x-application-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("default-app");

    // Allow /health, /ready, /metrics, /v1/invalidation without strict API key check for monitoring probes
    let path = req.uri().path();
    if path == "/health" || path == "/ready" || path == "/metrics" {
        return next.run(req).await;
    }

    if let Some(key) = api_key {
        req.extensions_mut().insert(SecurityContext {
            tenant_id: TenantId::new(tenant),
            application_id: ApplicationId::new(app),
            api_key: key.to_string(),
        });
        next.run(req).await
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "type": "https://sunborn.ai/errors/unauthorized",
                "title": "Unauthorized",
                "status": 401,
                "detail": "Missing x-api-key or Authorization Bearer header"
            })),
        )
            .into_response()
    }
}
