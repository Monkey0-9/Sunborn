//! Native Google Gemini API Upstream Adapter
//!
//! Speaks Google Gemini generateContent wire format directly,
//! supporting Gemini 1.5 Pro and Gemini 1.5 Flash.

use super::provider::{LlmOriginProvider, OriginExecutionResult};
use async_trait::async_trait;
use std::time::{Duration, Instant};
use sunborn_core::model::RequestEnvelope;
use sunborn_core::{Result, SunbornError};

pub struct GeminiProvider {
    pub endpoint_url: String,
    pub api_key: Option<String>,
    client: reqwest::Client,
}

impl GeminiProvider {
    pub fn new(endpoint_url: impl Into<String>, api_key: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(5))
            .pool_max_idle_per_host(20)
            .build()
            .unwrap_or_default();

        Self {
            endpoint_url: endpoint_url.into(),
            api_key,
            client,
        }
    }
}

#[async_trait]
impl LlmOriginProvider for GeminiProvider {
    async fn execute(&self, req: &RequestEnvelope) -> Result<OriginExecutionResult> {
        let start = Instant::now();

        let model_id = if req.model.starts_with("gemini-") {
            req.model.as_str()
        } else {
            "gemini-1.5-pro"
        };

        let base = self.endpoint_url.trim_end_matches('/');
        let target_url = if base.contains(":generateContent") {
            base.to_string()
        } else {
            format!("{}/v1beta/models/{}:generateContent", base, model_id)
        };

        let prompt_text = if !req.messages.is_empty() {
            req.messages
                .iter()
                .map(|m| format!("{}: {}", m.role, m.content.as_deref().unwrap_or("")))
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            req.prompt.clone()
        };

        let payload = serde_json::json!({
            "contents": [
                {
                    "parts": [
                        { "text": prompt_text }
                    ]
                }
            ]
        });

        let mut http_req = self.client.post(&target_url).json(&payload);
        if let Some(key) = &self.api_key {
            http_req = http_req.header("x-goog-api-key", key);
        }

        let resp = http_req
            .send()
            .await
            .map_err(|e| SunbornError::origin(format!("Gemini upstream connection error: {e}")))?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .map_err(|e| SunbornError::origin(format!("Failed to read Gemini response: {e}")))?;

        if !status.is_success() {
            return Err(SunbornError::origin(format!(
                "Gemini upstream returned HTTP {}: {}",
                status, body_text
            )));
        }

        let parsed: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| SunbornError::origin(format!("Failed to parse Gemini JSON: {e}")))?;

        let content = parsed
            .get("candidates")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|cand| cand.get("content"))
            .and_then(|cnt| cnt.get("parts"))
            .and_then(|parts| parts.as_array())
            .and_then(|p_arr| p_arr.first())
            .and_then(|p| p.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string();

        let prompt_tokens = parsed
            .get("usageMetadata")
            .and_then(|u| u.get("promptTokenCount"))
            .and_then(|n| n.as_u64())
            .unwrap_or(0) as u32;

        let completion_tokens = parsed
            .get("usageMetadata")
            .and_then(|u| u.get("candidatesTokenCount"))
            .and_then(|n| n.as_u64())
            .unwrap_or(0) as u32;

        Ok(OriginExecutionResult {
            content,
            model: req.model.clone(),
            prompt_tokens,
            completion_tokens,
            latency_ms: start.elapsed().as_millis() as u64,
            finish_reason: Some("stop".to_string()),
            tool_calls: None,
        })
    }

    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}
