use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use sunborn_core::{ApplicationId, TenantId};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiCredential {
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub key_hash: String,
    pub rate_limit_rps: u32,
    pub allowed_models: Vec<String>,
    pub is_admin: bool,
}

#[derive(Clone, Debug)]
pub struct SecurityContext {
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub api_key_hash: String,
    pub allowed_models: Vec<String>,
    pub is_authenticated: bool,
    pub is_admin: bool,
}

impl SecurityContext {
    pub fn require_admin(&self) -> Result<(), &'static str> {
        if !self.is_authenticated || !self.is_admin {
            return Err("Administrative privileges required for this operation");
        }
        Ok(())
    }

    /// Enforces m-of-n administrative quorum for high-consequence operations
    /// (e.g., cryptographic shredding, tenant deletion, chaos fault injection).
    /// When `SUNBORN_ADMIN_QUORUM_REQUIRED=true`, a distinct secondary administrator key
    /// must be supplied via the `x-quorum-admin-key` header.
    pub fn require_admin_quorum(
        &self,
        quorum_key: Option<&str>,
        registry: &ApiKeyRegistry,
    ) -> Result<(), &'static str> {
        self.require_admin()?;
        let quorum_enforced = std::env::var("SUNBORN_ADMIN_QUORUM_REQUIRED")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        if !quorum_enforced {
            return Ok(());
        }

        match quorum_key {
            Some(raw_key) => {
                let quorum_hash = ApiKeyRegistry::hash_key(raw_key);
                if quorum_hash == self.api_key_hash {
                    return Err("Quorum authorization violation: Second administrator key must be distinct from the primary caller");
                }
                match registry.validate_key(raw_key) {
                    Some(cred) if cred.is_admin => Ok(()),
                    _ => Err("Quorum authorization failed: Invalid or unauthorized secondary administrator credentials"),
                }
            }
            None => Err("Quorum authorization required: High-consequence operation requires 'x-quorum-admin-key' header from secondary administrator"),
        }
    }
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
        registry.load_from_env();
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
        is_admin: bool,
    ) {
        let hash = Self::hash_key(raw_key);
        let cred = ApiCredential {
            tenant_id,
            application_id,
            key_hash: hash.clone(),
            rate_limit_rps: 100,
            allowed_models,
            is_admin,
        };
        self.credentials.write().unwrap().insert(hash, cred);
    }

    pub fn validate_key(&self, raw_key: &str) -> Option<ApiCredential> {
        let hash = Self::hash_key(raw_key);
        self.credentials.read().unwrap().get(&hash).cloned()
    }

    /// Hydrates durable API credentials from PostgreSQL into the in-memory lookup cache.
    pub async fn hydrate_from_db(
        &self,
        db: &sunborn_db::PostgresDatabaseStore,
    ) -> Result<usize, sunborn_core::SunbornError> {
        let records = db.fetch_all_api_credentials().await?;
        let count = records.len();
        let mut guard = self.credentials.write().unwrap();
        for r in records {
            guard.insert(
                r.key_hash.clone(),
                ApiCredential {
                    tenant_id: TenantId::new(r.tenant_id),
                    application_id: ApplicationId::new(r.application_id),
                    key_hash: r.key_hash,
                    rate_limit_rps: r.rate_limit_rps,
                    allowed_models: r.allowed_models,
                    is_admin: r.is_admin,
                },
            );
        }
        tracing::info!(
            hydrated_credentials = count,
            "Hydrated durable API credentials from PostgreSQL"
        );
        Ok(count)
    }

    pub fn load_from_env(&self) {
        if let Ok(admin_key) = std::env::var("SUNBORN_ADMIN_KEY") {
            let trimmed = admin_key.trim();
            if !trimmed.is_empty() {
                self.register_key(
                    trimmed,
                    TenantId::new("system-admin"),
                    ApplicationId::new("admin-ops"),
                    vec!["*".to_string()],
                    true,
                );
            }
        }

        if let Ok(keys_str) = std::env::var("SUNBORN_API_KEYS") {
            let trimmed = keys_str.trim();
            if trimmed.starts_with('[') {
                #[derive(Deserialize)]
                struct KeyEntry {
                    key: String,
                    tenant: String,
                    app: String,
                    #[serde(default)]
                    is_admin: bool,
                    #[serde(default)]
                    allowed_models: Vec<String>,
                }
                if let Ok(entries) = serde_json::from_str::<Vec<KeyEntry>>(trimmed) {
                    for entry in entries {
                        let models = if entry.allowed_models.is_empty() {
                            vec!["*".to_string()]
                        } else {
                            entry.allowed_models
                        };
                        self.register_key(
                            &entry.key,
                            TenantId::new(entry.tenant),
                            ApplicationId::new(entry.app),
                            models,
                            entry.is_admin,
                        );
                    }
                }
            } else {
                for item in trimmed.split(',') {
                    let parts: Vec<&str> = item.split(':').map(|s| s.trim()).collect();
                    if parts.len() >= 3 {
                        let is_admin = parts
                            .get(3)
                            .map(|v| *v == "true" || *v == "1")
                            .unwrap_or(false);
                        self.register_key(
                            parts[0],
                            TenantId::new(parts[1]),
                            ApplicationId::new(parts[2]),
                            vec!["*".to_string()],
                            is_admin,
                        );
                    }
                }
            }
        }
    }

    pub fn seed_test_fixtures(&self) {
        self.register_key(
            "sunborn-admin-key",
            TenantId::new("system-admin"),
            ApplicationId::new("admin-ops"),
            vec!["*".to_string()],
            true,
        );
        self.register_key(
            "key-enterprise-fintech",
            TenantId::new("enterprise-fintech"),
            ApplicationId::new("app-fintech"),
            vec!["*".to_string()],
            true,
        );
        self.register_key(
            "sunborn-dev-key",
            TenantId::new("default-tenant"),
            ApplicationId::new("default-app"),
            vec!["*".to_string()],
            true,
        );
        self.register_key(
            "test-key-tenant-corp-1",
            TenantId::new("tenant-corp-1"),
            ApplicationId::new("app-rag"),
            vec!["*".to_string()],
            false,
        );
        self.register_key(
            "test-key-tenant-a",
            TenantId::new("tenant-a"),
            ApplicationId::new("app-1"),
            vec!["*".to_string()],
            false,
        );
        self.register_key(
            "test-key-tenant-b",
            TenantId::new("tenant-b"),
            ApplicationId::new("app-1"),
            vec!["*".to_string()],
            false,
        );
        self.register_key(
            "test-key-alpha",
            TenantId::new("tenant-alpha"),
            ApplicationId::new("app-default"),
            vec!["*".to_string()],
            false,
        );
        self.register_key(
            "test-key-beta",
            TenantId::new("tenant-beta"),
            ApplicationId::new("app-default"),
            vec!["*".to_string()],
            false,
        );
        self.register_key(
            "key-acme-corp",
            TenantId::new("acme-corp"),
            ApplicationId::new("app-crm"),
            vec!["*".to_string()],
            false,
        );
        self.register_key(
            "key-healthcare-systems",
            TenantId::new("healthcare-systems"),
            ApplicationId::new("app-ehr"),
            vec!["*".to_string()],
            false,
        );
    }
}

