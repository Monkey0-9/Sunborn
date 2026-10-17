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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub r#type: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default)]
    pub tool_call_id: Option<String>,
}

impl ChatMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: Some(content.into()),
            name: None,
            tool_calls: None,
            tool_call_id: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestEnvelope {
    #[serde(default)]
    pub request_id: RequestId,
    pub tenant_id: TenantId,
    pub application_id: ApplicationId,
    pub model: String,
    pub prompt: String,
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub stop_sequences: Vec<String>,
    #[serde(default)]
    pub tools: Vec<ToolDefinition>,
    #[serde(default)]
    pub tool_choice: Option<String>,
    #[serde(default)]
    pub response_format: Option<String>,
    #[serde(default)]
    pub knowledge_epoch: Option<u64>,
    #[serde(default)]
    pub retrieval_context_hashes: Vec<String>,
    #[serde(default)]
    pub dependency_snapshots: Vec<crate::certificate::SourceVersionSnapshot>,
    #[serde(default)]
    pub metadata: serde_json::Value,
    #[serde(default = "chrono::Utc::now")]
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
            messages: Vec::new(),
            system_prompt: None,
            temperature: None,
            top_p: None,
            max_tokens: None,
            stop_sequences: Vec::new(),
            tools: Vec::new(),
            tool_choice: None,
            response_format: None,
            knowledge_epoch: None,
            retrieval_context_hashes: Vec::new(),
            dependency_snapshots: Vec::new(),
            metadata: serde_json::json!({}),
            timestamp: Utc::now(),
        }
    }

    /// Validates request envelope boundaries and constraints
    pub fn validate(&self) -> Result<(), crate::SunbornError> {
        if self.prompt.trim().is_empty() {
            return Err(crate::SunbornError::bad_request("Prompt cannot be empty"));
        }
        if self.prompt.len() > 1_048_576 {
            return Err(crate::SunbornError::bad_request(
                "Prompt size exceeds maximum limit of 1MB",
            ));
        }
        if let Some(t) = self.temperature {
            if !(0.0..=2.0).contains(&t) {
                return Err(crate::SunbornError::bad_request(
                    "Temperature must be between 0.0 and 2.0",
                ));
            }
        }
        if let Some(p) = self.top_p {
            if !(0.0..=1.0).contains(&p) {
                return Err(crate::SunbornError::bad_request(
                    "Top-p must be between 0.0 and 1.0",
                ));
            }
        }
        if self.tenant_id.0.trim().is_empty() {
            return Err(crate::SunbornError::bad_request(
                "Tenant ID cannot be empty",
            ));
        }
        if self.application_id.0.trim().is_empty() {
            return Err(crate::SunbornError::bad_request(
                "Application ID cannot be empty",
            ));
        }
        Ok(())
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_version: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy_id: Option<String>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub certificate: Option<crate::certificate::ReuseCertificate>,
}
