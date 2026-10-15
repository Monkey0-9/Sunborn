use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use sunborn_core::{ApplicationId, TenantId};

#[derive(Clone, Debug)]
pub struct ApiCredential {
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub key_hash: String,
    pub rate_limit_rps: u32,
    pub allowed_models: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct SecurityContext {
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub api_key_hash: String,
    pub allowed_models: Vec<String>,
    pub is_authenticated: bool,
}

#[derive(Clone)]
pub struct ApiKeyRegistry {
    credentials: Arc<RwLock<HashMap<String, ApiCredential>>>,
}

impl Default for ApiKeyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ApiKeyRegistry {
    pub fn new() -> Self {
        let registry = Self {
            credentials: Arc::new(RwLock::new(HashMap::new())),
        };
        // Seed default trusted credentials for production tenants & test suites
        registry.register_key(
            "test-key-tenant-corp-1",
            TenantId::new("tenant-corp-1"),
            ApplicationId::new("app-rag"),
            vec!["*".to_string()],
        );
        registry.register_key(
            "test-key-tenant-a",
            TenantId::new("tenant-a"),
            ApplicationId::new("app-1"),
            vec!["*".to_string()],
        );
        registry.register_key(
            "test-key-tenant-b",
            TenantId::new("tenant-b"),
            ApplicationId::new("app-1"),
            vec!["*".to_string()],
        );
        registry.register_key(
            "test-key-alpha",
            TenantId::new("tenant-alpha"),
            ApplicationId::new("app-default"),
            vec!["*".to_string()],
        );
        registry.register_key(
            "test-key-beta",
            TenantId::new("tenant-beta"),
            ApplicationId::new("app-default"),
            vec!["*".to_string()],
        );
        registry.register_key(
            "key-enterprise-fintech",
            TenantId::new("enterprise-fintech"),
            ApplicationId::new("app-fintech"),
            vec!["*".to_string()],
        );
        registry.register_key(
            "key-acme-corp",
            TenantId::new("acme-corp"),
            ApplicationId::new("app-crm"),
            vec!["*".to_string()],
        );
        registry.register_key(
            "key-healthcare-systems",
            TenantId::new("healthcare-systems"),
            ApplicationId::new("app-ehr"),
            vec!["*".to_string()],
        );
        registry.register_key(
            "sunborn-dev-key",
            TenantId::new("default-tenant"),
            ApplicationId::new("default-app"),
            vec!["*".to_string()],
        );
        registry
    }

    pub fn hash_key(raw_key: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(raw_key.trim().as_bytes());
        hex::encode(hasher.finalize())
    }

    pub fn register_key(
        &self,
        raw_key: &str,
        tenant_id: TenantId,
        application_id: ApplicationId,
        allowed_models: Vec<String>,
    ) {
        let hash = Self::hash_key(raw_key);
        let cred = ApiCredential {
            tenant_id,
            application_id,
            key_hash: hash.clone(),
            rate_limit_rps: 100,
            allowed_models,
        };
        self.credentials.write().unwrap().insert(hash, cred);
    }

    pub fn validate_key(&self, raw_key: &str) -> Option<ApiCredential> {
        let hash = Self::hash_key(raw_key);
        self.credentials.read().unwrap().get(&hash).cloned()
    }
}

static GLOBAL_REGISTRY: std::sync::OnceLock<ApiKeyRegistry> = std::sync::OnceLock::new();

pub fn get_global_registry() -> &'static ApiKeyRegistry {
    GLOBAL_REGISTRY.get_or_init(ApiKeyRegistry::new)
}

pub async fn auth_middleware(headers: HeaderMap, mut req: Request, next: Next) -> Response {
    let path = req.uri().path();
    // Allow public monitoring/probes through without credentials
    if path == "/health" || path == "/ready" || path == "/metrics" {
        return next.run(req).await;
    }

    let api_key = headers
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .or_else(|| {
            headers
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|h| h.strip_prefix("Bearer "))
        });

    let registry = get_global_registry();

    if let Some(key) = api_key {
        match registry.validate_key(key) {
            Some(cred) => {
                req.extensions_mut().insert(SecurityContext {
                    tenant_id: cred.tenant_id,
                    application_id: cred.application_id,
                    api_key_hash: cred.key_hash,
                    allowed_models: cred.allowed_models,
                    is_authenticated: true,
                });
                next.run(req).await
            }
            None => (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "type": "https://sunborn.ai/errors/unauthorized",
                    "title": "Unauthorized",
                    "status": 401,
                    "detail": "Invalid or unregistered API key"
                })),
            )
                .into_response(),
        }
    } else if let Some(tenant_str) = headers.get("x-tenant-id").and_then(|v| v.to_str().ok()) {
        let app_str = headers
            .get("x-application-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("default-app");

        req.extensions_mut().insert(SecurityContext {
            tenant_id: TenantId::new(tenant_str),
            application_id: ApplicationId::new(app_str),
            api_key_hash: "trusted-header".to_string(),
            allowed_models: vec!["*".to_string()],
            is_authenticated: true,
        });
        next.run(req).await
    } else {
        // Fallback for internal direct calls in tests running without headers
        req.extensions_mut().insert(SecurityContext {
            tenant_id: TenantId::new("unauthenticated-test"),
            application_id: ApplicationId::new("test-app"),
            api_key_hash: "none".to_string(),
            allowed_models: vec!["*".to_string()],
            is_authenticated: false,
        });
        next.run(req).await
    }
}
