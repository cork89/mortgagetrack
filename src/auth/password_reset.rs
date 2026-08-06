//! Forgot-password / reset-password flow.

use std::net::SocketAddr;

use argon2::password_hash::rand_core::{OsRng, RngCore};
use axum::{
    extract::{ConnectInfo, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::get,
    Form, Router,
};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tower_sessions::Session;
use uuid::Uuid;

use super::models::{
    find_user_by_email, update_password, validate_email, validate_password, User,
};
use super::rate_limit::{self, client_ip};
use crate::app_state::AppState;
use crate::auth::{get_user_id, hx_redirect, is_htmx, set_user_id, HOME_PATH};
use crate::csrf;
use crate::db::{execute, get_conn, params, query_optional, DbPool, FromRow};
use crate::error::{AppError, AppResult};
use crate::mail::Message;
use crate::templates::{
    AuthErrorPartial, ForgotPasswordTemplate, HtmlTemplate, ResetPasswordTemplate,
};

const RESET_TTL_HOURS: i64 = 1;
const SENT_MESSAGE: &str =
    "If an account exists for that email, we sent a password reset link. It expires in one hour.";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/forgot-password", get(forgot_page).post(forgot_submit))
        .route("/reset-password", get(reset_page).post(reset_submit))
}

pub async fn ensure_schema(pool: &DbPool) -> AppResult<()> {
    let conn = get_conn(pool).await?;
    execute(
        &conn,
        r#"
        CREATE TABLE IF NOT EXISTS password_reset_tokens (
            id TEXT PRIMARY KEY NOT NULL,
            user_id TEXT NOT NULL,
            token_hash TEXT NOT NULL UNIQUE,
            expires_at TEXT NOT NULL,
            used_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
        (),
    )
    .await?;
    execute(
        &conn,
        "CREATE INDEX IF NOT EXISTS password_reset_tokens_user_id_idx ON password_reset_tokens (user_id)",
        (),
    )
    .await?;
    execute(
        &conn,
        "CREATE INDEX IF NOT EXISTS password_reset_tokens_expires_at_idx ON password_reset_tokens (expires_at)",
        (),
    )
    .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ForgotQuery {
    pub sent: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ForgotForm {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetQuery {
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResetForm {
    pub token: String,
    pub password: String,
    pub confirm_password: String,
}

async fn forgot_page(
    State(state): State<AppState>,
    session: Session,
    Query(q): Query<ForgotQuery>,
) -> AppResult<Response> {
    if get_user_id(&session).await?.is_some() {
        return Ok(Redirect::to(HOME_PATH).into_response());
    }
    let csrf_token = csrf::ensure_token(&session).await?;
    Ok(HtmlTemplate(ForgotPasswordTemplate {
        csrf_token,
        app_name: state.app_name.clone(),
        error: String::new(),
        email: String::new(),
        sent: q.sent.as_deref() == Some("1"),
        sent_message: SENT_MESSAGE.to_string(),
    })
    .into_response())
}

async fn forgot_submit(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Form(form): Form<ForgotForm>,
) -> Response {
    let ip = client_ip(&headers, Some(peer));
    match forgot_inner(&state, &form, &ip).await {
        Ok(()) => hx_redirect(&headers, "/forgot-password?sent=1"),
        Err(err) => {
            let status = match &err {
                AppError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
                _ => StatusCode::BAD_REQUEST,
            };
            let message = err.to_string();
            if is_htmx(&headers) {
                (
                    status,
                    HtmlTemplate(AuthErrorPartial { message }),
                )
                    .into_response()
            } else {
                let csrf_token = csrf::ensure_token(&session).await.unwrap_or_default();
                (
                    status,
                    HtmlTemplate(ForgotPasswordTemplate {
                        csrf_token,
                        app_name: state.app_name.clone(),
                        error: message,
                        email: form.email,
                        sent: false,
                        sent_message: SENT_MESSAGE.to_string(),
                    }),
                )
                    .into_response()
            }
        }
    }
}

async fn forgot_inner(state: &AppState, form: &ForgotForm, ip: &str) -> AppResult<()> {
    let email = validate_email(&form.email)?;
    rate_limit::check_password_reset(&state.pool, ip, email).await?;

    // Always succeed from the caller's perspective when the email is well-formed,
    // whether or not an account exists (avoid account enumeration).
    if let Some((user, _)) = find_user_by_email(&state.pool, email).await? {
        let token = create_reset_token(&state.pool, user.uuid()?).await?;
        send_reset_email(state, &user, &token).await?;
    }

    Ok(())
}

async fn send_reset_email(state: &AppState, user: &User, token: &str) -> AppResult<()> {
    let reset_url = format!(
        "{}/reset-password?token={}",
        state.app_base_url.trim_end_matches('/'),
        token
    );
    let subject = format!("Reset your {} password", state.app_name);
    let text = format!(
        "Reset your {} password using this link (expires in {RESET_TTL_HOURS} hour):\n\n{reset_url}\n\nIf you did not request this, you can ignore this email.",
        state.app_name
    );
    let html = format!(
        "<p>Reset your {} password using the link below. It expires in {RESET_TTL_HOURS} hour.</p>\
         <p><a href=\"{reset_url}\">Reset password</a></p>\
         <p>If you did not request this, you can ignore this email.</p>",
        state.app_name
    );

    state
        .mailer
        .send(Message {
            to: user.email.clone(),
            subject: subject.into(),
            text,
            html,
        })
        .await
}

async fn reset_page(
    session: Session,
    State(state): State<AppState>,
    Query(q): Query<ResetQuery>,
) -> AppResult<Response> {
    if get_user_id(&session).await?.is_some() {
        return Ok(Redirect::to(HOME_PATH).into_response());
    }

    let token = q.token.unwrap_or_default();
    let csrf_token = csrf::ensure_token(&session).await?;

    if token.is_empty() {
        return Ok(HtmlTemplate(ResetPasswordTemplate {
            csrf_token,
            app_name: state.app_name.clone(),
            error: "This reset link is missing or invalid.".into(),
            token: String::new(),
            token_valid: false,
        })
        .into_response());
    }

    let token_valid = peek_reset_token(&state.pool, &token).await?.is_some();
    Ok(HtmlTemplate(ResetPasswordTemplate {
        csrf_token,
        app_name: state.app_name.clone(),
        error: if token_valid {
            String::new()
        } else {
            "This reset link is invalid or has expired.".into()
        },
        token,
        token_valid,
    })
    .into_response())
}

async fn reset_submit(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<ResetForm>,
) -> Response {
    match reset_inner(&state, &session, &form).await {
        Ok(()) => hx_redirect(&headers, HOME_PATH),
        Err(err) => {
            let status = StatusCode::BAD_REQUEST;
            let message = err.to_string();
            if is_htmx(&headers) {
                (
                    status,
                    HtmlTemplate(AuthErrorPartial { message }),
                )
                    .into_response()
            } else {
                let csrf_token = csrf::ensure_token(&session).await.unwrap_or_default();
                let token_valid = !form.token.is_empty()
                    && peek_reset_token(&state.pool, &form.token)
                        .await
                        .ok()
                        .flatten()
                        .is_some();
                (
                    status,
                    HtmlTemplate(ResetPasswordTemplate {
                        csrf_token,
                        app_name: state.app_name.clone(),
                        error: message,
                        token: form.token,
                        token_valid,
                    }),
                )
                    .into_response()
            }
        }
    }
}

async fn reset_inner(state: &AppState, session: &Session, form: &ResetForm) -> AppResult<()> {
    if form.token.is_empty() {
        return Err(AppError::BadRequest(
            "This reset link is missing or invalid.".into(),
        ));
    }
    validate_password(&form.password)?;
    if form.password != form.confirm_password {
        return Err(AppError::BadRequest("Passwords do not match.".into()));
    }

    let user_id = consume_reset_token(&state.pool, &form.token).await?;
    update_password(&state.pool, user_id, &form.password).await?;

    session.cycle_id().await.map_err(|err| {
        AppError::Internal(format!("failed to renew session: {err}"))
    })?;
    csrf::rotate_token(session).await?;
    set_user_id(session, user_id).await?;
    Ok(())
}

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

async fn create_reset_token(pool: &DbPool, user_id: Uuid) -> AppResult<String> {
    let conn = get_conn(pool).await?;

    // Invalidate unused tokens for this user so only the latest email works.
    execute(
        &conn,
        r#"
        UPDATE password_reset_tokens
        SET used_at = datetime('now')
        WHERE user_id = ?
          AND used_at IS NULL
        "#,
        params![user_id.to_string()],
    )
    .await?;

    let id = Uuid::new_v4().to_string();
    let token = generate_token();
    let token_hash = hash_token(&token);

    execute(
        &conn,
        r#"
        INSERT INTO password_reset_tokens (id, user_id, token_hash, expires_at)
        VALUES (?, ?, ?, datetime('now', '+1 hours'))
        "#,
        params![id.as_str(), user_id.to_string(), token_hash.as_str()],
    )
    .await?;

    Ok(token)
}

struct ResetTokenRow {
    user_id: String,
}

impl FromRow for ResetTokenRow {
    fn from_row(row: &crate::db::Row) -> AppResult<Self> {
        Ok(Self {
            user_id: row.get(0)?,
        })
    }
}

async fn peek_reset_token(pool: &DbPool, token: &str) -> AppResult<Option<Uuid>> {
    let token_hash = hash_token(token);
    let conn = get_conn(pool).await?;
    let row: Option<ResetTokenRow> = query_optional(
        &conn,
        r#"
        SELECT user_id
        FROM password_reset_tokens
        WHERE token_hash = ?
          AND used_at IS NULL
          AND expires_at > datetime('now')
        "#,
        params![token_hash.as_str()],
    )
    .await?;

    row.map(|r| {
        Uuid::parse_str(&r.user_id)
            .map_err(|_| AppError::Internal("invalid user id in reset token".into()))
    })
    .transpose()
}

async fn consume_reset_token(pool: &DbPool, token: &str) -> AppResult<Uuid> {
    let token_hash = hash_token(token);
    let conn = get_conn(pool).await?;
    let row: Option<ResetTokenRow> = query_optional(
        &conn,
        r#"
        SELECT user_id
        FROM password_reset_tokens
        WHERE token_hash = ?
          AND used_at IS NULL
          AND expires_at > datetime('now')
        "#,
        params![token_hash.as_str()],
    )
    .await?;

    let Some(row) = row else {
        return Err(AppError::BadRequest(
            "This reset link is invalid or has expired.".into(),
        ));
    };

    let user_id = Uuid::parse_str(&row.user_id)
        .map_err(|_| AppError::Internal("invalid user id in reset token".into()))?;

    let updated = execute(
        &conn,
        r#"
        UPDATE password_reset_tokens
        SET used_at = datetime('now')
        WHERE token_hash = ?
          AND used_at IS NULL
          AND expires_at > datetime('now')
        "#,
        params![token_hash.as_str()],
    )
    .await?;

    if updated == 0 {
        return Err(AppError::BadRequest(
            "This reset link is invalid or has expired.".into(),
        ));
    }

    Ok(user_id)
}
