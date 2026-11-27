use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};
use sunborn_core::{RequestEnvelope, Result, SunbornError};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginExecutionResult {
    pub content: String,
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub model: String,
    pub latency_ms: u64,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub tool_calls: Option<Vec<sunborn_core::model::ToolCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OriginStreamChunk {
    pub delta_content: String,
    #[serde(default)]
    pub delta_tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub finish_reason: Option<String>,
    #[serde(default)]
    pub prompt_tokens: Option<u32>,
    #[serde(default)]
    pub completion_tokens: Option<u32>,
    #[serde(default)]
    pub model: Option<String>,
}

#[async_trait]
pub trait LlmOriginProvider: Send + Sync {
    async fn execute(&self, req: &RequestEnvelope) -> Result<OriginExecutionResult>;
    async fn execute_stream(
        &self,
        req: &RequestEnvelope,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<OriginStreamChunk>>> {
        let res = self.execute(req).await?;
        let (tx, rx) = tokio::sync::mpsc::channel(16);
        tokio::spawn(async move {
            let tool_calls_json = res.tool_calls.as_ref().map(|tcs| {
                tcs.iter()
                    .enumerate()
                    .map(|(idx, tc)| {
                        serde_json::json!({
                            "index": idx,
                            "id": tc.id,
                            "type": tc.r#type,
                            "function": {
                                "name": tc.function.name,
                                "arguments": tc.function.arguments,
                            }
                        })
                    })
                    .collect()
            });
            let _ = tx
                .send(Ok(OriginStreamChunk {
                    delta_content: res.content,
                    delta_tool_calls: tool_calls_json,
                    finish_reason: res.finish_reason.or_else(|| Some("stop".to_string())),
                    prompt_tokens: Some(res.prompt_tokens),
                    completion_tokens: Some(res.completion_tokens),
                    model: Some(res.model),
                }))
                .await;
        });
        Ok(rx)
    }
    fn is_mock(&self) -> bool {
        false
    }
    async fn health_check(&self) -> Result<()> {
        Ok(())
    }
}

/// Production HTTP client executing calls against OpenAI/Anthropic/vLLM HTTP endpoints.
pub struct HttpLlmOriginProvider {
    pub endpoint_url: String,
    pub api_key: Option<String>,
    pub timeout_secs: u64,
    pub max_retries: u32,
    client: reqwest::Client,
}

impl HttpLlmOriginProvider {
    pub fn new(endpoint_url: impl Into<String>, api_key: Option<String>) -> Self {
        let timeout_secs = 30;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(timeout_secs))
            .connect_timeout(Duration::from_secs(5))
            .pool_max_idle_per_host(20)
            .build()
            .unwrap_or_default();

        Self {
            endpoint_url: endpoint_url.into(),
            api_key,
            timeout_secs,
            max_retries: 2,
            client,
        }
    }

    pub fn with_client(
        endpoint_url: impl Into<String>,
        api_key: Option<String>,
        client: reqwest::Client,
    ) -> Self {
        Self {
            endpoint_url: endpoint_url.into(),
            api_key,
            timeout_secs: 30,
            max_retries: 2,
            client,
        }
    }

    /// Dedicated factory for xAI Grok API endpoints
    pub fn for_grok(api_key: Option<String>) -> Self {
        Self::new("https://api.x.ai/v1", api_key)
    }

    fn build_target_url(&self) -> String {
        let mut target_url = self.endpoint_url.trim_end_matches('/').to_string();
        if !target_url.ends_with("/chat/completions") {
            target_url.push_str("/chat/completions");
        }
        target_url
    }

    fn build_openai_payload(&self, req: &RequestEnvelope, stream: bool) -> serde_json::Value {
        let messages: Vec<serde_json::Value> = if !req.messages.is_empty() {
            req.messages
                .iter()
                .map(|m| {
                    let mut obj = serde_json::json!({
                        "role": m.role,
                        "content": m.content.clone().unwrap_or_default(),
                    });
                    if let Some(name) = &m.name {
                        obj["name"] = serde_json::Value::String(name.clone());
                    }
                    if let Some(tool_calls) = &m.tool_calls {
                        obj["tool_calls"] =
                            serde_json::to_value(tool_calls).unwrap_or(serde_json::Value::Null);
                    }
                    if let Some(tool_call_id) = &m.tool_call_id {
                        obj["tool_call_id"] = serde_json::Value::String(tool_call_id.clone());
                    }
                    obj
                })
                .collect()
        } else {
            let mut msgs = Vec::new();
            if let Some(sys) = &req.system_prompt {
                msgs.push(serde_json::json!({ "role": "system", "content": sys }));
            }
            msgs.push(serde_json::json!({ "role": "user", "content": req.prompt }));
            msgs
        };

        let mut payload = serde_json::json!({
            "model": req.model,
            "messages": messages,
        });
        // Preserve system-prefix caching synergy on OpenAI-compatible upstreams
        if let Some(sys) = &req.system_prompt {
            use sha2::{Digest, Sha256};
            let mut h = Sha256::new();
            h.update(req.tenant_id.0.as_bytes());
            h.update(b":");
            h.update(sys.as_bytes());
            let prefix_hash = hex::encode(h.finalize());
            payload["prompt_cache_key"] = serde_json::json!(format!("sb-{}", &prefix_hash[..24]));
        }

        if let Some(temp) = req.temperature {
            payload["temperature"] = serde_json::json!(temp);
        }
        if let Some(top_p) = req.top_p {
            payload["top_p"] = serde_json::json!(top_p);
        }
        if let Some(max_tokens) = req.max_tokens {
            payload["max_tokens"] = serde_json::json!(max_tokens);
        }
        if !req.stop_sequences.is_empty() {
            payload["stop"] = serde_json::json!(req.stop_sequences);
        }
        if !req.tools.is_empty() {
            let tools_json: Vec<serde_json::Value> = req
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters
                        }
                    })
                })
                .collect();
            payload["tools"] = serde_json::json!(tools_json);
        }
        if let Some(tc) = &req.tool_choice {
            payload["tool_choice"] = serde_json::json!(tc);
        }
        if let Some(rf) = &req.response_format {
            if let Ok(rf_val) = serde_json::from_str::<serde_json::Value>(rf) {
                payload["response_format"] = rf_val;
            } else {
                payload["response_format"] = serde_json::json!({ "type": rf });
            }
        }
        if stream {
            payload["stream"] = serde_json::json!(true);
            payload["stream_options"] = serde_json::json!({ "include_usage": true });
        }
        payload
    }
}

