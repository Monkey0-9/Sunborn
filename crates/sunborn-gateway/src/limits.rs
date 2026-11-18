//! Request-size and completion-token caps applied before admission.
//!
//! These are server-side limits, not client hints. They bound the worst-case
//! origin cost of a free-tier request and prevent a caller from forwarding an
//! arbitrarily large completion budget.

use sunborn_core::{RequestEnvelope, SunbornError};

const DEFAULT_MAX_PROMPT_CHARS: usize = 32_000;
const DEFAULT_MAX_COMPLETION_TOKENS: u32 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestLimits {
    pub max_prompt_chars: usize,
    pub max_completion_tokens: u32,
}

impl Default for RequestLimits {
    fn default() -> Self {
        Self {
            max_prompt_chars: DEFAULT_MAX_PROMPT_CHARS,
            max_completion_tokens: DEFAULT_MAX_COMPLETION_TOKENS,
        }
    }
}

impl RequestLimits {
    pub fn from_env() -> Self {
        let max_prompt_chars = std::env::var("SUNBORN_MAX_PROMPT_CHARS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .unwrap_or(DEFAULT_MAX_PROMPT_CHARS);
        let max_completion_tokens = std::env::var("SUNBORN_MAX_COMPLETION_TOKENS")
            .ok()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(DEFAULT_MAX_COMPLETION_TOKENS);
        Self {
            max_prompt_chars,
            max_completion_tokens,
        }
    }

    pub fn new(max_prompt_chars: usize, max_completion_tokens: u32) -> Self {
        Self {
            max_prompt_chars,
            max_completion_tokens,
        }
    }

    /// Enforces the prompt-size cap in place.
    ///
    /// A zero value disables the cap. Completion length is applied to the
    /// outbound origin clone by [`Self::apply_completion_cap`], not to the
    /// request used for cache identity.
    pub fn enforce(&self, req: &mut RequestEnvelope) -> Result<(), SunbornError> {
        if self.max_prompt_chars > 0 {
            let message_chars: usize = req
                .messages
                .iter()
                .map(|message| message.content.as_deref().unwrap_or_default().len())
                .sum();
            let content_chars = req.prompt.len().max(message_chars);
            let system_chars = req
                .system_prompt
                .as_deref()
                .map(str::len)
                .unwrap_or_default();
            let total_chars = content_chars.saturating_add(system_chars);
            if total_chars > self.max_prompt_chars {
                return Err(SunbornError::bad_request(format!(
                    "Request exceeds the maximum prompt size of {} characters (including messages and system prompt)",
                    self.max_prompt_chars
                )));
            }
        }
        Ok(())
    }

    /// Applies the completion cap to the outbound origin request.
    ///
    /// A missing budget receives the cap, so an origin cannot choose a larger
    /// default by omission.
    pub fn apply_completion_cap(&self, req: &mut RequestEnvelope) {
        if self.max_completion_tokens > 0 {
            let requested = req.max_tokens.unwrap_or(self.max_completion_tokens);
            req.max_tokens = Some(requested.min(self.max_completion_tokens));
        }
    }

    /// Completion tokens to reserve for this request.
    pub fn estimated_completion_tokens(&self, req: &RequestEnvelope) -> u32 {
        if self.max_completion_tokens > 0 {
            req.max_tokens
                .unwrap_or(self.max_completion_tokens)
                .min(self.max_completion_tokens)
        } else {
            req.max_tokens.unwrap_or(500)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(prompt: &str) -> RequestEnvelope {
        RequestEnvelope::new("tenant", "app", "gpt-4o", prompt)
    }

    #[test]
    fn clamps_a_larger_completion_budget_on_the_origin_copy() {
        let limits = RequestLimits::new(1_000, 100);
        let mut req = request("hello");
        req.max_tokens = Some(5_000);
        limits.enforce(&mut req).expect("within prompt cap");
        assert_eq!(req.max_tokens, Some(5_000), "identity must not change");
        limits.apply_completion_cap(&mut req);
        assert_eq!(req.max_tokens, Some(100));
    }

    #[test]
    fn supplies_the_cap_on_the_origin_copy_when_the_client_omits_max_tokens() {
        let limits = RequestLimits::new(1_000, 100);
        let mut req = request("hello");
        limits.enforce(&mut req).expect("within prompt cap");
        assert_eq!(req.max_tokens, None, "identity must remain unchanged");
        limits.apply_completion_cap(&mut req);
        assert_eq!(req.max_tokens, Some(100));
    }

    #[test]
    fn rejects_a_prompt_over_the_character_cap() {
        let limits = RequestLimits::new(10, 100);
        let mut req = request("this prompt is longer than ten characters");
        let error = limits
            .enforce(&mut req)
            .expect_err("prompt must be rejected");
        assert!(error.message.contains("maximum prompt size"));
    }

    #[test]
    fn accounts_for_message_content() {
        let limits = RequestLimits::new(20, 100);
        let mut req = request("hi");
        req.messages.push(sunborn_core::model::ChatMessage::new(
            "user",
            "x".repeat(30),
        ));
        assert!(limits.enforce(&mut req).is_err());
    }

    #[test]
    fn zero_limits_disable_both_caps() {
        let limits = RequestLimits::new(0, 0);
        let mut req = request(&"x".repeat(100));
        req.max_tokens = Some(9_999);
        limits.enforce(&mut req).expect("caps disabled");
        limits.apply_completion_cap(&mut req);
        assert_eq!(req.max_tokens, Some(9_999));
        assert_eq!(limits.estimated_completion_tokens(&req), 9_999);
    }
}
