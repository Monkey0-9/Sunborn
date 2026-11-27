//! Enterprise Cloud Provider Adapters (Azure OpenAI, Google Vertex AI, AWS Bedrock).
//!
//! Mirrors the multi-cloud provider surface of AISIX and AgentGateway while
//! feeding directly into Sunborn's `LlmOriginProvider` trait and arbitrage router.

use super::provider::{LlmOriginProvider, OriginExecutionResult};
use async_trait::async_trait;
use std::time::{Duration, Instant};
use sunborn_core::{RequestEnvelope, Result, SunbornError};

fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(45))
        .connect_timeout(Duration::from_secs(5))
        .pool_max_idle_per_host(20)
        .build()
        .unwrap_or_default()
}

/// Azure OpenAI Service adapter (`/openai/deployments/{deployment}/chat/completions`).
pub struct AzureOpenAiProvider {
    pub endpoint_url: String,
    pub deployment: String,
    pub api_version: String,
    pub api_key: String,
    client: reqwest::Client,
}

impl AzureOpenAiProvider {
    pub fn new(
        endpoint_url: impl Into<String>,
        deployment: impl Into<String>,
        api_version: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            endpoint_url: endpoint_url.into(),
            deployment: deployment.into(),
            api_version: api_version.into(),
            api_key: api_key.into(),
            client: build_http_client(),
        }
    }

    pub fn target_url(&self) -> String {
        format!(
            "{}/openai/deployments/{}/chat/completions?api-version={}",
            self.endpoint_url.trim_end_matches('/'),
            self.deployment,
            self.api_version
        )
    }
}