static GLOBAL_REGISTRY: std::sync::OnceLock<ApiKeyRegistry> = std::sync::OnceLock::new();

pub fn get_global_registry() -> &'static ApiKeyRegistry {
    GLOBAL_REGISTRY.get_or_init(|| {
        let registry = ApiKeyRegistry::new();
        let is_prod = std::env::var("SUNBORN_ENV")
            .map(|v| v.to_lowercase() == "production")
            .unwrap_or(false);
        let allow_fixtures = std::env::var("SUNBORN_ALLOW_TEST_FIXTURES")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(false);

        // Fail-closed security rule: Never seed test credentials in production.
        // In release builds, require explicit SUNBORN_ALLOW_TEST_FIXTURES=true.
        if !is_prod && (allow_fixtures || cfg!(test) || cfg!(debug_assertions)) {
            registry.seed_test_fixtures();
        }
        registry
    })
}

pub async fn auth_middleware(headers: HeaderMap, mut req: Request, next: Next) -> Response {
    let path = req.uri().path();
    // Allow public monitoring/probes and authentication endpoints through without credentials
    if path == "/health" || path == "/ready" || path == "/metrics" || path == "/v1/auth/login" {
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

    match api_key {
        Some(key) => match registry.validate_key(key) {
            Some(cred) => {
                req.extensions_mut().insert(SecurityContext {
                    tenant_id: cred.tenant_id,
                    application_id: cred.application_id,
                    api_key_hash: cred.key_hash,
                    allowed_models: cred.allowed_models,
                    is_authenticated: true,
                    is_admin: cred.is_admin,
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
        },
        None => (
            StatusCode::UNAUTHORIZED,
            Json(json!({
                "type": "https://sunborn.ai/errors/unauthorized",
                "title": "Unauthorized",
                "status": 401,
                "detail": "Missing x-api-key or Authorization Bearer header"
            })),
        )
            .into_response(),
    }
}
