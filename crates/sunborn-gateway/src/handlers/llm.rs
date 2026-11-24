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

    let prompt_tokens: u32 = chat_req
        .messages
        .iter()
        .map(|m| estimate_tokens(&m.content))
        .sum::<u32>()
        .max(1);
    let completion_tokens = estimate_tokens(&resp_envelope.content).max(1);

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
