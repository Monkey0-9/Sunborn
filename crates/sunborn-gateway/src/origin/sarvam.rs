//! Native Sarvam AI Indic Sovereign Model Adapter
//!
//! Directly interfaces with Sarvam AI's inference endpoints, providing
//! native sovereign LLM capabilities (sarvam-2b, bulbul:v1) with dual
//! authentication (subscription key / bearer token) and Indic token tracking.

use super::provider::{LlmOriginProvider, OriginExecutionResult};
use async_trait::async_trait;
use std::time::{Duration, Instant};
use sunborn_core::model::RequestEnvelope;
use sunborn_core::{Result, SunbornError};

pub struct SarvamOriginProvider {
    pub endpoint_url: String,
    pub api_key: Option<String>,
    client: reqwest::Client,
}

impl SarvamOriginProvider {
    pub fn new(endpoint_url: impl Into<String>, api_key: Option<String>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(45))
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

    pub fn default_sovereign(api_key: Option<String>) -> Self {
        Self::new("https://api.sarvam.ai/v1", api_key)
    }
}

#[async_trait]
impl LlmOriginProvider for SarvamOriginProvider {
    async fn execute(&self, req: &RequestEnvelope) -> Result<OriginExecutionResult> {
        let start = Instant::now();

        let mut target_url = self.endpoint_url.trim_end_matches('/').to_string();
        if !target_url.ends_with("/chat/completions") {
            target_url.push_str("/chat/completions");
        }

        // Format user messages for Sarvam chat completion schema
        let messages: Vec<serde_json::Value> = if !req.messages.is_empty() {
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
            vec![serde_json::json!({
                "role": "user",
                "content": req.prompt,
            })]
        };

        let model = if req.model.trim().is_empty() || req.model == "default" {
            "sarvam-2b".to_string()
        } else {
            req.model.clone()
        };

        let mut payload = serde_json::json!({
            "model": model,
            "messages": messages,
            "max_tokens": req.max_tokens.unwrap_or(2048),
        });

        if let Some(temp) = req.temperature {
            payload["temperature"] = serde_json::json!(temp);
        }

        let mut http_req = self.client.post(&target_url).json(&payload);
        if let Some(key) = &self.api_key {
            http_req = http_req
                .header("api-subscription-key", key)
                .header("Authorization", format!("Bearer {key}"));
        }

        let resp = http_req.send().await.map_err(|e| {
            SunbornError::origin(format!(
                "Sarvam AI sovereign upstream connection error: {e}"
            ))
        })?;

        let status = resp.status();
        let body_text = resp
            .text()
            .await
            .map_err(|e| SunbornError::origin(format!("Failed to read Sarvam AI response: {e}")))?;

        if !status.is_success() {
            return Err(SunbornError::origin(format!(
                "Sarvam AI sovereign upstream returned HTTP {status}: {body_text}"
            )));
        }

        let parsed: serde_json::Value = serde_json::from_str(&body_text).map_err(|e| {
            SunbornError::origin(format!("Failed to parse Sarvam AI JSON response: {e}"))
        })?;

        let content = parsed["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string();

        let prompt_tokens = parsed["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32;
        let completion_tokens = parsed["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32;

        let latency_ms = start.elapsed().as_millis() as u64;

        Ok(OriginExecutionResult {
            content,
            prompt_tokens,
            completion_tokens,
            model,
            latency_ms,
            finish_reason: parsed["choices"][0]["finish_reason"]
                .as_str()
                .map(String::from),
            tool_calls: None,
        })
    }
}
