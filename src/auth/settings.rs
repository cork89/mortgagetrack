//! Settings: change password, avatar, default tab, and delete account.

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

use super::models::{
    delete_user, password_hash_for_user, update_avatar as set_user_avatar, update_default_tab,
    update_password, update_payments_year_expand, validate_password, verify_password,
};
use crate::app_state::AppState;
use crate::auth::{
    hx_redirect, is_htmx, purge_session, trust_identity_headers, AuthUser, HOME_PATH,
};
use crate::csrf;
use crate::error::{AppError, AppResult};
use crate::models::{PaymentsYearExpand, TabId};
use crate::templates::{
    AuthErrorPartial, AvatarOption, HtmlTemplate, SettingsTemplate, TabOption,
};

/// Selectable avatar ids (`static/avatars/{id}.webp`).
pub const AVATAR_OPTIONS: &[&str] = &[
    "barrow",
    "boat",
    "boot",
    "car",
    "dog",
    "hat",
    "iron",
    "thimble",
];

pub fn is_valid_avatar(avatar: &str) -> bool {
    AVATAR_OPTIONS.contains(&avatar)
}

/// Stable pick from the selector when the user has not chosen one yet.
pub fn default_avatar_id(user_id: &Uuid) -> &'static str {
    let bytes = user_id.as_bytes();
    let idx = u16::from_le_bytes([bytes[0], bytes[1]]) as usize % AVATAR_OPTIONS.len();
    AVATAR_OPTIONS[idx]
}

fn resolved_avatar_id(user_id: &Uuid, avatar: Option<&str>) -> &'static str {
    avatar
        .filter(|a| is_valid_avatar(a))
        .and_then(|id| AVATAR_OPTIONS.iter().copied().find(|&opt| opt == id))
        .unwrap_or_else(|| default_avatar_id(user_id))
}

/// Image URL for a user: chosen avatar, or a stable pick from the selector.
pub fn avatar_src(user_id: &Uuid, avatar: Option<&str>) -> String {
    format!("/static/avatars/{}.webp", resolved_avatar_id(user_id, avatar))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(settings_page))
        .route("/settings/password", post(change_password))
        .route("/settings/delete", post(delete_account))
        .route("/settings/avatar", post(update_avatar))
        .route("/settings/default-tab", post(update_default_tab_handler))
        .route(
            "/settings/payments-year-expand",
            post(update_payments_year_expand_handler),
        )
        .route("/avatars/{id}", get(avatar))
}

