//! `AuthUser` extractor with HTMX-aware unauthorized redirects.
//!
//! Identity resolution order when `AUTH_TRUST_HEADERS` is enabled:
//! 1. `X-User-Id` / `X-User-Email` / `X-User-Role` / `X-User-Paid-Until` injected by
//!    the Cloudflare Worker after Better Auth
//! 2. Legacy tower-sessions `user_id` (local `cargo run` / transition)

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use chrono::{DateTime, Utc};
use tower_sessions::Session;
use uuid::Uuid;

use super::middleware::{get_user_id, is_htmx};
use super::models::{
    ensure_user_projection, find_user_by_id, paid_until_active, parse_paid_until, UserRole,
};
use crate::app_state::AppState;
use crate::config::env_bool;
use crate::db::DbPool;
use crate::error::AppResult;

pub const USER_ID_HEADER: &str = "x-user-id";
pub const USER_EMAIL_HEADER: &str = "x-user-email";
pub const USER_ROLE_HEADER: &str = "x-user-role";
pub const USER_PAID_UNTIL_HEADER: &str = "x-user-paid-until";

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
    pub avatar: Option<String>,
    pub default_tab: String,
    pub payments_year_expand: String,
    pub role: UserRole,
    pub paid_until: Option<DateTime<Utc>>,
}

impl AuthUser {
    pub fn is_admin(&self) -> bool {
        self.role == UserRole::Admin
    }

    pub fn is_paid(&self) -> bool {
        self.is_admin() || paid_until_active(self.paid_until, Utc::now())
    }
}

/// Authenticated admin. Unauthenticated → login redirect; non-admin → 403.
#[derive(Debug, Clone)]
pub struct AdminUser(pub AuthUser);

/// Authenticated paid (or admin) user. Unauthenticated → login redirect; unpaid → 403.
#[derive(Debug, Clone)]
pub struct PaidUser(pub AuthUser);

pub fn trust_identity_headers() -> bool {
    env_bool("AUTH_TRUST_HEADERS", false)
}

fn header_str<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
}

/// Read Worker-injected identity (only when trusted).
pub fn identity_from_headers(
    headers: &HeaderMap,
) -> AppResult<Option<(Uuid, Option<String>, UserRole, Option<DateTime<Utc>>)>> {
    if !trust_identity_headers() {
        return Ok(None);
    }
    let Some(raw) = header_str(headers, USER_ID_HEADER) else {
        return Ok(None);
    };
    let id = Uuid::parse_str(raw)
        .map_err(|_| crate::error::AppError::Internal("invalid X-User-Id header".into()))?;
    let email = header_str(headers, USER_EMAIL_HEADER).map(str::to_string);
    let role = UserRole::parse(header_str(headers, USER_ROLE_HEADER).unwrap_or("user"));
    let paid_until = parse_paid_until(header_str(headers, USER_PAID_UNTIL_HEADER));
    Ok(Some((id, email, role, paid_until)))
}

/// Resolve the signed-in user id from trusted headers or the session.
pub async fn resolve_user_id(headers: &HeaderMap, session: &Session) -> AppResult<Option<Uuid>> {
    if let Some((id, _, _, _)) = identity_from_headers(headers)? {
        return Ok(Some(id));
    }
    get_user_id(session).await
}

fn auth_user_from_row(user_id: Uuid, user: super::models::User) -> AuthUser {
    let role = user.role_enum();
    let paid_until = user.paid_until_dt();
    AuthUser {
        id: user_id,
        email: user.email,
        avatar: user.avatar,
        default_tab: user.default_tab,
        payments_year_expand: user.payments_year_expand,
        role,
        paid_until,
    }
}

async fn load_auth_user(
    headers: &HeaderMap,
    session: &Session,
    pool: &DbPool,
) -> AppResult<Option<AuthUser>> {
    if let Some((user_id, email, role, paid_until)) = identity_from_headers(headers)? {
        let user = if let Some(email) = email.as_deref() {
            ensure_user_projection(pool, user_id, email, role, paid_until).await?
        } else if let Some(user) = find_user_by_id(pool, user_id).await? {
            user
        } else {
            return Ok(None);
        };
        let mut auth = auth_user_from_row(user_id, user);
        // Prefer edge headers as source of truth when present.
        auth.role = role;
        auth.paid_until = paid_until;
        return Ok(Some(auth));
    }

    let Some(user_id) = get_user_id(session).await? else {
        return Ok(None);
    };
    let Some(user) = find_user_by_id(pool, user_id).await? else {
        return Ok(None);
    };
    Ok(Some(auth_user_from_row(user_id, user)))
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

#[derive(Debug)]
pub enum AuthzReject {
    Unauthenticated(AuthRedirect),
    Forbidden,
}

impl IntoResponse for AuthzReject {
    fn into_response(self) -> Response {
        match self {
            Self::Unauthenticated(redirect) => redirect.into_response(),
            Self::Forbidden => StatusCode::FORBIDDEN.into_response(),
        }
    }
}

impl FromRequestParts<AppState> for AdminUser {
    type Rejection = AuthzReject;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state)
            .await
            .map_err(AuthzReject::Unauthenticated)?;
        if !user.is_admin() {
            return Err(AuthzReject::Forbidden);
        }
        Ok(AdminUser(user))
    }
}

impl FromRequestParts<AppState> for PaidUser {
    type Rejection = AuthzReject;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let user = AuthUser::from_request_parts(parts, state)
            .await
            .map_err(AuthzReject::Unauthenticated)?;
        if !user.is_paid() {
            return Err(AuthzReject::Forbidden);
        }
        Ok(PaidUser(user))
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn sample_user(role: UserRole, paid_until: Option<DateTime<Utc>>) -> AuthUser {
        AuthUser {
            id: Uuid::nil(),
            email: "a@b.com".into(),
            avatar: None,
            default_tab: "calendar".into(),
            payments_year_expand: "current".into(),
            role,
            paid_until,
        }
    }

    #[test]
    fn admin_inherits_paid() {
        let admin = sample_user(UserRole::Admin, None);
        assert!(admin.is_admin());
        assert!(admin.is_paid());
    }

    #[test]
    fn unpaid_when_no_paid_until() {
        let user = sample_user(UserRole::User, None);
        assert!(!user.is_admin());
        assert!(!user.is_paid());
    }

    #[test]
    fn paid_while_before_expiry() {
        let until = Utc::now() + Duration::hours(1);
        let user = sample_user(UserRole::User, Some(until));
        assert!(user.is_paid());
    }

    #[test]
    fn unpaid_after_expiry() {
        let until = Utc::now() - Duration::seconds(1);
        let user = sample_user(UserRole::User, Some(until));
        assert!(!user.is_paid());
    }
}
