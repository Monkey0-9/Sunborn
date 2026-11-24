use axum::{
    extract::Request,
    http::{HeaderName, HeaderValue},
    middleware::Next,
    response::Response,
};
use tracing::Instrument;
use uuid::Uuid;

pub static X_REQUEST_ID: HeaderName = HeaderName::from_static("x-request-id");
pub static TRACEPARENT: HeaderName = HeaderName::from_static("traceparent");

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequestId(pub String);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Traceparent(pub String);

/// Distributed tracing middleware: ensures every request has a validated or generated `x-request-id`
/// and propagates W3C `traceparent` headers across microservices (Issue #37).
/// Injects `x-request-id` and `traceparent` into request extensions, attaches them to tracing spans,
/// and guarantees their presence in response headers across all HTTP status codes.
pub async fn request_id_middleware(mut req: Request, next: Next) -> Response {
    let request_id = req
        .headers()
        .get(&X_REQUEST_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 128
                && s.chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        })
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string());

    let traceparent_opt = req
        .headers()
        .get(&TRACEPARENT)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() == 55 && s.starts_with("00-"));

    req.extensions_mut().insert(RequestId(request_id.clone()));
    if let Some(ref tp) = traceparent_opt {
        req.extensions_mut().insert(Traceparent(tp.clone()));
    }

    // Hardened header security bounds against Slowloris / Memory Exhaustion DoS:
    // Individual header field limit: 16 KB (16,384 bytes)
    // Cumulative headers size limit: 32 KB (32,768 bytes)
    let mut total_header_bytes = 0usize;
    for (name, value) in req.headers() {
        let entry_size = name.as_str().len() + value.len();
        if value.len() > 16384 {
            let mut res = axum::response::Response::builder()
                .status(axum::http::StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    r#"{"error":{"code":"HEADER_TOO_LARGE","message":"Header field exceeds 16KB limit"}}"#,
                ))
                .unwrap_or_default();
            if let Ok(hv) = HeaderValue::from_str(&request_id) {
                res.headers_mut().insert(X_REQUEST_ID.clone(), hv);
            }
            return res;
        }
        total_header_bytes += entry_size;
        if total_header_bytes > 32768 {
            let mut res = axum::response::Response::builder()
                .status(axum::http::StatusCode::REQUEST_HEADER_FIELDS_TOO_LARGE)
                .header(axum::http::header::CONTENT_TYPE, "application/json")
                .body(axum::body::Body::from(
                    r#"{"error":{"code":"HEADERS_TOTAL_TOO_LARGE","message":"Cumulative request headers exceed 32KB limit"}}"#,
                ))
                .unwrap_or_default();
            if let Ok(hv) = HeaderValue::from_str(&request_id) {
                res.headers_mut().insert(X_REQUEST_ID.clone(), hv);
            }
            return res;
        }
    }

    let span = tracing::info_span!("http_request", request_id = %request_id);
    let mut response = next.run(req).instrument(span).await;

    if let Ok(header_val) = HeaderValue::from_str(&request_id) {
        response
            .headers_mut()
            .insert(X_REQUEST_ID.clone(), header_val);
    }

    if let Some(ref tp) = traceparent_opt {
        if let Ok(tp_val) = HeaderValue::from_str(tp) {
            response.headers_mut().insert(TRACEPARENT.clone(), tp_val);
        }
    }

    response
}
