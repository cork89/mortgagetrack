//! Account settings: change password and delete account.

use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use tower_sessions::Session;
use uuid::Uuid;

use super::identicon;
use super::models::{
    delete_user, password_hash_for_user, update_password, validate_password, verify_password,
};
use crate::app_state::AppState;
use crate::auth::{hx_redirect, is_htmx, purge_session, AuthUser, HOME_PATH};
use crate::csrf;
use crate::error::{AppError, AppResult};
use crate::templates::{AccountTemplate, AuthErrorPartial, HtmlTemplate};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/account", get(account_page))
        .route("/account/password", post(change_password))
        .route("/account/delete", post(delete_account))
        .route("/avatars/{id}", get(avatar))
}

#[derive(Debug, Deserialize)]
pub struct AccountQuery {
    pub password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordForm {
    pub current_password: String,
    pub new_password: String,
    pub confirm_password: String,
}

#[derive(Debug, Deserialize)]
pub struct DeleteAccountForm {
    pub password: String,
}

async fn account_page(
    session: Session,
    user: AuthUser,
    Query(q): Query<AccountQuery>,
) -> AppResult<Response> {
    let csrf_token = csrf::ensure_token(&session).await?;
    let password_updated = q.password.as_deref() == Some("updated");
    Ok(HtmlTemplate(AccountTemplate {
        csrf_token,
        user_id: user.id.to_string(),
        email: user.email,
        password_updated,
        password_error: String::new(),
        delete_error: String::new(),
    })
    .into_response())
}

async fn avatar(Path(id): Path<String>) -> AppResult<Response> {
    let id = id.strip_suffix(".svg").unwrap_or(&id);
    let user_id = Uuid::parse_str(id)
        .map_err(|_| AppError::NotFound("Avatar not found.".into()))?;

    let svg = identicon::svg_for_seed(&user_id.to_string());
    let mut response = (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/svg+xml; charset=utf-8")],
        svg,
    )
        .into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=604800, immutable"),
    );
    Ok(response)
}

async fn change_password(
    State(state): State<AppState>,
    session: Session,
    user: AuthUser,
    headers: HeaderMap,
    Form(form): Form<ChangePasswordForm>,
) -> Response {
    match change_password_inner(&state, &session, &user, &form).await {
        Ok(()) => hx_redirect(&headers, "/account?password=updated"),
        Err(err) => {
            let message = err.to_string();
            if is_htmx(&headers) {
                (
                    StatusCode::BAD_REQUEST,
                    HtmlTemplate(AuthErrorPartial { message }),
                )
                    .into_response()
            } else {
                let csrf_token = csrf::ensure_token(&session).await.unwrap_or_default();
                (
                    StatusCode::BAD_REQUEST,
                    HtmlTemplate(AccountTemplate {
                        csrf_token,
                        user_id: user.id.to_string(),
                        email: user.email,
                        password_updated: false,
                        password_error: message,
                        delete_error: String::new(),
                    }),
                )
                    .into_response()
            }
        }
    }
}

async fn change_password_inner(
    state: &AppState,
    session: &Session,
    user: &AuthUser,
    form: &ChangePasswordForm,
) -> AppResult<()> {
    if form.current_password.is_empty() {
        return Err(AppError::BadRequest("Enter your current password.".into()));
    }

    let Some(hash) = password_hash_for_user(&state.pool, user.id).await? else {
        return Err(AppError::Internal("No credentials for user".into()));
    };

    if !verify_password(&form.current_password, &hash).await? {
        return Err(AppError::BadRequest(
            "Current password is incorrect.".into(),
        ));
    }

    validate_password(&form.new_password)?;
    if form.new_password != form.confirm_password {
        return Err(AppError::BadRequest("Passwords do not match.".into()));
    }

    update_password(&state.pool, user.id, &form.new_password).await?;

    session.cycle_id().await.map_err(|err| {
        AppError::Internal(format!("failed to renew session: {err}"))
    })?;
    csrf::rotate_token(session).await?;
    Ok(())
}

async fn delete_account(
    State(state): State<AppState>,
    session: Session,
    user: AuthUser,
    headers: HeaderMap,
    Form(form): Form<DeleteAccountForm>,
) -> Response {
    match delete_account_inner(&state, &session, &user, &form).await {
        Ok(()) => hx_redirect(&headers, HOME_PATH),
        Err(err) => {
            let message = err.to_string();
            if is_htmx(&headers) {
                (
                    StatusCode::BAD_REQUEST,
                    HtmlTemplate(AuthErrorPartial { message }),
                )
                    .into_response()
            } else {
                let csrf_token = csrf::ensure_token(&session).await.unwrap_or_default();
                (
                    StatusCode::BAD_REQUEST,
                    HtmlTemplate(AccountTemplate {
                        csrf_token,
                        user_id: user.id.to_string(),
                        email: user.email,
                        password_updated: false,
                        password_error: String::new(),
                        delete_error: message,
                    }),
                )
                    .into_response()
            }
        }
    }
}

async fn delete_account_inner(
    state: &AppState,
    session: &Session,
    user: &AuthUser,
    form: &DeleteAccountForm,
) -> AppResult<()> {
    if form.password.is_empty() {
        return Err(AppError::BadRequest("Enter your password.".into()));
    }

    let Some(hash) = password_hash_for_user(&state.pool, user.id).await? else {
        return Err(AppError::Internal("No credentials for user".into()));
    };

    if !verify_password(&form.password, &hash).await? {
        return Err(AppError::BadRequest("Password is incorrect.".into()));
    }

    delete_user(&state.pool, user.id).await?;
    purge_session(session).await?;
    Ok(())
}
