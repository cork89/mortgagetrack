use axum::{
    extract::State,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Form, Json, Router,
};
use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use tower_sessions::Session;

use crate::app_state::AppState;
use crate::auth::{
    list_users_for_admin, paid_until_active, parse_paid_until, set_user_paid_until,
    trust_identity_headers, AdminUser, UserRole,
};
use crate::csrf;
use crate::error::{AppError, AppResult};
use crate::templates::{AdminUserView, AdminUsersTemplate, HtmlTemplate};

/// Far-future entitlement used by the admin "paid" switch (matches Worker UI).
const PAID_UNTIL_FOREVER: &str = "9999-12-31T23:59:59.000Z";

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin", get(admin_users_page))
        .route("/admin/access", post(admin_set_access))
        .route("/admin/ping", get(admin_ping))
}

#[derive(Serialize)]
struct PingResponse {
    ok: bool,
    role: &'static str,
    paid_until: Option<String>,
    paid: bool,
}

#[derive(Debug, Deserialize)]
struct AccessForm {
    user_id: String,
    /// "1" / "true" → grant forever; anything else → clear.
    paid: String,
}

async fn admin_ping(AdminUser(user): AdminUser) -> Json<PingResponse> {
    Json(PingResponse {
        ok: true,
        role: user.role.as_str(),
        paid_until: user.paid_until.map(|dt| dt.to_rfc3339()),
        paid: user.is_paid(),
    })
}

fn format_paid_until(dt: DateTime<Utc>) -> String {
    dt.format("%Y-%m-%d %H:%M UTC").to_string()
}

fn forever_paid_until() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(9999, 12, 31, 23, 59, 59)
        .single()
        .expect("valid far-future datetime")
}

async fn admin_users_page(
    State(state): State<AppState>,
    session: Session,
    AdminUser(viewer): AdminUser,
) -> AppResult<Response> {
    let csrf_token = csrf::ensure_token(&session).await?;
    let now = Utc::now();
    let rows = list_users_for_admin(&state.pool).await?;
    let users = rows
        .into_iter()
        .map(|row| {
            let role = UserRole::parse(&row.role);
            let is_admin = role == UserRole::Admin;
            let paid_until = parse_paid_until(row.paid_until.as_deref());
            let has_paid_entitlement = paid_until_active(paid_until, now);
            let (paid_until_label, paid_until_iso) = match paid_until {
                Some(dt) => (format_paid_until(dt), dt.to_rfc3339()),
                None => (String::new(), String::new()),
            };
            AdminUserView {
                id: row.id.clone(),
                email: row.email,
                is_admin,
                has_paid_entitlement,
                paid_until_label,
                paid_until_iso,
                is_self: row.id == viewer.id.to_string(),
            }
        })
        .collect();

    Ok(HtmlTemplate(AdminUsersTemplate {
        csrf_token,
        app_name: state.app_name.clone(),
        auth_edge: trust_identity_headers(),
        users,
    })
    .into_response())
}

/// Mirror paidUntil onto the domain `users` row (keeps local SQLite admin list in sync).
/// Source of truth for sessions remains Worker `POST /api/admin/access`.
async fn admin_set_access(
    AdminUser(_viewer): AdminUser,
    State(state): State<AppState>,
    Form(form): Form<AccessForm>,
) -> AppResult<Response> {
    let user_id = form.user_id.trim();
    if user_id.is_empty() {
        return Err(AppError::BadRequest("user_id is required".into()));
    }
    let mark_paid = matches!(form.paid.trim(), "1" | "true" | "on" | "yes");
    let paid_until = if mark_paid {
        Some(parse_paid_until(Some(PAID_UNTIL_FOREVER)).unwrap_or_else(forever_paid_until))
    } else {
        None
    };
    set_user_paid_until(&state.pool, user_id, paid_until).await?;
    Ok(Redirect::to("/admin").into_response())
}
