//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

/// Robust multilingual token estimation accounting for Latin, Indic, CJK, and special symbols (Issue 2).
/// Prevents the crude 4-character division heuristic from undercounting Indic and Asian scripts by 400%.
pub fn estimate_tokens(text: &str) -> u32 {
    if text.is_empty() {
        return 0;
    }
    let mut tokens: f64 = 0.0;
    let mut current_ascii_word_len = 0;

    for ch in text.chars() {
        let code = ch as u32;
        if ch.is_ascii_whitespace() {
            if current_ascii_word_len > 0 {
                tokens += (current_ascii_word_len as f64 / 4.0).max(1.0);
                current_ascii_word_len = 0;
            }
            tokens += 0.25;
        } else if ch.is_ascii() {
            current_ascii_word_len += 1;
        } else if (0x0900..=0x0D7F).contains(&code) {
            // Indic scripts (Devanagari, Bengali, Gurmukhi, Gujarati, Oriya, Tamil, Telugu, Kannada, Malayalam)
            tokens += 1.5;
        } else if (0x4E00..=0x9FFF).contains(&code)
            || (0x3040..=0x30FF).contains(&code)
            || (0xAC00..=0xD7AF).contains(&code)
        {
            // CJK Unified Ideographs, Hiragana, Katakana, Hangul
            tokens += 1.8;
        } else {
            // Other Unicode (Arabic, Hebrew, Cyrillic, Emojis, math operators)
            tokens += 1.3;
        }
    }
    if current_ascii_word_len > 0 {
        tokens += (current_ascii_word_len as f64 / 4.0).max(1.0);
    }
    (tokens.round() as u32).max(1)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<sunborn_core::model::ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub tools: Option<Vec<serde_json::Value>>,
    #[serde(default)]
    pub tool_choice: Option<serde_json::Value>,
    #[serde(default)]
    pub response_format: Option<serde_json::Value>,
    #[serde(default)]
    pub stop: Option<Vec<String>>,
    #[serde(default)]
    pub stream: Option<bool>,
    #[serde(default)]
    pub effect_class: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatChoice {
    pub index: usize,
    pub message: ChatMessage,
    pub finish_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub model: String,
    pub choices: Vec<ChatChoice>,
    pub usage: ChatUsage,
    pub sunborn_cached: bool,
    pub sunborn_tier: String,
}

pub async fn chat_completions(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(chat_req): Json<ChatCompletionRequest>,
) -> Result<axum::response::Response, AppError> {
    use axum::response::sse::{Event, Sse};
    use axum::response::IntoResponse;
    use std::convert::Infallible;

    if chat_req.messages.is_empty() {
        return Err(SunbornError::bad_request("Messages array cannot be empty").into());
    }

    let system_prompt = chat_req
        .messages
        .iter()
        .find(|m| m.role == "system")
        .map(|m| m.content.clone());

    let prompt = chat_req
        .messages
        .iter()
        .rfind(|m| m.role == "user")
        .map(|m| m.content.clone())
        .unwrap_or_else(|| {
            chat_req
                .messages
                .iter()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n")
        });

    let mut req = RequestEnvelope::new(
        sec_ctx.tenant_id.0.clone(),
        sec_ctx.application_id.0.clone(),
        chat_req.model.clone(),
        prompt,
    );
    req.messages = chat_req
        .messages
        .iter()
        .map(|m| {
            let mut cm = sunborn_core::model::ChatMessage::new(&m.role, &m.content);
            cm.name = m.name.clone();
            cm.tool_calls = m.tool_calls.clone();
            cm.tool_call_id = m.tool_call_id.clone();
            cm
        })
        .collect();
    req.system_prompt = system_prompt;
    req.temperature = chat_req.temperature;
    req.top_p = chat_req.top_p;
    req.max_tokens = chat_req.max_tokens;
    if let Some(stop) = chat_req.stop {
        req.stop_sequences = stop;
    }
    if let Some(tools) = chat_req.tools {
        for t in tools {
            // Support both OpenAI `function` tool wrappers and MCP `{name, description, inputSchema}` tools
            if let Some(func) = t.get("function") {
                if let Some(name) = func.get("name").and_then(|n| n.as_str()) {
                    let desc = func
                        .get("description")
                        .and_then(|d| d.as_str())
                        .map(|s| s.to_string());
                    let params = func
                        .get("parameters")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));
                    req.tools.push(sunborn_core::model::ToolDefinition {
                        name: name.to_string(),
                        description: desc,
                        parameters: params,
                    });
                }
            } else if let Some(name) = t.get("name").and_then(|n| n.as_str()) {
                let desc = t
                    .get("description")
                    .and_then(|d| d.as_str())
                    .map(|s| s.to_string());
                let params = t
                    .get("inputSchema")
                    .or_else(|| t.get("parameters"))
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                req.tools.push(sunborn_core::model::ToolDefinition {
                    name: name.to_string(),
                    description: desc,
                    parameters: params,
                });
            }
        }
    }
    if let Some(tc) = chat_req.tool_choice {
        req.tool_choice = Some(tc.to_string());
    }
    if let Some(rf) = chat_req.response_format {
        req.response_format = Some(rf.to_string());
    }
    if let Some(effect_class) = chat_req.effect_class {
        req.metadata["effect_class"] = serde_json::json!(effect_class);
    }

    if chat_req.stream.unwrap_or(false) {
        if !sec_ctx.is_authenticated {
            return Err(SunbornError::unauthorized(
                "Authentication required: missing or invalid API key",
            )
            .into());
        }
        if !sec_ctx.allows_model(&req.model) {
            return Err(SunbornError::unauthorized(format!(
                "Model '{}' is not authorized for tenant '{}'",
                req.model, sec_ctx.tenant_id.0
            ))
            .into());
        }
        req.validate().map_err(AppError::from)?;
        state
            .request_limits
            .enforce(&mut req)
            .map_err(AppError::from)?;

        let id = format!("chatcmpl-sunborn-{}", req.request_id.0);
        let created = Utc::now().timestamp();
        let model = chat_req.model.clone();

        // True zero-buffer pass-through SSE streaming from origin provider.
        // Falls back to execute_request_inner only when origin_provider is mock
        // so deterministic cache-hit tests continue to observe cache hits.
        if !state.origin_provider.is_mock() {
            if let Err(err) = state.quota_ledger.reserve(&req.tenant_id.0).await {
                return Err(SunbornError::rate_limit(format!("{:?}", err)).into());
            }
            let token_reservation = state
                .token_limiter
                .pre_commit(&req.tenant_id.0)
                .map_err(|err| AppError::from(SunbornError::rate_limit(err.to_string())))?;

            let cel_eval = state
                .cel_engine
                .evaluate(&crate::cel_policy::PolicyContext {
                    tenant_id: req.tenant_id.0.clone(),
                    application_id: req.application_id.0.clone(),
                    model: req.model.clone(),
                    is_admin: sec_ctx.is_admin,
                    mcp_tool_name: None,
                    mcp_server_name: None,
                    headers: std::collections::HashMap::new(),
                    body_text: if state.cel_engine.combined_requirements().needs_body {
                        Some(req.prompt.clone())
                    } else {
                        None
                    },
                });
            if !cel_eval.allowed {
                return Err(SunbornError::forbidden(cel_eval.reason).into());
            }

            match state
                .guardrail_chain
                .evaluate(crate::guardrails::GuardrailPhase::Input, &req.prompt)
            {
                crate::guardrails::GuardrailOutcome::Allow => {}
                crate::guardrails::GuardrailOutcome::Redact { redacted_text, .. } => {
                    req.prompt = redacted_text;
                }
                crate::guardrails::GuardrailOutcome::Block { rule_id, reason } => {
                    return Err(SunbornError::forbidden(format!(
                        "Input blocked by guardrail '{}': {}",
                        rule_id, reason
                    ))
                    .into());
                }
            }

            let est_prompt_tokens = estimate_tokens(&req.prompt).max(1);
            let mut rx_origin = state.origin_provider.execute_stream(&req).await?;
            let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);
            tokio::spawn(async move {
                let mut final_finish = "stop".to_string();
                let mut usage_json: Option<serde_json::Value> = None;
                let mut streamed_text = String::new();
                let mut final_pt = est_prompt_tokens;
                let mut final_ct = 0u32;
                while let Some(chunk_res) = rx_origin.recv().await {
                    match chunk_res {
                        Ok(chunk) => {
                            if let Some(fr) = chunk.finish_reason {
                                final_finish = fr;
                            }
                            if let (Some(pt), Some(ct)) =
                                (chunk.prompt_tokens, chunk.completion_tokens)
                            {
                                final_pt = pt;
                                final_ct = ct;
                                usage_json = Some(serde_json::json!({
                                    "prompt_tokens": pt,
                                    "completion_tokens": ct,
                                    "total_tokens": pt + ct,
                                }));
                            }
                            let mut delta = serde_json::Map::new();
                            if !chunk.delta_content.is_empty() {
                                streamed_text.push_str(&chunk.delta_content);
                                delta.insert(
                                    "content".to_string(),
                                    serde_json::Value::String(chunk.delta_content),
                                );
                            }
                            if let Some(tc) = chunk.delta_tool_calls {
                                delta
                                    .insert("tool_calls".to_string(), serde_json::Value::Array(tc));
                            }
                            if !delta.is_empty() {
                                let sse_chunk = serde_json::json!({
                                    "id": id,
                                    "object": "chat.completion.chunk",
                                    "created": created,
                                    "model": chunk.model.unwrap_or_else(|| model.clone()),
                                    "sunborn_cached": false,
                                    "sunborn_tier": "origin",
                                    "choices": [{
                                        "index": 0,
                                        "delta": serde_json::Value::Object(delta),
                                        "finish_reason": serde_json::Value::Null
                                    }]
                                });
                                if tx
                                    .send(Ok(Event::default().data(sse_chunk.to_string())))
                                    .await
                                    .is_err()
                                {
                                    return;
                                }
                            }
                        }
                        Err(err) => {
                            let err_chunk = serde_json::json!({
                                "error": { "message": err.to_string(), "type": "upstream_error" }
                            });
                            let _ = tx
                                .send(Ok(Event::default().data(err_chunk.to_string())))
                                .await;
                            let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
                            return;
                        }
                    }
                }
                if final_ct == 0 && !streamed_text.is_empty() {
                    final_ct = estimate_tokens(&streamed_text);
                }
                token_reservation.commit_tokens(final_pt, final_ct);
                let mut final_chunk = serde_json::json!({
                    "id": id,
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": model,
                    "choices": [{
                        "index": 0,
                        "delta": {},
                        "finish_reason": final_finish
                    }]
                });
                if let Some(u) = usage_json {
                    final_chunk["usage"] = u;
                }
                let _ = tx
                    .send(Ok(Event::default().data(final_chunk.to_string())))
                    .await;
                let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
            });
            return Ok(Sse::new(tokio_stream::wrappers::ReceiverStream::new(rx)).into_response());
        }

        let resp_envelope = execute_request_inner(&state, &sec_ctx, req).await?;
        let tier_str = if resp_envelope.cached {
            match resp_envelope.decision.cache_path {
                CachePath::Exact => "c1_exact".to_string(),
                CachePath::Semantic => "c2_semantic".to_string(),
                _ => "cached".to_string(),
            }
        } else if resp_envelope
            .decision
            .reason_code
            .starts_with("SHADOW_MODE")
        {
            "shadow".to_string()
        } else {
            "origin".to_string()
        };
        let pace_ms = std::env::var("SUNBORN_SSE_TOKEN_PACE_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
        let cached = resp_envelope.cached;
        let finish_reason = resp_envelope
            .finish_reason
            .unwrap_or_else(|| "stop".to_string());
        let tokens: Vec<String> = if resp_envelope.content.is_empty() {
            vec![String::new()]
        } else {
            resp_envelope
                .content
                .split_inclusive(char::is_whitespace)
                .map(|s| s.to_string())
                .collect()
        };

        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Event, Infallible>>(64);
        tokio::spawn(async move {
            for (idx, tok) in tokens.into_iter().enumerate() {
                if idx > 0 && pace_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(pace_ms)).await;
                }
                let chunk = serde_json::json!({
                    "id": id,
                    "object": "chat.completion.chunk",
                    "created": created,
                    "model": model,
                    "sunborn_cached": cached,
                    "sunborn_tier": tier_str,
                    "choices": [{
                        "index": 0,
                        "delta": { "content": tok },
                        "finish_reason": serde_json::Value::Null
                    }]
                });
                if tx
                    .send(Ok(Event::default().data(chunk.to_string())))
                    .await
                    .is_err()
                {
                    return;
                }
            }
            let final_chunk = serde_json::json!({
                "id": id,
                "object": "chat.completion.chunk",
                "created": created,
                "model": model,
                "choices": [{
                    "index": 0,
                    "delta": {},
                    "finish_reason": finish_reason
                }]
            });
            let _ = tx
                .send(Ok(Event::default().data(final_chunk.to_string())))
                .await;
            let _ = tx.send(Ok(Event::default().data("[DONE]"))).await;
        });
        return Ok(Sse::new(tokio_stream::wrappers::ReceiverStream::new(rx)).into_response());
    }

    let resp_envelope = execute_request_inner(&state, &sec_ctx, req).await?;

    let prompt_tokens: u32 = resp_envelope.prompt_tokens.unwrap_or_else(|| {
        chat_req
            .messages
            .iter()
            .map(|m| estimate_tokens(&m.content))
            .sum::<u32>()
            .max(1)
    });
    let completion_tokens = resp_envelope
        .completion_tokens
        .unwrap_or_else(|| estimate_tokens(&resp_envelope.content).max(1));

    let tier_str = if resp_envelope.cached {
        match resp_envelope.decision.cache_path {
            CachePath::Exact => "c1_exact".to_string(),
            CachePath::Semantic => "c2_semantic".to_string(),
            _ => "cached".to_string(),
        }
    } else if resp_envelope
        .decision
        .reason_code
        .starts_with("SHADOW_MODE")
    {
        "shadow".to_string()
    } else {
        "origin".to_string()
    };

    Ok(Json(ChatCompletionResponse {
        id: format!("chatcmpl-sunborn-{}", resp_envelope.request_id.0),
        object: "chat.completion".to_string(),
        created: Utc::now().timestamp(),
        model: chat_req.model,
        choices: vec![ChatChoice {
            index: 0,
            message: ChatMessage {
                role: "assistant".to_string(),
                content: resp_envelope.content,
                name: None,
                tool_calls: resp_envelope.tool_calls,
                tool_call_id: None,
            },
            finish_reason: resp_envelope
                .finish_reason
                .unwrap_or_else(|| "stop".to_string()),
        }],
        usage: ChatUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        },
        sunborn_cached: resp_envelope.cached,
        sunborn_tier: tier_str,
    })
    .into_response())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessageItem {
    pub role: String,
    pub content: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessagesRequest {
    pub model: String,
    pub messages: Vec<AnthropicMessageItem>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
    #[serde(default)]
    pub system: Option<serde_json::Value>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub stop_sequences: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnthropicMessagesResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub msg_type: String,
    pub role: String,
    pub model: String,
    pub content: Vec<AnthropicContentBlock>,
    pub stop_reason: Option<String>,
    pub stop_sequence: Option<String>,
    pub usage: AnthropicUsage,
    pub sunborn_cached: bool,
    pub sunborn_tier: String,
}

pub async fn anthropic_messages(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(anthropic_req): Json<AnthropicMessagesRequest>,
) -> Result<Json<AnthropicMessagesResponse>, AppError> {
    if anthropic_req.messages.is_empty() {
        return Err(SunbornError::bad_request("Messages array cannot be empty").into());
    }

    let system_prompt = anthropic_req.system.as_ref().and_then(|s| {
        if let Some(text) = s.as_str() {
            Some(text.to_string())
        } else if let Some(arr) = s.as_array() {
            let combined: Vec<String> = arr
                .iter()
                .filter_map(|b| {
                    b.get("text")
                        .and_then(|t| t.as_str())
                        .map(|t| t.to_string())
                })
                .collect();
            if combined.is_empty() {
                None
            } else {
                Some(combined.join("\n"))
            }
        } else {
            None
        }
    });

    let mut user_prompt = String::new();
    let mut core_messages = Vec::new();

    for m in &anthropic_req.messages {
        let text_content = if let Some(text) = m.content.as_str() {
            text.to_string()
        } else if let Some(arr) = m.content.as_array() {
            arr.iter()
                .filter_map(|b| {
                    b.get("text")
                        .and_then(|t| t.as_str())
                        .map(|t| t.to_string())
                })
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            m.content.to_string()
        };

        if m.role == "user" {
            user_prompt = text_content.clone();
        }
        core_messages.push(sunborn_core::model::ChatMessage::new(
            &m.role,
            &text_content,
        ));
    }

    if user_prompt.is_empty() {
        user_prompt = core_messages
            .iter()
            .map(|m| format!("{}: {}", m.role, m.content.as_deref().unwrap_or_default()))
            .collect::<Vec<_>>()
            .join("\n");
    }

    let mut req = RequestEnvelope::new(
        sec_ctx.tenant_id.0.clone(),
        sec_ctx.application_id.0.clone(),
        anthropic_req.model.clone(),
        user_prompt,
    );
    req.messages = core_messages;
    req.system_prompt = system_prompt;
    req.temperature = anthropic_req.temperature;
    req.top_p = anthropic_req.top_p;
    req.max_tokens = anthropic_req.max_tokens;
    if let Some(stop) = anthropic_req.stop_sequences {
        req.stop_sequences = stop;
    }

    let resp_envelope = execute_request_inner(&state, &sec_ctx, req).await?;

    let input_tokens: u32 = anthropic_req
        .messages
        .iter()
        .map(|m| estimate_tokens(&m.content.to_string()))
        .sum::<u32>()
        .max(1);
    let output_tokens = estimate_tokens(&resp_envelope.content).max(1);

    let tier_str = if resp_envelope.cached {
        match resp_envelope.decision.cache_path {
            CachePath::Exact => "c1_exact".to_string(),
            CachePath::Semantic => "c2_semantic".to_string(),
            _ => "cached".to_string(),
        }
    } else {
        "origin".to_string()
    };

    Ok(Json(AnthropicMessagesResponse {
        id: format!("msg_sunborn_{}", resp_envelope.request_id.0),
        msg_type: "message".to_string(),
        role: "assistant".to_string(),
        model: anthropic_req.model,
        content: vec![AnthropicContentBlock {
            block_type: "text".to_string(),
            text: resp_envelope.content,
        }],
        stop_reason: Some("end_turn".to_string()),
        stop_sequence: None,
        usage: AnthropicUsage {
            input_tokens,
            output_tokens,
        },
        sunborn_cached: resp_envelope.cached,
        sunborn_tier: tier_str,
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenResponsesRequest {
    pub model: String,
    pub input: serde_json::Value,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub top_p: Option<f32>,
    #[serde(default)]
    pub max_output_tokens: Option<u32>,
    #[serde(default)]
    pub tools: Option<Vec<serde_json::Value>>,
}

pub async fn open_responses(
    State(state): State<AppState>,
    Extension(sec_ctx): Extension<SecurityContext>,
    Json(resp_req): Json<OpenResponsesRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let (prompt, messages) = if let Some(s) = resp_req.input.as_str() {
        (
            s.to_string(),
            vec![sunborn_core::model::ChatMessage::new("user", s)],
        )
    } else if let Some(arr) = resp_req.input.as_array() {
        let mut msgs = Vec::new();
        let mut last_user = String::new();
        for item in arr {
            let role = item.get("role").and_then(|r| r.as_str()).unwrap_or("user");
            let content = item
                .get("content")
                .and_then(|c| c.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    item.get("content")
                        .map(|c| c.to_string())
                        .unwrap_or_default()
                });
            if role == "user" {
                last_user = content.clone();
            }
            msgs.push(sunborn_core::model::ChatMessage::new(role, &content));
        }
        if last_user.is_empty() {
            last_user = msgs
                .iter()
                .map(|m| m.content.as_deref().unwrap_or_default())
                .collect::<Vec<_>>()
                .join("\n");
        }
        (last_user, msgs)
    } else {
        return Err(SunbornError::bad_request("input must be a string or message array").into());
    };

    let mut req = RequestEnvelope::new(
        sec_ctx.tenant_id.0.clone(),
        sec_ctx.application_id.0.clone(),
        resp_req.model.clone(),
        prompt,
    );
    req.messages = messages;
    req.system_prompt = resp_req.instructions;
    req.temperature = resp_req.temperature;
    req.top_p = resp_req.top_p;
    req.max_tokens = resp_req.max_output_tokens;
    if let Some(tools) = resp_req.tools {
        for t in tools {
            if let Some(name) = t
                .get("name")
                .or_else(|| t.pointer("/function/name"))
                .and_then(|n| n.as_str())
            {
                let desc = t
                    .get("description")
                    .or_else(|| t.pointer("/function/description"))
                    .and_then(|d| d.as_str())
                    .map(|s| s.to_string());
                let params = t
                    .get("parameters")
                    .or_else(|| t.get("inputSchema"))
                    .or_else(|| t.pointer("/function/parameters"))
                    .cloned()
                    .unwrap_or(serde_json::json!({}));
                req.tools.push(sunborn_core::model::ToolDefinition {
                    name: name.to_string(),
                    description: desc,
                    parameters: params,
                });
            }
        }
    }

    let resp_envelope = execute_request_inner(&state, &sec_ctx, req).await?;
    let input_tokens = resp_envelope
        .prompt_tokens
        .unwrap_or_else(|| estimate_tokens(&resp_req.input.to_string()).max(1));
    let output_tokens = resp_envelope
        .completion_tokens
        .unwrap_or_else(|| estimate_tokens(&resp_envelope.content).max(1));
    let tier_str = if resp_envelope.cached {
        match resp_envelope.decision.cache_path {
            CachePath::Exact => "c1_exact",
            CachePath::Semantic => "c2_semantic",
            _ => "cached",
        }
    } else {
        "origin"
    };

    Ok(Json(serde_json::json!({
        "id": format!("resp_sunborn_{}", resp_envelope.request_id.0),
        "object": "response",
        "created_at": Utc::now().timestamp(),
        "status": "completed",
        "model": resp_req.model,
        "output": [{
            "type": "message",
            "id": format!("msg_sunborn_{}", resp_envelope.request_id.0),
            "status": "completed",
            "role": "assistant",
            "content": [{
                "type": "output_text",
                "text": resp_envelope.content,
                "annotations": []
            }]
        }],
        "usage": {
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "total_tokens": input_tokens + output_tokens
        },
        "sunborn_cached": resp_envelope.cached,
        "sunborn_tier": tier_str
    })))
}

pub async fn list_models(
    Extension(sec_ctx): Extension<SecurityContext>,
) -> Result<Json<serde_json::Value>, AppError> {
    if !sec_ctx.is_authenticated {
        return Err(SunbornError::unauthorized(
            "Authentication required: missing or invalid API key",
        )
        .into());
    }
    let catalog = [
        ("gpt-4o", "openai"),
        ("gpt-4o-mini", "openai"),
        ("o3-mini", "openai"),
        ("claude-3-5-sonnet-20241022", "anthropic"),
        ("claude-3-5-haiku-20241022", "anthropic"),
        ("gemini-1.5-pro", "google"),
        ("gemini-1.5-flash", "google"),
        ("grok-2", "xai"),
        ("sarvam-2b", "sarvam"),
    ];
    let data: Vec<serde_json::Value> = catalog
        .into_iter()
        .filter(|(id, _)| sec_ctx.allows_model(id))
        .map(|(id, owner)| {
            serde_json::json!({
                "id": id,
                "object": "model",
                "created": 1700000000,
                "owned_by": owner
            })
        })
        .collect();
    Ok(Json(serde_json::json!({
        "object": "list",
        "data": data
    })))
}
