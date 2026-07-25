//! HTMX-aware redirects and session key helpers.

use axum::{
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;

use crate::error::{AppError, AppResult};

pub const USER_ID_KEY: &str = "user_id";

/// Post-auth landing page (Homestead dashboard lives at `/`).
pub const HOME_PATH: &str = "/";

pub fn is_htmx(headers: &HeaderMap) -> bool {
    headers
        .get("hx-request")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("true"))
}

/// HTMX requests get `HX-Redirect`; full-page requests get a 303.
pub fn hx_redirect(headers: &HeaderMap, location: &str) -> Response {
    if is_htmx(headers) {
        let mut response = StatusCode::OK.into_response();
        if let Ok(value) = HeaderValue::from_str(location) {
            response.headers_mut().insert(
                header::HeaderName::from_static("hx-redirect"),
                value,
            );
        }
        response
    } else {
        Redirect::to(location).into_response()
    }
}

pub async fn get_user_id(session: &Session) -> AppResult<Option<uuid::Uuid>> {
    let raw: Option<String> = session
        .get(USER_ID_KEY)
        .await
        .map_err(|err| AppError::Internal(format!("session error: {err}")))?;
    match raw {
        Some(s) => {
            let id = uuid::Uuid::parse_str(&s)
                .map_err(|_| AppError::Internal("invalid user_id in session".into()))?;
            Ok(Some(id))
        }
        None => Ok(None),
    }
}

pub async fn set_user_id(session: &Session, user_id: uuid::Uuid) -> AppResult<()> {
    session
        .insert(USER_ID_KEY, user_id.to_string())
        .await
        .map_err(|err| AppError::Internal(format!("session error: {err}")))
}

/// Clear and destroy the session (`tower-sessions` equivalent of purge).
pub async fn purge_session(session: &Session) -> AppResult<()> {
    session
        .flush()
        .await
        .map_err(|err| AppError::Internal(format!("failed to purge session: {err}")))
}
