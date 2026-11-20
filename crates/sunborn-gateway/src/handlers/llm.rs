//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
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
) -> Result<Json<ChatCompletionResponse>, AppError> {
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
        .map(|m| sunborn_core::model::ChatMessage::new(&m.role, &m.content))
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
            if let Some(name) = t
                .get("function")
                .and_then(|f| f.get("name"))
                .and_then(|n| n.as_str())
            {
                let desc = t
                    .get("function")
                    .and_then(|f| f.get("description"))
                    .and_then(|d| d.as_str())
                    .map(|s| s.to_string());
                let params = t
                    .get("function")
                    .and_then(|f| f.get("parameters"))
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
    if let Some(rf) = chat_req.response_format {
        req.response_format = Some(rf.to_string());
    }

    let resp_envelope = execute_request_inner(&state, &sec_ctx, req).await?;

    let prompt_total_chars: usize = chat_req.messages.iter().map(|m| m.content.len()).sum();
    let prompt_tokens = (prompt_total_chars / 4).max(1) as u32;
    let completion_tokens = (resp_envelope.content.len() / 4).max(1) as u32;

    let tier_str = if resp_envelope.cached {
        match resp_envelope.decision.cache_path {
            CachePath::Exact => "c1_exact".to_string(),
            CachePath::Semantic => "c2_semantic".to_string(),
            _ => "cached".to_string(),
        }
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
            },
            finish_reason: "stop".to_string(),
        }],
        usage: ChatUsage {
            prompt_tokens,
            completion_tokens,
            total_tokens: prompt_tokens + completion_tokens,
        },
        sunborn_cached: resp_envelope.cached,
        sunborn_tier: tier_str,
    }))
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

    let prompt_total_chars: usize = anthropic_req
        .messages
        .iter()
        .map(|m| m.content.to_string().len())
        .sum();
    let input_tokens = (prompt_total_chars / 4).max(1) as u32;
    let output_tokens = (resp_envelope.content.len() / 4).max(1) as u32;

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
