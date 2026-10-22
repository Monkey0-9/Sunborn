use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantRecord {
    pub id: String,
    pub name: String,
    pub status: String,
    pub rate_limit_rps: u32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurableCacheRecord {
    pub key: String,
    pub tenant_id: String,
    pub application_id: String,
    pub model: String,
    pub policy_version: String,
    pub hash: String,
    pub prompt_canonical: String,
    pub content: String,
    pub hit_count: u64,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_accessed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxRecord {
    pub id: i64,
    pub tenant_id: String,
    pub entity_type: String,
    pub entity_id: String,
    pub action: String,
    pub payload: serde_json::Value,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub request_id: Uuid,
    pub tenant_id: String,
    pub action: String,
    pub details: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DurableApiCredentialRecord {
    pub key_hash: String,
    pub tenant_id: String,
    pub application_id: String,
    pub rate_limit_rps: u32,
    pub allowed_models: Vec<String>,
    pub is_admin: bool,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct NewApiCredentialRecord {
    pub key_hash: String,
    pub tenant_id: String,
    pub application_id: String,
    pub rate_limit_rps: i32,
    pub allowed_models: Vec<String>,
    pub is_admin: bool,
    pub expires_at: Option<DateTime<Utc>>,
}