#[async_trait]
impl LlmOriginProvider for AzureOpenAiProvider {
    async fn execute(&self, req: &RequestEnvelope) -> Result<OriginExecutionResult> {
        let start = Instant::now();
        let url = self.target_url();
        let messages = build_chat_messages(req);
        let mut payload = serde_json::json!({ "messages": messages });
        if let Some(t) = req.temperature {
            payload["temperature"] = serde_json::json!(t);
        }
        if let Some(m) = req.max_tokens {
            payload["max_tokens"] = serde_json::json!(m);
        }

        let resp = self
            .client
            .post(&url)
            .header("api-key", &self.api_key)
            .json(&payload)
            .send()
            .await
            .map_err(|e| SunbornError::origin(format!("Azure OpenAI request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(SunbornError::origin(format!(
                "Azure OpenAI HTTP {}: {}",
                status, err
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SunbornError::origin(format!("Azure OpenAI parse error: {}", e)))?;

        let content = body
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let finish_reason = body
            .pointer("/choices/0/finish_reason")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let prompt_tokens = body
            .pointer("/usage/prompt_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;
        let completion_tokens = body
            .pointer("/usage/completion_tokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        Ok(OriginExecutionResult {
            content,
            prompt_tokens,
            completion_tokens,
            model: req.model.clone(),
            latency_ms: start.elapsed().as_millis() as u64,
            finish_reason,
            tool_calls: None,
        })
    }
}

/// Google Cloud Vertex AI Gemini adapter (`generateContent`).
pub struct VertexAiProvider {
    pub project_id: String,
    pub location: String,
    pub access_token: String,
    client: reqwest::Client,
}

impl VertexAiProvider {
    pub fn new(
        project_id: impl Into<String>,
        location: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Self {
        Self {
            project_id: project_id.into(),
            location: location.into(),
            access_token: access_token.into(),
            client: build_http_client(),
        }
    }

    pub fn target_url(&self, model: &str) -> String {
        format!(
            "https://{}-aiplatform.googleapis.com/v1/projects/{}/locations/{}/publishers/google/models/{}:generateContent",
            self.location, self.project_id, self.location, model
        )
    }
}

#[async_trait]
impl LlmOriginProvider for VertexAiProvider {
    async fn execute(&self, req: &RequestEnvelope) -> Result<OriginExecutionResult> {
        let start = Instant::now();
        let url = self.target_url(&req.model);
        let mut payload = serde_json::json!({
            "contents": [{
                "role": "user",
                "parts": [{ "text": req.prompt }]
            }]
        });
        if let Some(sys) = &req.system_prompt {
            payload["systemInstruction"] = serde_json::json!({
                "parts": [{ "text": sys }]
            });
        }

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.access_token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| SunbornError::origin(format!("Vertex AI request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(SunbornError::origin(format!(
                "Vertex AI HTTP {}: {}",
                status, err
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SunbornError::origin(format!("Vertex AI parse error: {}", e)))?;

        let content = body
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let prompt_tokens = body
            .pointer("/usageMetadata/promptTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;
        let completion_tokens = body
            .pointer("/usageMetadata/candidatesTokenCount")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        Ok(OriginExecutionResult {
            content,
            prompt_tokens,
            completion_tokens,
            model: req.model.clone(),
            latency_ms: start.elapsed().as_millis() as u64,
            finish_reason: Some("stop".to_string()),
            tool_calls: None,
        })
    }
}

/// AWS Bedrock Converse API adapter (`/model/{model_id}/converse`).
pub struct BedrockConverseProvider {
    pub region: String,
    pub bearer_token: String,
    client: reqwest::Client,
}

impl BedrockConverseProvider {
    pub fn new(region: impl Into<String>, bearer_token: impl Into<String>) -> Self {
        Self {
            region: region.into(),
            bearer_token: bearer_token.into(),
            client: build_http_client(),
        }
    }

    pub fn target_url(&self, model: &str) -> String {
        format!(
            "https://bedrock-runtime.{}.amazonaws.com/model/{}/converse",
            self.region, model
        )
    }
}

#[async_trait]
impl LlmOriginProvider for BedrockConverseProvider {
    async fn execute(&self, req: &RequestEnvelope) -> Result<OriginExecutionResult> {
        let start = Instant::now();
        let url = self.target_url(&req.model);
        let mut payload = serde_json::json!({
            "messages": [{
                "role": "user",
                "content": [{ "text": req.prompt }]
            }]
        });
        if let Some(sys) = &req.system_prompt {
            payload["system"] = serde_json::json!([{ "text": sys }]);
        }

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&self.bearer_token)
            .json(&payload)
            .send()
            .await
            .map_err(|e| SunbornError::origin(format!("Bedrock request failed: {}", e)))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err = resp.text().await.unwrap_or_default();
            return Err(SunbornError::origin(format!(
                "Bedrock HTTP {}: {}",
                status, err
            )));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| SunbornError::origin(format!("Bedrock parse error: {}", e)))?;

        let content = body
            .pointer("/output/message/content/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let prompt_tokens = body
            .pointer("/usage/inputTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;
        let completion_tokens = body
            .pointer("/usage/outputTokens")
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as u32;

        Ok(OriginExecutionResult {
            content,
            prompt_tokens,
            completion_tokens,
            model: req.model.clone(),
            latency_ms: start.elapsed().as_millis() as u64,
            finish_reason: body
                .get("stopReason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            tool_calls: None,
        })
    }
}

fn build_chat_messages(req: &RequestEnvelope) -> Vec<serde_json::Value> {
    if !req.messages.is_empty() {
        req.messages
            .iter()
            .map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content.clone().unwrap_or_default(),
                })
            })
            .collect()
    } else {
        let mut msgs = Vec::new();
        if let Some(sys) = &req.system_prompt {
            msgs.push(serde_json::json!({ "role": "system", "content": sys }));
        }
        msgs.push(serde_json::json!({ "role": "user", "content": req.prompt }));
        msgs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_provider_urls_match_upstream_specs() {
        let azure = AzureOpenAiProvider::new(
            "https://my-res.openai.azure.com/",
            "gpt4o-dep",
            "2024-10-21",
            "k",
        );
        assert_eq!(
            azure.target_url(),
            "https://my-res.openai.azure.com/openai/deployments/gpt4o-dep/chat/completions?api-version=2024-10-21"
        );
        let vertex = VertexAiProvider::new("my-proj", "us-central1", "tok");
        assert_eq!(
            vertex.target_url("gemini-1.5-pro"),
            "https://us-central1-aiplatform.googleapis.com/v1/projects/my-proj/locations/us-central1/publishers/google/models/gemini-1.5-pro:generateContent"
        );
        let bedrock = BedrockConverseProvider::new("us-east-1", "tok");
        assert_eq!(
            bedrock.target_url("anthropic.claude-3-5-sonnet-20241022-v2:0"),
            "https://bedrock-runtime.us-east-1.amazonaws.com/model/anthropic.claude-3-5-sonnet-20241022-v2:0/converse"
        );
    }
}
