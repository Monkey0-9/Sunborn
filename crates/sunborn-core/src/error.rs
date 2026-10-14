use serde::{Deserialize, Serialize};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, SunbornError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    Unauthorized,
    Forbidden,
    TenantNotFound,
    InvalidPayload,
    RateLimitExceeded,
    CacheMiss,
    CacheUnavailable,
    OriginError,
    OriginTimeout,
    InternalError,
    ConfigurationError,
}

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
#[error("{code:?}: {message}")]
pub struct SunbornError {
    pub code: ErrorCode,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl SunbornError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn unauthorized(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthorized, msg)
    }

    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, msg)
    }

    pub fn invalid_payload(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidPayload, msg)
    }

    pub fn internal(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, msg)
    }

    pub fn rate_limit(msg: impl Into<String>) -> Self {
        Self::new(ErrorCode::RateLimitExceeded, msg)
    }
}
