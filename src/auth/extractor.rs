//! `AuthUser` extractor with HTMX-aware unauthorized redirects.
//!
//! Identity resolution order when `AUTH_TRUST_HEADERS` is enabled:
//! 1. `X-User-Id` / `X-User-Email` injected by the Cloudflare Worker after Better Auth
//! 2. Legacy tower-sessions `user_id` (local `cargo run` / transition)

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use tower_sessions::Session;
use uuid::Uuid;

use super::middleware::{get_user_id, is_htmx};
use super::models::{ensure_user_projection, find_user_by_id};
use crate::app_state::AppState;
use crate::config::env_bool;
use crate::db::DbPool;
use crate::error::AppResult;

pub const USER_ID_HEADER: &str = "x-user-id";
pub const USER_EMAIL_HEADER: &str = "x-user-email";

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
    pub avatar: Option<String>,
    pub default_tab: String,
    pub payments_year_expand: String,
}

pub fn trust_identity_headers() -> bool {
    env_bool("AUTH_TRUST_HEADERS", false)
}

/// Read Worker-injected identity (only when trusted).
pub fn identity_from_headers(headers: &HeaderMap) -> AppResult<Option<(Uuid, Option<String>)>> {
    if !trust_identity_headers() {
        return Ok(None);
    }
    let Some(raw) = headers
        .get(USER_ID_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
    else {
        return Ok(None);
    };
    let id = Uuid::parse_str(raw)
        .map_err(|_| crate::error::AppError::Internal("invalid X-User-Id header".into()))?;
    let email = headers
        .get(USER_EMAIL_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    Ok(Some((id, email)))
}

/// Resolve the signed-in user id from trusted headers or the session.
pub async fn resolve_user_id(headers: &HeaderMap, session: &Session) -> AppResult<Option<Uuid>> {
    if let Some((id, _)) = identity_from_headers(headers)? {
        return Ok(Some(id));
    }
    get_user_id(session).await
}

async fn load_auth_user(
    headers: &HeaderMap,
    session: &Session,
    pool: &DbPool,
) -> AppResult<Option<AuthUser>> {
    if let Some((user_id, email)) = identity_from_headers(headers)? {
        let user = if let Some(email) = email.as_deref() {
            ensure_user_projection(pool, user_id, email).await?
        } else if let Some(user) = find_user_by_id(pool, user_id).await? {
            user
        } else {
            return Ok(None);
        };
        return Ok(Some(AuthUser {
            id: user_id,
            email: user.email,
            avatar: user.avatar,
            default_tab: user.default_tab,
            payments_year_expand: user.payments_year_expand,
        }));
    }

    let Some(user_id) = get_user_id(session).await? else {
        return Ok(None);
    };
    let Some(user) = find_user_by_id(pool, user_id).await? else {
        return Ok(None);
    };
    Ok(Some(AuthUser {
        id: user_id,
        email: user.email,
        avatar: user.avatar,
        default_tab: user.default_tab,
        payments_year_expand: user.payments_year_expand,
    }))
}

/// Resolve the signed-in user from headers/session, if any.
pub async fn current_user(
    headers: &HeaderMap,
    session: &Session,
    pool: &DbPool,
) -> AppResult<Option<AuthUser>> {
    load_auth_user(headers, session, pool).await
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AuthRedirect;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let htmx = is_htmx(&parts.headers);
        let reject = || AuthRedirect {
            location: "/login",
            htmx,
        };

        let headers = parts.headers.clone();

        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| reject())?;

        load_auth_user(&headers, &session, &state.pool)
            .await
            .map_err(|_| reject())?
            .ok_or_else(reject)
    }
}

/// Reject unauthenticated access: `HX-Redirect` for HTMX, else 303 to login.
#[derive(Debug)]
pub struct AuthRedirect {
    location: &'static str,
    htmx: bool,
}

impl IntoResponse for AuthRedirect {
    fn into_response(self) -> Response {
        if self.htmx {
            let mut response = StatusCode::UNAUTHORIZED.into_response();
            if let Ok(value) = HeaderValue::from_str(self.location) {
                response.headers_mut().insert(
                    header::HeaderName::from_static("hx-redirect"),
                    value,
                );
            }
            response
        } else {
            Redirect::to(self.location).into_response()
        }
    }
}
