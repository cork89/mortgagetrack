use askama::Template;
use axum::{
    http::{header::HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};

use crate::models::{
    ChartPair, DashboardView, EmptyState, ExtraPayment, ImprovementRowView, MonthCell,
    PaymentRowView, PayoffAccelerator, ProfileOption, YearStat, YearSummary,
};

/// Wrap any Askama template so Axum handlers can return it as `text/html`.
pub struct HtmlTemplate<T>(pub T);

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(body) => Html(body).into_response(),
            Err(err) => {
                tracing::error!(error = %err, "failed to render template");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("Template error: {err}"),
                )
                    .into_response()
            }
        }
    }
}

/// HTML fragment plus an `HX-Trigger` event so the client can mark other tabs stale
/// and advance the optimistic-concurrency version after a successful write.
pub fn panel_update<T: Template>(
    template: T,
    keep_tab: &str,
    invalidate_chart: bool,
    version: i64,
) -> Response {
    let mut response = HtmlTemplate(template).into_response();
    let trigger = serde_json::json!({
        "markPanelsStale": {
            "keep": keep_tab,
            "invalidateChart": invalidate_chart,
            "version": version
        }
    });
    if let Ok(value) = HeaderValue::from_str(&trigger.to_string()) {
        response.headers_mut().insert(
            HeaderName::from_static("hx-trigger"),
            value,
        );
    }
    response
}

/// 409 conflict: refresh the dashboard and tell the client to show a message.
pub fn conflict_dashboard(template: DashboardTemplate, message: &str) -> Response {
    let mut response = HtmlTemplate(template).into_response();
    *response.status_mut() = StatusCode::CONFLICT;
    if let Ok(value) = HeaderValue::from_str(message) {
        response.headers_mut().insert(
            HeaderName::from_static("x-conflict-message"),
            value,
        );
    }
    response.headers_mut().insert(
        HeaderName::from_static("hx-retarget"),
        HeaderValue::from_static("#main-panel"),
    );
    response.headers_mut().insert(
        HeaderName::from_static("hx-reswap"),
        HeaderValue::from_static("innerHTML"),
    );
    response
}

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub csrf_token: String,
    pub profiles: Vec<ProfileOption>,
    pub has_profiles: bool,
    pub is_owner: bool,
    pub empty: EmptyState,
    pub dashboard: Option<DashboardView>,
    pub default_start: String,
    pub error: String,
    pub user_email: String,
    pub avatar_src: String,
}

#[derive(Template)]
#[template(path = "partials/profile_bar.html")]
#[allow(dead_code)]
pub struct ProfileBarTemplate {
    pub profiles: Vec<ProfileOption>,
    pub has_profiles: bool,
    pub is_owner: bool,
    pub dashboard: Option<DashboardView>,
}

#[derive(Debug, Clone)]
pub struct CollaboratorView {
    pub user_id: String,
    pub email: String,
}

#[derive(Template)]
#[template(path = "partials/share_panel.html")]
pub struct SharePanelTemplate {
    pub profile_id: String,
    pub is_owner: bool,
    pub has_active_invite: bool,
    pub active_expires_at: String,
    pub fresh_invite_url: String,
    pub collaborators: Vec<CollaboratorView>,
}

#[derive(Template)]
#[template(path = "partials/summary.html")]
pub struct SummaryTemplate {
    pub accelerator: PayoffAccelerator,
    pub year_stats: Vec<YearStat>,
}

#[derive(Template)]
#[template(path = "partials/calendar.html")]
pub struct CalendarTemplate {
    pub profile_id: String,
    pub view_year: i32,
    pub months: Vec<MonthCell>,
    pub payment_filter: String,
    pub grain: String,
}

#[derive(Template)]
#[template(path = "partials/payments.html")]
pub struct PaymentsTemplate {
    pub profile_id: String,
    pub payment_rows: Vec<PaymentRowView>,
    pub payment_filter: String,
    pub summary: YearSummary,
    pub extra_date_default: String,
    pub view_year: i32,
    pub grain: String,
}

#[derive(Template)]
#[template(path = "partials/improvements.html")]
pub struct ImprovementsTemplate {
    pub profile_id: String,
    pub extra_date_default: String,
    pub improvements: Vec<ImprovementRowView>,
    pub improvements_total: String,
}

#[derive(Template)]
#[template(path = "partials/chart.html")]
pub struct ChartTemplate {
    #[allow(dead_code)]
    pub profile_id: String,
    pub chart: ChartPair,
}

#[derive(Template)]
#[template(path = "partials/dashboard.html")]
pub struct DashboardTemplate {
    pub empty: EmptyState,
    pub dashboard: Option<DashboardView>,
}

#[derive(Template)]
#[template(path = "partials/extras_list.html")]
#[allow(dead_code)]
pub struct ExtrasListTemplate {
    pub extras: Vec<ExtraPayment>,
    pub profile_id: String,
}

#[derive(Template)]
#[template(path = "partials/error.html")]
pub struct ErrorPartial {
    pub message: String,
}

#[derive(Template)]
#[template(path = "landing.html")]
pub struct LandingTemplate {
    pub csrf_token: String,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub csrf_token: String,
    pub error: String,
    pub email: String,
    pub next: String,
    pub next_query: String,
    pub share_invite: bool,
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    pub csrf_token: String,
    pub error: String,
    pub email: String,
    pub next: String,
    pub next_query: String,
    pub share_invite: bool,
}

#[derive(Template)]
#[template(path = "partials/auth_error.html")]
pub struct AuthErrorPartial {
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct AvatarOption {
    pub id: String,
    pub selected: bool,
}

#[derive(Template)]
#[template(path = "settings.html")]
pub struct SettingsTemplate {
    pub csrf_token: String,
    pub email: String,
    pub avatar_src: String,
    pub avatar_options: Vec<AvatarOption>,
    pub avatar_updated: bool,
    pub avatar_error: bool,
    pub password_updated: bool,
    pub password_error: String,
    pub delete_error: String,
}