#[async_trait]
impl LlmOriginProvider for HttpLlmOriginProvider {
    async fn execute(&self, req: &RequestEnvelope) -> Result<OriginExecutionResult> {
        let start = Instant::now();
        let target_url = self.build_target_url();
        let payload = self.build_openai_payload(req, false);

        // 3. Dispatch HTTP request with retries and exponential backoff
        let mut last_error = None;
        let mut attempts = 0;

        while attempts <= self.max_retries {
            attempts += 1;

            let mut request_builder = self.client.post(&target_url).json(&payload);
            if let Some(key) = &self.api_key {
                if !key.is_empty() {
                    request_builder = request_builder.bearer_auth(key);
                }
            }

            match request_builder.send().await {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        let resp_body: serde_json::Value = resp.json().await.map_err(|e| {
                            SunbornError::origin(format!(
                                "Failed to parse JSON from LLM origin: {}",
                                e
                            ))
                        })?;

                        let content = resp_body
                            .pointer("/choices/0/message/content")
                            .and_then(|c| c.as_str())
                            .unwrap_or("")
                            .to_string();

                        let finish_reason = resp_body
                            .pointer("/choices/0/finish_reason")
                            .and_then(|f| f.as_str())
                            .map(|s| s.to_string());

                        let tool_calls: Option<Vec<sunborn_core::model::ToolCall>> = resp_body
                            .pointer("/choices/0/message/tool_calls")
                            .and_then(|tc| serde_json::from_value(tc.clone()).ok());

                        let prompt_tokens = resp_body
                            .pointer("/usage/prompt_tokens")
                            .and_then(|pt| pt.as_u64())
                            .unwrap_or_else(|| (req.prompt.len() / 4).max(1) as u64)
                            as u32;

                        let completion_tokens = resp_body
                            .pointer("/usage/completion_tokens")
                            .and_then(|ct| ct.as_u64())
                            .unwrap_or_else(|| (content.len() / 4).max(1) as u64)
                            as u32;

                        let latency_ms = start.elapsed().as_millis() as u64;

                        return Ok(OriginExecutionResult {
                            content,
                            prompt_tokens,
                            completion_tokens,
                            model: req.model.clone(),
                            latency_ms,
                            finish_reason,
                            tool_calls,
                        });
                    } else if status.as_u16() == 429 || status.is_server_error() {
                        let err_text = resp.text().await.unwrap_or_default();
                        last_error = Some(format!("Origin HTTP status {}: {}", status, err_text));
                        if attempts <= self.max_retries {
                            let backoff_ms = 50 * (1 << (attempts - 1));
                            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                            continue;
                        }
                    } else {
                        let err_text = resp.text().await.unwrap_or_default();
                        return Err(SunbornError::origin(format!(
                            "LLM Origin returned client error {}: {}",
                            status, err_text
                        )));
                    }
                }
                Err(err) => {
                    last_error = Some(format!("HTTP connection failed to {}: {}", target_url, err));
                    if attempts <= self.max_retries {
                        let backoff_ms = 50 * (1 << (attempts - 1));
                        tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                        continue;
                    }
                }
            }
        }

        Err(SunbornError::origin(last_error.unwrap_or_else(|| {
            "Origin execution failed after maximum retries".to_string()
        })))
    }

    async fn execute_stream(
        &self,
        req: &RequestEnvelope,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<OriginStreamChunk>>> {
        use tokio_stream::StreamExt;

        let target_url = self.build_target_url();
        let payload = self.build_openai_payload(req, true);
        let mut request_builder = self.client.post(&target_url).json(&payload);
        if let Some(key) = &self.api_key {
            if !key.is_empty() {
                request_builder = request_builder.bearer_auth(key);
            }
        }

        let resp = request_builder.send().await.map_err(|err| {
            SunbornError::origin(format!(
                "HTTP stream connection failed to {}: {}",
                target_url, err
            ))
        })?;
        let status = resp.status();
        if !status.is_success() {
            let err_text = resp.text().await.unwrap_or_default();
            return Err(SunbornError::origin(format!(
                "LLM Origin stream returned HTTP {}: {}",
                status, err_text
            )));
        }

        let (tx, rx) = tokio::sync::mpsc::channel(64);
        let fallback_model = req.model.clone();
        tokio::spawn(async move {
            let mut byte_stream = resp.bytes_stream();
            let mut line_buf = String::new();
            while let Some(item) = byte_stream.next().await {
                let bytes = match item {
                    Ok(b) => b,
                    Err(e) => {
                        let _ = tx
                            .send(Err(SunbornError::origin(format!(
                                "Upstream SSE stream read error: {}",
                                e
                            ))))
                            .await;
                        return;
                    }
                };
                line_buf.push_str(&String::from_utf8_lossy(&bytes));
                while let Some(pos) = line_buf.find('\n') {
                    let raw_line = line_buf[..pos].trim_end_matches('\r').to_string();
                    line_buf.drain(..=pos);
                    let trimmed = raw_line.trim();
                    if trimmed.is_empty() || trimmed.starts_with(':') {
                        continue;
                    }
                    let Some(data) = trimmed.strip_prefix("data:") else {
                        continue;
                    };
                    let data = data.trim();
                    if data == "[DONE]" {
                        return;
                    }
                    let Ok(val) = serde_json::from_str::<serde_json::Value>(data) else {
                        continue;
                    };
                    let delta_content = val
                        .pointer("/choices/0/delta/content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string();
                    let delta_tool_calls = val
                        .pointer("/choices/0/delta/tool_calls")
                        .and_then(|tc| tc.as_array())
                        .cloned();
                    let finish_reason = val
                        .pointer("/choices/0/finish_reason")
                        .and_then(|f| f.as_str())
                        .map(|s| s.to_string());
                    let prompt_tokens = val
                        .pointer("/usage/prompt_tokens")
                        .and_then(|u| u.as_u64())
                        .map(|u| u as u32);
                    let completion_tokens = val
                        .pointer("/usage/completion_tokens")
                        .and_then(|u| u.as_u64())
                        .map(|u| u as u32);
                    let model = val
                        .get("model")
                        .and_then(|m| m.as_str())
                        .map(|s| s.to_string())
                        .or_else(|| Some(fallback_model.clone()));

                    if delta_content.is_empty()
                        && delta_tool_calls.is_none()
                        && finish_reason.is_none()
                        && prompt_tokens.is_none()
                        && completion_tokens.is_none()
                    {
                        continue;
                    }

                    if tx
                        .send(Ok(OriginStreamChunk {
                            delta_content,
                            delta_tool_calls,
                            finish_reason,
                            prompt_tokens,
                            completion_tokens,
                            model,
                        }))
                        .await
                        .is_err()
                    {
                        return;
                    }
                }
            }
        });

        Ok(rx)
    }

    async fn health_check(&self) -> Result<()> {
        let base = self.endpoint_url.trim_end_matches('/');
        let url = if base.ends_with("/chat/completions") {
            base.replace("/chat/completions", "/models")
        } else if base.ends_with("/v1") {
            format!("{}/models", base)
        } else {
            base.to_string()
        };

        let mut req = self.client.get(&url);
        if let Some(key) = &self.api_key {
            if !key.is_empty() {
                req = req.bearer_auth(key);
            }
        }

        match req.send().await {
            Ok(resp)
                if resp.status().is_success()
                    || resp.status().as_u16() == 401
                    || resp.status().as_u16() == 404 =>
            {
                Ok(())
            }
            Ok(resp) => Err(SunbornError::origin(format!(
                "LLM Origin endpoint at {} returned error status: {}",
                url,
                resp.status()
            ))),
            Err(err) => Err(SunbornError::origin(format!(
                "Failed to connect to LLM Origin endpoint at {}: {}",
                url, err
            ))),
        }
    }
}

/// Deterministic mock provider for unit tests, offline CI, and benchmark suites
pub struct MockOriginProvider;

impl MockOriginProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MockOriginProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmOriginProvider for MockOriginProvider {
    fn is_mock(&self) -> bool {
        true
    }

    async fn execute(&self, req: &RequestEnvelope) -> Result<OriginExecutionResult> {
        let prompt_tokens = (req.prompt.split_whitespace().count() * 2).max(1) as u32;
        let content = format!(
            "[Origin Response from {} for tenant {}]: {}",
            req.model, req.tenant_id.0, req.prompt
        );
        let completion_tokens = (content.split_whitespace().count() * 2).max(1) as u32;

        Ok(OriginExecutionResult {
            content,
            prompt_tokens,
            completion_tokens,
            model: req.model.clone(),
            latency_ms: 10,
            finish_reason: Some("stop".to_string()),
            tool_calls: None,
        })
    }
}

use std::sync::Arc;

/// Upstream provider entry in the dynamic spot arbitrage router
#[derive(Clone)]
pub struct ArbitrageProviderEntry {
    pub name: String,
    pub provider: Arc<dyn LlmOriginProvider>,
    pub cost_per_million_input: f64,
    pub cost_per_million_output: f64,
    pub priority: u32,
}

/// Dynamic Multi-Provider Spot Arbitrage & Failover Router.
/// Automatically executes requests against the highest-priority/lowest-cost healthy upstream provider,
/// falling over to secondary and tertiary providers in real-time on HTTP 429, 5xx, or network errors.
pub struct MultiProviderArbitrageRouter {
    providers: Vec<ArbitrageProviderEntry>,
    fallback_on_error: bool,
}

impl MultiProviderArbitrageRouter {
    pub fn new(mut providers: Vec<ArbitrageProviderEntry>) -> Self {
        providers.sort_by_key(|p| p.priority);
        Self {
            providers,
            fallback_on_error: true,
        }
    }

    pub fn providers(&self) -> &[ArbitrageProviderEntry] {
        &self.providers
    }

    fn order_providers_for_model(&self, model: &str) -> Vec<ArbitrageProviderEntry> {
        let mut ordered_providers = self.providers.clone();
        if model.starts_with("claude-") {
            ordered_providers.sort_by_key(|p| {
                if p.name.to_lowercase().contains("anthropic")
                    || p.name.to_lowercase().contains("claude")
                {
                    0
                } else {
                    p.priority + 1
                }
            });
        } else if model.starts_with("gemini-") {
            ordered_providers.sort_by_key(|p| {
                if p.name.to_lowercase().contains("gemini")
                    || p.name.to_lowercase().contains("google")
                {
                    0
                } else {
                    p.priority + 1
                }
            });
        } else if model.starts_with("gpt-") || model.starts_with("o1") || model.starts_with("o3") {
            ordered_providers.sort_by_key(|p| {
                if p.name.to_lowercase().contains("openai") {
                    0
                } else {
                    p.priority + 1
                }
            });
        } else if model.starts_with("grok-") {
            ordered_providers.sort_by_key(|p| {
                if p.name.to_lowercase().contains("grok") || p.name.to_lowercase().contains("xai") {
                    0
                } else {
                    p.priority + 1
                }
            });
        }
        ordered_providers
    }
}

#[async_trait]
impl LlmOriginProvider for MultiProviderArbitrageRouter {
    fn is_mock(&self) -> bool {
        self.providers.iter().all(|p| p.provider.is_mock())
    }

    async fn execute(&self, req: &RequestEnvelope) -> Result<OriginExecutionResult> {
        if self.providers.is_empty() {
            return Err(SunbornError::origin(
                "No upstream LLM providers configured in arbitrage router",
            ));
        }

        let ordered_providers = self.order_providers_for_model(&req.model);

        let mut last_error = None;
        for entry in &ordered_providers {
            match entry.provider.execute(req).await {
                Ok(res) => {
                    tracing::info!(
                        provider = %entry.name,
                        model = %res.model,
                        latency_ms = res.latency_ms,
                        "Arbitrage router successfully executed query"
                    );
                    return Ok(res);
                }
                Err(err) => {
                    tracing::warn!(
                        provider = %entry.name,
                        error = %err,
                        "Primary provider execution failed; cascading to next arbitrage provider"
                    );
                    last_error = Some(err);
                    if !self.fallback_on_error {
                        break;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| SunbornError::origin("All arbitrage providers failed")))
    }

    async fn execute_stream(
        &self,
        req: &RequestEnvelope,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<OriginStreamChunk>>> {
        if self.providers.is_empty() {
            return Err(SunbornError::origin(
                "No upstream LLM providers configured in arbitrage router",
            ));
        }

        let ordered_providers = self.order_providers_for_model(&req.model);
        let mut last_error = None;
        for entry in &ordered_providers {
            match entry.provider.execute_stream(req).await {
                Ok(rx) => {
                    tracing::info!(
                        provider = %entry.name,
                        model = %req.model,
                        "Arbitrage router opened pass-through stream"
                    );
                    return Ok(rx);
                }
                Err(err) => {
                    tracing::warn!(
                        provider = %entry.name,
                        error = %err,
                        "Primary provider stream initiation failed; cascading to next arbitrage provider"
                    );
                    last_error = Some(err);
                    if !self.fallback_on_error {
                        break;
                    }
                }
            }
        }

        Err(last_error
            .unwrap_or_else(|| SunbornError::origin("All arbitrage providers failed stream init")))
    }

    async fn health_check(&self) -> Result<()> {
        if self.providers.is_empty() {
            return Err(SunbornError::origin("No providers in arbitrage router"));
        }
        let mut any_healthy = false;
        let mut last_err = None;
        for p in &self.providers {
            match p.provider.health_check().await {
                Ok(()) => {
                    any_healthy = true;
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                }
            }
        }

        if any_healthy {
            Ok(())
        } else {
            Err(last_err
                .unwrap_or_else(|| SunbornError::origin("All providers failed healthcheck")))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingProvider;
    #[async_trait]
    impl LlmOriginProvider for FailingProvider {
        async fn execute(&self, _req: &RequestEnvelope) -> Result<OriginExecutionResult> {
            Err(SunbornError::origin(
                "Upstream HTTP 429 rate limit exceeded",
            ))
        }
        async fn health_check(&self) -> Result<()> {
            Err(SunbornError::origin("Service unavailable"))
        }
    }

    #[tokio::test]
    async fn test_multi_provider_arbitrage_failover() {
        let failing = Arc::new(FailingProvider);
        let healthy = Arc::new(MockOriginProvider::new());

        let router = MultiProviderArbitrageRouter::new(vec![
            ArbitrageProviderEntry {
                name: "primary-openai".into(),
                provider: failing,
                cost_per_million_input: 2.50,
                cost_per_million_output: 10.00,
                priority: 1,
            },
            ArbitrageProviderEntry {
                name: "secondary-mock".into(),
                provider: healthy,
                cost_per_million_input: 3.00,
                cost_per_million_output: 15.00,
                priority: 2,
            },
        ]);

        let req = RequestEnvelope::new(
            "test-tenant",
            "test-app",
            "gpt-4o",
            "What is quantum error correction?",
        );

        // Primary fails, router cascades seamlessly to secondary
        let res = router
            .execute(&req)
            .await
            .expect("Arbitrage failover should succeed");
        assert!(res.content.contains("quantum error correction"));
        assert!(router.health_check().await.is_ok());
    }
}
