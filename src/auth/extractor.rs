//! `AuthUser` extractor with HTMX-aware unauthorized redirects.

use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts, HeaderValue, StatusCode},
    response::{IntoResponse, Redirect, Response},
};
use sqlx::SqlitePool;
use tower_sessions::Session;
use uuid::Uuid;

use super::middleware::{get_user_id, is_htmx};
use super::models::find_user_by_id;
use crate::app_state::AppState;
use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
    pub avatar: Option<String>,
    pub default_tab: String,
}

/// Resolve the signed-in user from the session, if any.
pub async fn current_user(session: &Session, pool: &SqlitePool) -> AppResult<Option<AuthUser>> {
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
    }))
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

        let session = Session::from_request_parts(parts, state)
            .await
            .map_err(|_| reject())?;

        let user_id = get_user_id(&session)
            .await
            .map_err(|_| reject())?
            .ok_or_else(reject)?;

        let user = find_user_by_id(&state.pool, user_id)
            .await
            .map_err(|_| reject())?
            .ok_or_else(reject)?;

        Ok(AuthUser {
            id: user_id,
            email: user.email,
            avatar: user.avatar,
            default_tab: user.default_tab,
        })
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
