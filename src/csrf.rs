//! Synchronizer-token CSRF protection for cookie sessions.

use axum::{
    body::{to_bytes, Body},
    extract::Request,
    http::{header, HeaderMap, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use sha2::{Digest, Sha256};
use tower_sessions::Session;

use crate::error::{AppError, AppResult};

pub const CSRF_SESSION_KEY: &str = "csrf_token";
pub const CSRF_HEADER: &str = "x-csrf-token";
pub const CSRF_FORM_FIELD: &str = "csrf_token";

const MAX_BODY_BYTES: usize = 2 * 1024 * 1024;

/// Ensure the session has a CSRF token, creating one if needed.
pub async fn ensure_token(session: &Session) -> AppResult<String> {
    if let Some(token) = session
        .get::<String>(CSRF_SESSION_KEY)
        .await
        .map_err(|err| AppError::Internal(format!("session error: {err}")))?
    {
        return Ok(token);
    }
    rotate_token(session).await
}

/// Replace the session CSRF token (e.g. after login session cycling).
pub async fn rotate_token(session: &Session) -> AppResult<String> {
    let token = generate_token();
    session
        .insert(CSRF_SESSION_KEY, token.clone())
        .await
        .map_err(|err| AppError::Internal(format!("session error: {err}")))?;
    Ok(token)
}

fn generate_token() -> String {
    use argon2::password_hash::rand_core::{OsRng, RngCore};
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn tokens_match(expected: &str, provided: &str) -> bool {
    if expected.is_empty() || provided.is_empty() {
        return false;
    }
    let left = Sha256::digest(expected.as_bytes());
    let right = Sha256::digest(provided.as_bytes());
    // Constant-time-ish compare of digests (equal length).
    left.iter()
        .zip(right.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

fn is_mutating(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn header_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get(CSRF_HEADER)
        .or_else(|| headers.get("X-CSRF-Token"))
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

fn form_token(body: &[u8], content_type: Option<&str>) -> Option<String> {
    let content_type = content_type?.to_ascii_lowercase();
    if !content_type.starts_with("application/x-www-form-urlencoded") {
        return None;
    }
    let body = std::str::from_utf8(body).ok()?;
    for pair in body.split('&') {
        let mut parts = pair.splitn(2, '=');
        let key = parts.next()?;
        let value = parts.next().unwrap_or("");
        if key == CSRF_FORM_FIELD {
            return Some(urlencoding_decode(value));
        }
    }
    None
}

/// Minimal application/x-www-form-urlencoded decoder for the CSRF field.
fn urlencoding_decode(input: &str) -> String {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b'%' if i + 2 < bytes.len() => {
                let hex = &input[i + 1..i + 3];
                if let Ok(v) = u8::from_str_radix(hex, 16) {
                    out.push(v);
                    i += 3;
                } else {
                    out.push(bytes[i]);
                    i += 1;
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn forbidden() -> Response {
    (StatusCode::FORBIDDEN, "CSRF token missing or invalid").into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_csrf_from_urlencoded_body() {
        let body = b"email=a%40b.com&csrf_token=abc123&password=x";
        assert_eq!(
            form_token(body, Some("application/x-www-form-urlencoded")),
            Some("abc123".into())
        );
    }

    #[test]
    fn rejects_non_form_bodies() {
        assert_eq!(form_token(b"{}", Some("application/json")), None);
    }

    #[test]
    fn tokens_match_only_when_equal() {
        assert!(tokens_match("aaa", "aaa"));
        assert!(!tokens_match("aaa", "bbb"));
        assert!(!tokens_match("", "aaa"));
    }
}

/// Reject mutating requests that omit a valid session CSRF token
/// (`X-CSRF-Token` header or `csrf_token` form field).
pub async fn protect(session: Session, mut req: Request, next: Next) -> Response {
    if !is_mutating(req.method()) {
        return next.run(req).await;
    }

    let expected = match ensure_token(&session).await {
        Ok(token) => token,
        Err(err) => {
            tracing::error!(error = %err, "csrf: failed to read session token");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    if let Some(provided) = header_token(req.headers()) {
        if tokens_match(&expected, &provided) {
            return next.run(req).await;
        }
        return forbidden();
    }

    let content_type = req
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let body = std::mem::replace(req.body_mut(), Body::empty());
    let bytes = match to_bytes(body, MAX_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(err) => {
            tracing::warn!(error = %err, "csrf: failed to read body");
            return StatusCode::BAD_REQUEST.into_response();
        }
    };

    let provided = form_token(&bytes, content_type.as_deref());
    *req.body_mut() = Body::from(bytes);

    match provided {
        Some(token) if tokens_match(&expected, &token) => next.run(req).await,
        _ => forbidden(),
    }
}
