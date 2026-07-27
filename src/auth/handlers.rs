//! Registration, login, and logout handlers.

use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Router,
};
use serde::Deserialize;
use tower_sessions::Session;

use super::middleware::{
    get_user_id, hx_redirect, is_htmx, purge_session, set_user_id, HOME_PATH,
};
use super::models::{
    create_user, find_user_by_email, validate_email, validate_password, verify_password,
};
use super::next::safe_next;
use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::templates::{AuthErrorPartial, HtmlTemplate, LoginTemplate, RegisterTemplate};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", get(login_page).post(login_submit))
        .route("/register", get(register_page).post(register_submit))
        .route("/logout", post(logout).get(logout))
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

async fn login_page(session: Session, Query(q): Query<AuthQuery>) -> AppResult<Response> {
    let next = safe_next(q.next.as_deref()).unwrap_or("").to_string();
    if get_user_id(&session).await?.is_some() {
        let dest = if next.is_empty() { HOME_PATH } else { &next };
        return Ok(Redirect::to(dest).into_response());
    }
    Ok(HtmlTemplate(LoginTemplate {
        error: String::new(),
        email: String::new(),
        next,
    })
    .into_response())
}

async fn register_page(session: Session, Query(q): Query<AuthQuery>) -> AppResult<Response> {
    let next = safe_next(q.next.as_deref()).unwrap_or("").to_string();
    if get_user_id(&session).await?.is_some() {
        let dest = if next.is_empty() { HOME_PATH } else { &next };
        return Ok(Redirect::to(dest).into_response());
    }
    Ok(HtmlTemplate(RegisterTemplate {
        error: String::new(),
        email: String::new(),
        next,
    })
    .into_response())
}

async fn login_submit(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<LoginForm>,
) -> Response {
    let next = safe_next(form.next.as_deref())
        .unwrap_or(HOME_PATH)
        .to_string();
    match login_inner(&state, &session, &form).await {
        Ok(()) => hx_redirect(&headers, &next),
        Err(err) => auth_error_response(
            &headers,
            err.to_string(),
            &form.email,
            true,
            form.next.as_deref().unwrap_or(""),
        ),
    }
}

async fn login_inner(state: &AppState, session: &Session, form: &LoginForm) -> AppResult<()> {
    let email = validate_email(&form.email)?;
    if form.password.is_empty() {
        return Err(AppError::BadRequest("Enter your password.".into()));
    }

    let Some((user, password_hash)) = find_user_by_email(&state.pool, email).await? else {
        return Err(AppError::BadRequest("Invalid email or password.".into()));
    };

    if !verify_password(&form.password, &password_hash).await? {
        return Err(AppError::BadRequest("Invalid email or password.".into()));
    }

    // Renew session id to mitigate fixation, then store the authenticated user.
    session.cycle_id().await.map_err(|err| {
        AppError::Internal(format!("failed to renew session: {err}"))
    })?;
    set_user_id(session, user.uuid()?).await?;
    Ok(())
}

async fn register_submit(
    State(state): State<AppState>,
    session: Session,
    headers: HeaderMap,
    Form(form): Form<RegisterForm>,
) -> Response {
    let next = safe_next(form.next.as_deref())
        .unwrap_or(HOME_PATH)
        .to_string();
    match register_inner(&state, &session, &form).await {
        Ok(()) => hx_redirect(&headers, &next),
        Err(err) => auth_error_response(
            &headers,
            err.to_string(),
            &form.email,
            false,
            form.next.as_deref().unwrap_or(""),
        ),
    }
}

async fn register_inner(
    state: &AppState,
    session: &Session,
    form: &RegisterForm,
) -> AppResult<()> {
    let email = validate_email(&form.email)?;
    validate_password(&form.password)?;
    if form.password != form.confirm_password {
        return Err(AppError::BadRequest("Passwords do not match.".into()));
    }

    let user = create_user(&state.pool, email, &form.password).await?;
    session.cycle_id().await.map_err(|err| {
        AppError::Internal(format!("failed to renew session: {err}"))
    })?;
    set_user_id(session, user.uuid()?).await?;
    Ok(())
}

async fn logout(session: Session, headers: HeaderMap) -> AppResult<Response> {
    purge_session(&session).await?;
    Ok(hx_redirect(&headers, "/login"))
}

fn auth_error_response(
    headers: &HeaderMap,
    message: String,
    email: &str,
    is_login: bool,
    next: &str,
) -> Response {
    let next = safe_next(Some(next)).unwrap_or("").to_string();
    if is_htmx(headers) {
        (
            StatusCode::BAD_REQUEST,
            HtmlTemplate(AuthErrorPartial { message }),
        )
            .into_response()
    } else if is_login {
        (
            StatusCode::BAD_REQUEST,
            HtmlTemplate(LoginTemplate {
                error: message,
                email: email.to_string(),
                next,
            }),
        )
            .into_response()
    } else {
        (
            StatusCode::BAD_REQUEST,
            HtmlTemplate(RegisterTemplate {
                error: message,
                email: email.to_string(),
                next,
            }),
        )
            .into_response()
    }
}
