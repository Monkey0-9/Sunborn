//! Native Anthropic Messages API Upstream Adapter
//!
//! Directly speaks the Anthropic Messages wire format (`/v1/messages`)
//! supporting Claude 3.5 Sonnet, Claude 3 Opus, and Claude 3.5 Haiku.

use super::provider::{LlmOriginProvider, OriginExecutionResult};
use async_trait::async_trait;
use std::time::{Duration, Instant};
use sunborn_core::model::RequestEnvelope;
use sunborn_core::{Result, SunbornError};

pub struct AnthropicMessagesProvider {
    pub endpoint_url: String,
    pub api_key: Option<String>,
    client: reqwest::Client,
}

impl AnthropicMessagesProvider {
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
impl LlmOriginProvider for AnthropicMessagesProvider {
    async fn execute(&self, req: &RequestEnvelope) -> Result<OriginExecutionResult> {
        let start = Instant::now();

        let mut target_url = self.endpoint_url.trim_end_matches('/').to_string();
        if !target_url.ends_with("/messages") {
            target_url.push_str("/v1/messages");
        }

        // Format user messages for Anthropic wire schema
        let messages: Vec<serde_json::Value> = if !req.messages.is_empty() {
            req.messages
                .iter()
                .filter(|m| m.role != "system")
                .map(|m| {
                    serde_json::json!({
                        "role": if m.role == "assistant" { "assistant" } else { "user" },
                        "content": m.content.clone().unwrap_or_default(),
                    })
                })
                .collect()
        } else {
            vec![serde_json::json!({
                "role": "user",
                "content": req.prompt,
            })]
        };

        let mut payload = serde_json::json!({
            "model": req.model,
            "max_tokens": req.max_tokens.unwrap_or(2048),
            "messages": messages,
        });

        if let Some(sys) = &req.system_prompt {
            payload["system"] = serde_json::Value::String(sys.clone());
        }
        if let Some(temp) = req.temperature {
            payload["temperature"] = serde_json::json!(temp);
        }

        let mut http_req = self.client.post(&target_url).json(&payload);
        if let Some(key) = &self.api_key {
            http_req = http_req.header("x-api-key", key);
        }
        http_req = http_req.header("anthropic-version", "2023-06-01");

        let resp = http_req.send().await.map_err(|e| {
            SunbornError::origin(format!("Anthropic upstream connection error: {e}"))
        })?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .map_err(|e| SunbornError::origin(format!("Failed to read Anthropic response: {e}")))?;

        if !status.is_success() {
            return Err(SunbornError::origin(format!(
                "Anthropic upstream returned HTTP {}: {}",
                status, body_text
            )));
        }

        let parsed: serde_json::Value = serde_json::from_str(&body_text)
            .map_err(|e| SunbornError::origin(format!("Failed to parse Anthropic JSON: {e}")))?;

        let content = parsed
            .get("content")
            .and_then(|c| c.as_array())
            .and_then(|arr| arr.first())
            .and_then(|block| block.get("text"))
            .and_then(|t| t.as_str())
            .unwrap_or_default()
            .to_string();

        let prompt_tokens = parsed
            .get("usage")
            .and_then(|u| u.get("input_tokens"))
            .and_then(|n| n.as_u64())
            .unwrap_or(0) as u32;

        let completion_tokens = parsed
            .get("usage")
            .and_then(|u| u.get("output_tokens"))
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