#[derive(Debug, Deserialize)]
pub struct SettingsQuery {
    pub password: Option<String>,
    pub avatar: Option<String>,
    pub tab: Option<String>,
    pub years: Option<String>,
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

fn settings_template(
    user: &AuthUser,
    csrf_token: String,
    app_name: String,
    q: &SettingsQuery,
) -> SettingsTemplate {
    let current = resolved_avatar_id(&user.id, user.avatar.as_deref());
    let current_tab = TabId::parse(&user.default_tab);
    let current_year_expand = PaymentsYearExpand::parse(&user.payments_year_expand);
    SettingsTemplate {
        csrf_token,
        app_name,
        email: user.email.clone(),
        avatar_src: avatar_src(&user.id, user.avatar.as_deref()),
        avatar_options: AVATAR_OPTIONS
            .iter()
            .map(|id| AvatarOption {
                id: (*id).to_string(),
                selected: *id == current,
            })
            .collect(),
        tab_options: TabId::ALL
            .iter()
            .map(|tab| TabOption {
                id: tab.as_str().to_string(),
                label: tab.label().to_string(),
                selected: *tab == current_tab,
            })
            .collect(),
        year_expand_options: PaymentsYearExpand::ALL
            .iter()
            .map(|mode| TabOption {
                id: mode.as_str().to_string(),
                label: mode.label().to_string(),
                selected: *mode == current_year_expand,
            })
            .collect(),
        avatar_updated: q.avatar.as_deref() == Some("updated"),
        avatar_error: q.avatar.as_deref() == Some("error"),
        default_tab_updated: q.tab.as_deref() == Some("updated"),
        default_tab_error: q.tab.as_deref() == Some("error"),
        year_expand_updated: q.years.as_deref() == Some("updated"),
        year_expand_error: q.years.as_deref() == Some("error"),
        password_updated: q.password.as_deref() == Some("updated"),
        password_error: String::new(),
        delete_error: String::new(),
        auth_edge: trust_identity_headers(),
        is_admin: user.is_admin(),
    }
}

async fn settings_page(
    State(state): State<AppState>,
    session: Session,
    user: AuthUser,
    Query(q): Query<SettingsQuery>,
) -> AppResult<Response> {
    let csrf_token = csrf::ensure_token(&session).await?;
    Ok(HtmlTemplate(settings_template(
        &user,
        csrf_token,
        state.app_name.clone(),
        &q,
    ))
    .into_response())
}

async fn avatar(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Response> {
    let id_str = id.strip_suffix(".svg").unwrap_or(&id);
    let user_id = Uuid::parse_str(id_str)
        .map_err(|_| AppError::NotFound("Avatar not found.".into()))?;

    let stored = match crate::auth::models::find_user_by_id(&state.pool, user_id).await {
        Ok(Some(user)) => user.avatar,
        _ => None,
    };
    let redirect_url = avatar_src(&user_id, stored.as_deref());
    let mut response = (StatusCode::FOUND, [(header::LOCATION, redirect_url)]).into_response();
    // Avatars can change; never mark this URL immutable.
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    Ok(response)
}

#[derive(Debug, Deserialize)]
pub struct UpdateAvatarForm {
    pub avatar: String,
}

async fn update_avatar(
    State(state): State<AppState>,
    _session: Session,
    user: AuthUser,
    headers: HeaderMap,
    Form(form): Form<UpdateAvatarForm>,
) -> Response {
    if !is_valid_avatar(&form.avatar) {
        return hx_redirect(&headers, "/settings?avatar=error");
    }
    if let Err(err) = set_user_avatar(&state.pool, user.id, &form.avatar).await {
        tracing::error!("Failed to update avatar: {}", err);
        return hx_redirect(&headers, "/settings?avatar=error");
    }
    hx_redirect(&headers, "/settings?avatar=updated")
}

#[derive(Debug, Deserialize)]
pub struct UpdateDefaultTabForm {
    pub default_tab: String,
}

async fn update_default_tab_handler(
    State(state): State<AppState>,
    _session: Session,
    user: AuthUser,
    headers: HeaderMap,
    Form(form): Form<UpdateDefaultTabForm>,
) -> Response {
    let Some(tab) = TabId::try_parse(&form.default_tab) else {
        return hx_redirect(&headers, "/settings?tab=error");
    };
    if let Err(err) = update_default_tab(&state.pool, user.id, tab.as_str()).await {
        tracing::error!("Failed to update default tab: {}", err);
        return hx_redirect(&headers, "/settings?tab=error");
    }
    hx_redirect(&headers, "/settings?tab=updated")
}

#[derive(Debug, Deserialize)]
pub struct UpdatePaymentsYearExpandForm {
    pub payments_year_expand: String,
}

async fn update_payments_year_expand_handler(
    State(state): State<AppState>,
    _session: Session,
    user: AuthUser,
    headers: HeaderMap,
    Form(form): Form<UpdatePaymentsYearExpandForm>,
) -> Response {
    let Some(mode) = PaymentsYearExpand::try_parse(&form.payments_year_expand) else {
        return hx_redirect(&headers, "/settings?years=error");
    };
    if let Err(err) = update_payments_year_expand(&state.pool, user.id, mode.as_str()).await {
        tracing::error!("Failed to update payments year expand: {}", err);
        return hx_redirect(&headers, "/settings?years=error");
    }
    hx_redirect(&headers, "/settings?years=updated")
}

async fn change_password(
    State(state): State<AppState>,
    session: Session,
    user: AuthUser,
    headers: HeaderMap,
    Form(form): Form<ChangePasswordForm>,
) -> Response {
    if trust_identity_headers() {
        return (
            StatusCode::GONE,
            "Password changes are handled by Better Auth at /api/auth/change-password.",
        )
            .into_response();
    }
    match change_password_inner(&state, &session, &user, &form).await {
        Ok(()) => hx_redirect(&headers, "/settings?password=updated"),
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
                let mut page = settings_template(
                    &user,
                    csrf_token,
                    state.app_name.clone(),
                    &SettingsQuery {
                        password: None,
                        avatar: None,
                        tab: None,
                        years: None,
                    },
                );
                page.password_error = message;
                (StatusCode::BAD_REQUEST, HtmlTemplate(page)).into_response()
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
    if trust_identity_headers() {
        return (
            StatusCode::GONE,
            "Account deletion is handled by Better Auth at /api/auth/delete-user.",
        )
            .into_response();
    }
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
                let mut page = settings_template(
                    &user,
                    csrf_token,
                    state.app_name.clone(),
                    &SettingsQuery {
                        password: None,
                        avatar: None,
                        tab: None,
                        years: None,
                    },
                );
                page.delete_error = message;
                (StatusCode::BAD_REQUEST, HtmlTemplate(page)).into_response()
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
