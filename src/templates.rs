use askama::Template;
use axum::{
    http::{header::HeaderName, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};

use crate::models::{
    ChartPair, DashboardView, EmptyState, ExtraPayment, MonthCell, PaymentChip, PaymentRowView,
    ProfileOption, YearStat, YearSummary,
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

/// HTML fragment plus an `HX-Trigger` event so the client can mark other tabs stale.
pub fn panel_update<T: Template>(
    template: T,
    keep_tab: &str,
    invalidate_chart: bool,
) -> Response {
    let mut response = HtmlTemplate(template).into_response();
    let trigger = serde_json::json!({
        "markPanelsStale": {
            "keep": keep_tab,
            "invalidateChart": invalidate_chart
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

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub profiles: Vec<ProfileOption>,
    pub has_profiles: bool,
    pub empty: EmptyState,
    pub dashboard: Option<DashboardView>,
    pub default_start: String,
    pub error: String,
    pub user_email: String,
}

#[derive(Template)]
#[template(path = "partials/item_list.html")]
pub struct ItemListTemplate {
    pub dashboard: DashboardView,
}

#[derive(Template)]
#[template(path = "partials/item_card.html")]
#[allow(dead_code)]
pub struct ItemCardTemplate {
    pub chip: PaymentChip,
    pub profile_id: String,
    pub tab: String,
    pub year: i32,
    pub filter: String,
    pub grain: String,
}

#[derive(Template)]
#[template(path = "partials/profile_bar.html")]
#[allow(dead_code)]
pub struct ProfileBarTemplate {
    pub profiles: Vec<ProfileOption>,
    pub has_profiles: bool,
    pub dashboard: Option<DashboardView>,
}

#[derive(Template)]
#[template(path = "partials/year_strip.html")]
#[allow(dead_code)]
pub struct YearStripTemplate {
    pub year_stats: Vec<YearStat>,
}

#[derive(Template)]
#[template(path = "partials/calendar.html")]
pub struct CalendarTemplate {
    pub profile_id: String,
    pub view_year: i32,
    pub months: Vec<MonthCell>,
    /// When set, emitted as an HTMX out-of-band swap for `#yearStrip`.
    pub year_stats: Option<Vec<YearStat>>,
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
    /// When set, emitted as an HTMX out-of-band swap for `#yearStrip`.
    pub year_stats: Option<Vec<YearStat>>,
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
#[template(path = "partials/panels_sync.html")]
#[allow(dead_code)]
pub struct PanelsSyncTemplate {
    pub dashboard: DashboardView,
}

#[derive(Template)]
#[template(path = "partials/error.html")]
pub struct ErrorPartial {
    pub message: String,
}

#[derive(Template)]
#[template(path = "login.html")]
pub struct LoginTemplate {
    pub error: String,
    pub email: String,
}

#[derive(Template)]
#[template(path = "register.html")]
pub struct RegisterTemplate {
    pub error: String,
    pub email: String,
}

#[derive(Template)]
#[template(path = "partials/auth_error.html")]
pub struct AuthErrorPartial {
    pub message: String,
}
