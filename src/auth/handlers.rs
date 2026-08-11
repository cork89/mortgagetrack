//! Registration, login, and logout handlers.

use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, OriginalUri, Query, State},
    http::{HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use tower_sessions::Session;
use uuid::Uuid;

use super::models::{
    create_user, find_user_by_email, validate_email, validate_password, verify_password,
};
use super::rate_limit::{self, client_ip};
use crate::app_state::AppState;
use crate::auth::{
    encode_query_value, hx_redirect, is_htmx, is_share_invite_next, purge_session, resolve_user_id,
    safe_next, set_pending_share, set_user_id, share_token_from_next, take_pending_share,
    trust_identity_headers, HOME_PATH,
};
use crate::csrf;
use crate::error::{AppError, AppResult};
use crate::models::{accept_share_link, set_active_profile};
use crate::templates::{AuthErrorPartial, HtmlTemplate, LoginTemplate, RegisterTemplate};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", get(login_page).post(login_submit))
        .route("/register", get(register_page).post(register_submit))
        .route("/logout", post(logout))
}

#[derive(Debug, Deserialize)]
pub struct AuthQuery {
    pub next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    pub email: String,
    pub password: String,
    pub next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterForm {
    pub email: String,
    pub password: String,
    pub confirm_password: String,
    pub next: Option<String>,
}

fn edge_auth_gone() -> Response {
    (
        StatusCode::GONE,
        "Credential auth is handled by Better Auth at /api/auth. Reload the page.",
    )
        .into_response()
}

fn oauth_authorize_return(uri: &Uri) -> Option<String> {
    let query = uri.query()?;
    if !query.split('&').any(|p| p.starts_with("client_id=")) {
        return None;
    }
    Some(format!("/api/auth/oauth2/authorize?{query}"))
}

async fn login_page(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Query(q): Query<AuthQuery>,
) -> AppResult<Response> {
    remember_share_invite(&session, q.next.as_deref()).await?;
    let fields = auth_next_fields(q.next.as_deref());
    if resolve_user_id(&headers, &session).await?.is_some() {
        if let Some(oauth) = oauth_authorize_return(&uri) {
            return Ok(Redirect::to(&oauth).into_response());
        }
        let dest = if fields.next.is_empty() {
            HOME_PATH
        } else {
            &fields.next
        };
        return Ok(Redirect::to(dest).into_response());
    }
    let csrf_token = csrf::ensure_token(&session).await?;
    Ok(HtmlTemplate(LoginTemplate {
        csrf_token,
        app_name: state.app_name.clone(),
        error: String::new(),
        email: String::new(),
        next: fields.next,
        next_query: fields.next_query,
        share_invite: fields.share_invite,
        auth_edge: trust_identity_headers(),
    })
    .into_response())
}

async fn register_page(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    OriginalUri(uri): OriginalUri,
    Query(q): Query<AuthQuery>,
) -> AppResult<Response> {
    remember_share_invite(&session, q.next.as_deref()).await?;
    let fields = auth_next_fields(q.next.as_deref());
    if resolve_user_id(&headers, &session).await?.is_some() {
        if let Some(oauth) = oauth_authorize_return(&uri) {
            return Ok(Redirect::to(&oauth).into_response());
        }
        let dest = if fields.next.is_empty() {
            HOME_PATH
        } else {
            &fields.next
        };
        return Ok(Redirect::to(dest).into_response());
    }
    let csrf_token = csrf::ensure_token(&session).await?;
    Ok(HtmlTemplate(RegisterTemplate {
        csrf_token,
        app_name: state.app_name.clone(),
        error: String::new(),
        email: String::new(),
        next: fields.next,
        next_query: fields.next_query,
        share_invite: fields.share_invite,
        auth_edge: trust_identity_headers(),
    })
    .into_response())
}

async fn login_submit(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Form(form): Form<LoginForm>,
) -> Response {
    if trust_identity_headers() {
        return edge_auth_gone();
    }
    remember_share_invite(&session, form.next.as_deref())
        .await
        .ok();
    let next = safe_next(form.next.as_deref())
        .unwrap_or(HOME_PATH)
        .to_string();
    let ip = client_ip(&headers, Some(peer));
    match login_inner(&state, &session, &form, &ip).await {
        Ok(user_id) => auth_success_redirect(&state, &session, user_id, &headers, &next).await,
        Err(err) => {
            let status = auth_error_status(&err);
            let csrf_token = csrf::ensure_token(&session).await.unwrap_or_default();
            auth_error_response(
                &state.app_name,
                &headers,
                status,
                csrf_token,
                err.to_string(),
                &form.email,
                true,
                form.next.as_deref().unwrap_or(""),
            )
        }
    }
}

async fn login_inner(
    state: &AppState,
    session: &Session,
    form: &LoginForm,
    ip: &str,
) -> AppResult<Uuid> {
    let email = validate_email(&form.email)?;
    if form.password.is_empty() {
        return Err(AppError::BadRequest("Enter your password.".into()));
    }

    rate_limit::check_login(&state.pool, ip, email).await?;

    let Some((user, password_hash)) = find_user_by_email(&state.pool, email).await? else {
        return Err(AppError::BadRequest("Invalid email or password.".into()));
    };

    if !verify_password(&form.password, &password_hash).await? {
        return Err(AppError::BadRequest("Invalid email or password.".into()));
    }

    rate_limit::clear_login_email(&state.pool, email).await?;

    session
        .cycle_id()
        .await
        .map_err(|err| AppError::Internal(format!("failed to renew session: {err}")))?;
    csrf::rotate_token(session).await?;
    let user_id = user.uuid()?;
    set_user_id(session, user_id).await?;
    Ok(user_id)
}

async fn register_submit(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Form(form): Form<RegisterForm>,
) -> Response {
    if trust_identity_headers() {
        return edge_auth_gone();
    }
    remember_share_invite(&session, form.next.as_deref())
        .await
        .ok();
    let next = safe_next(form.next.as_deref())
        .unwrap_or(HOME_PATH)
        .to_string();
    let ip = client_ip(&headers, Some(peer));
    match register_inner(&state, &session, &form, &ip).await {
        Ok(user_id) => auth_success_redirect(&state, &session, user_id, &headers, &next).await,
        Err(err) => {
            let status = auth_error_status(&err);
            let csrf_token = csrf::ensure_token(&session).await.unwrap_or_default();
            auth_error_response(
                &state.app_name,
                &headers,
                status,
                csrf_token,
                err.to_string(),
                &form.email,
                false,
                form.next.as_deref().unwrap_or(""),
            )
        }
    }
}

async fn register_inner(
    state: &AppState,
    session: &Session,
    form: &RegisterForm,
    ip: &str,
) -> AppResult<Uuid> {
    let email = validate_email(&form.email)?;
    validate_password(&form.password)?;
    if form.password != form.confirm_password {
        return Err(AppError::BadRequest("Passwords do not match.".into()));
    }

    rate_limit::check_register(&state.pool, ip, email).await?;

    let user = create_user(&state.pool, email, &form.password).await?;
    session
        .cycle_id()
        .await
        .map_err(|err| AppError::Internal(format!("failed to renew session: {err}")))?;
    csrf::rotate_token(session).await?;
    let user_id = user.uuid()?;
    set_user_id(session, user_id).await?;
    Ok(user_id)
}

async fn logout(session: Session, headers: HeaderMap) -> AppResult<Response> {
    purge_session(&session).await?;
    Ok(hx_redirect(&headers, HOME_PATH))
}

struct AuthNextFields {
    next: String,
    next_query: String,
    share_invite: bool,
}

fn auth_next_fields(next: Option<&str>) -> AuthNextFields {
    let next = safe_next(next).unwrap_or("").to_string();
    AuthNextFields {
        share_invite: is_share_invite_next(&next),
        next_query: if next.is_empty() {
            String::new()
        } else {
            encode_query_value(&next)
        },
        next,
    }
}

async fn remember_share_invite(session: &Session, next: Option<&str>) -> AppResult<()> {
    let Some(next) = safe_next(next) else {
        return Ok(());
    };
    let Some(token) = share_token_from_next(next) else {
        return Ok(());
    };
    set_pending_share(session, token).await
}

async fn auth_success_redirect(
    state: &AppState,
    session: &Session,
    user_id: Uuid,
    headers: &HeaderMap,
    next: &str,
) -> Response {
    if let Ok(Some(token)) = take_pending_share(session).await {
        if let Ok(profile_id) = accept_share_link(&state.pool, user_id, &token).await {
            let _ = set_active_profile(&state.pool, user_id, Some(&profile_id)).await;
            return hx_redirect(headers, HOME_PATH);
        }
    }
    let dest = safe_next(Some(next)).unwrap_or(HOME_PATH);
    hx_redirect(headers, dest)
}

fn auth_error_status(err: &AppError) -> StatusCode {
    match err {
        AppError::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
        _ => StatusCode::BAD_REQUEST,
    }
}

fn auth_error_response(
    app_name: &str,
    headers: &HeaderMap,
    status: StatusCode,
    csrf_token: String,
    message: String,
    email: &str,
    is_login: bool,
    next: &str,
) -> Response {
    let fields = auth_next_fields(Some(next));
    if is_htmx(headers) {
        (status, HtmlTemplate(AuthErrorPartial { message })).into_response()
    } else if is_login {
        (
            status,
            HtmlTemplate(LoginTemplate {
                csrf_token,
                app_name: app_name.to_string(),
                error: message,
                email: email.to_string(),
                next: fields.next,
                next_query: fields.next_query,
                share_invite: fields.share_invite,
                auth_edge: false,
            }),
        )
            .into_response()
    } else {
        (
            status,
            HtmlTemplate(RegisterTemplate {
                csrf_token,
                app_name: app_name.to_string(),
                error: message,
                email: email.to_string(),
                next: fields.next,
                next_query: fields.next_query,
                share_invite: fields.share_invite,
                auth_edge: false,
            }),
        )
            .into_response()
    }
}
