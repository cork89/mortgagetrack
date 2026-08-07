use axum::{
    extract::State,
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use tower_sessions::Session;

use crate::app_state::AppState;
use crate::auth::{
    list_users_for_admin, paid_until_active, parse_paid_until, AdminUser, UserRole,
};
use crate::csrf;
use crate::error::AppResult;
use crate::templates::{AdminUserView, AdminUsersTemplate, HtmlTemplate};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/admin", get(admin_users_page))
        .route("/admin/ping", get(admin_ping))
}

#[derive(Serialize)]
struct PingResponse {
    ok: bool,
    role: &'static str,
    paid_until: Option<String>,
    paid: bool,
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
            let is_paid = is_admin || paid_until_active(paid_until, now);
            let (paid_until_label, paid_until_iso) = match paid_until {
                Some(dt) => (format_paid_until(dt), dt.to_rfc3339()),
                None => (String::new(), String::new()),
            };
            AdminUserView {
                email: row.email,
                is_admin,
                is_paid,
                paid_until_label,
                paid_until_iso,
                is_self: row.id == viewer.id.to_string(),
            }
        })
        .collect();

    Ok(HtmlTemplate(AdminUsersTemplate {
        csrf_token,
        app_name: state.app_name.clone(),
        users,
    })
    .into_response())
}
