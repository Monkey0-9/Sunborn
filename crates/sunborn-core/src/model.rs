use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TenantId(pub String);

impl TenantId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ApplicationId(pub String);

impl ApplicationId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RequestId(pub Uuid);

impl RequestId {
    pub fn generate() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for RequestId {
    fn default() -> Self {
        Self::generate()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CorrectnessTier {
    C0StrictExact,
    C1DeterministicEquivalent,
    C2SemanticBoundedRisk,
    C3FreshnessGuaranteed,
    C4ShadowValidated,
    C5DegradedFallback,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionAction {
    SafeHit,
    Revalidate,
    PartialReuse,
    OriginExecute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CachePath {
    Exact,
    Semantic,
    Execution,
    Miss,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    pub request_id: RequestId,
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub timestamp: DateTime<Utc>,
}

impl RequestEnvelope {
    pub fn new(
        tenant_id: impl Into<String>,
        application_id: impl Into<String>,
        model: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            request_id: RequestId::generate(),
            tenant_id: TenantId::new(tenant_id),
            application_id: ApplicationId::new(application_id),
            model: model.into(),
            prompt: prompt.into(),
            system_prompt: None,
            temperature: None,
            max_tokens: None,
            metadata: serde_json::json!({}),
            timestamp: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub action: DecisionAction,
    pub cache_path: CachePath,
    pub confidence: f32,
    pub risk_score: f32,
    pub reason_code: String,
    pub latency_saved_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResponseEnvelope {
    pub request_id: RequestId,
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub decision: DecisionRecord,
    pub content: String,
    pub cached: bool,
    pub cache_key: Option<String>,
    pub created_at: DateTime<Utc>,
    pub duration_ms: u64,
}
