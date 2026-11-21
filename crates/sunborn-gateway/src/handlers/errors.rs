//! Handlers grouped by domain; see `super` for shared imports.

use super::*;

pub struct AppError(SunbornError);

impl From<SunbornError> for AppError {
    fn from(err: SunbornError) -> Self {
        Self(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match self.0.code {
            sunborn_core::ErrorCode::Unauthorized => StatusCode::UNAUTHORIZED,
            sunborn_core::ErrorCode::Forbidden => StatusCode::FORBIDDEN,
            sunborn_core::ErrorCode::InvalidPayload => StatusCode::BAD_REQUEST,
            sunborn_core::ErrorCode::RateLimitExceeded => StatusCode::TOO_MANY_REQUESTS,
            sunborn_core::ErrorCode::CacheMiss => StatusCode::NOT_FOUND,
            sunborn_core::ErrorCode::CacheUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            sunborn_core::ErrorCode::CapacityExceeded => StatusCode::SERVICE_UNAVAILABLE,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };

        (
            status,
            Json(json!({
                "type": format!("https://sunborn.ai/errors/{:?}", self.0.code).to_lowercase(),
                "title": format!("{:?}", self.0.code),
                "status": status.as_u16(),
                "detail": self.0.message.clone(),
                "error": self.0,
            })),
        )
            .into_response()
    }
}
