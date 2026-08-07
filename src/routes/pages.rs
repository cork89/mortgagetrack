use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, post},
    Form, Router,
};
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;
use tower_sessions::Session;

use crate::app_state::AppState;
use crate::auth::{avatar_src, current_user, hx_redirect, is_htmx, AuthUser};
use crate::csrf;
use crate::error::{AppError, AppResult};
use crate::models::{
    add_extra, add_improvement, build_dashboard, clear_paid, create_profile, delete_extra,
    delete_improvement, delete_profile, empty_state, list_extras, list_paid_keys, load_page_bundle,
    load_profile, mark_due_paid, rename_profile, set_active_profile, set_paid, update_improvement,
    update_profile_loan, upsert_payment_note, PaymentFilter, PaymentsYearExpand, ProfileOption,
    TabId,
};
use crate::templates::{
    panel_trigger, panel_update, CalendarTemplate, ChartTemplate, DashboardTemplate, ErrorPartial,
    HtmlTemplate, ImprovementsTemplate, IndexTemplate, LandingTemplate, PaymentsTemplate,
    SummaryTemplate,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/partials/dashboard", get(dashboard_partial))
        .route("/partials/summary", get(summary_partial))
        .route("/partials/calendar", get(calendar_partial))
        .route("/partials/payments", get(payments_partial))
        .route("/partials/improvements", get(improvements_partial))
        .route("/partials/chart", get(chart_partial))
        .route("/profiles", post(create_profile_handler))
        .route("/profiles/switch", post(switch_profile))
        .route("/profiles/{id}", post(update_profile_handler))
        .route("/profiles/{id}/rename", post(rename_profile_handler))
        .route("/profiles/{id}/delete", post(delete_profile_handler))
        .route("/profiles/{id}/clear-paid", post(clear_paid_handler))
        .route("/profiles/{id}/mark-due", post(mark_due_handler))
        .route("/profiles/{id}/toggle-paid", post(toggle_paid_handler))
        .route("/profiles/{id}/notes", post(upsert_note_handler))
        .route("/profiles/{id}/extras", post(add_extra_handler))
        .route(
            "/profiles/{id}/extras/{extra_id}",
            delete(delete_extra_handler).post(delete_extra_handler),
        )
        .route("/profiles/{id}/improvements", post(add_improvement_handler))
        .route(
            "/profiles/{id}/improvements/{improvement_id}",
            delete(delete_improvement_handler).post(delete_improvement_handler),
        )
        .route(
            "/profiles/{id}/improvements/{improvement_id}/update",
            post(update_improvement_handler),
        )
}

#[derive(Debug, Deserialize)]
pub struct IndexQuery {
    pub tab: Option<String>,
    pub year: Option<i32>,
    pub filter: Option<String>,
    pub grain: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ProfileForm {
    pub name: String,
    pub principal: f64,
    pub rate: f64,
    pub term: i32,
    pub start_date: String,
}

#[derive(Debug, Deserialize)]
pub struct RenameForm {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct SwitchForm {
    pub profile_id: String,
    pub tab: Option<String>,
    pub year: Option<i32>,
    pub filter: Option<String>,
    pub grain: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ToggleForm {
    pub pay_key: String,
    pub paid: bool,
    pub year: Option<i32>,
    pub filter: Option<String>,
    pub grain: Option<String>,
    pub tab: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExtraForm {
    pub date: String,
    pub amount: f64,
    pub filter: Option<String>,
    #[serde(default)]
    pub recast: bool,
}

#[derive(Debug, Deserialize)]
pub struct ImprovementForm {
    pub date: String,
    pub amount: f64,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateImprovementForm {
    pub date: String,
    pub amount: f64,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Deserialize)]
pub struct NoteForm {
    pub pay_key: String,
    pub note: String,
    pub year: Option<i32>,
    pub filter: Option<String>,
    pub grain: Option<String>,
}

async fn index(
    session: Session,
    headers: HeaderMap,
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<Response> {
    let csrf_token = csrf::ensure_token(&session).await?;
    let Some(user) = current_user(&headers, &session, &state.pool).await? else {
        return Ok(HtmlTemplate(LandingTemplate {
            csrf_token,
            app_name: state.app_name.clone(),
        })
        .into_response());
    };
    let mut page = load_page(&state, &q, &user).await?;
    page.csrf_token = csrf_token;
    Ok(HtmlTemplate(page).into_response())
}

async fn dashboard_partial(
    user: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let page = load_page(&state, &q, &user).await?;
    Ok(HtmlTemplate(DashboardTemplate {
        empty: page.empty,
        dashboard: page.dashboard,
    }))
}

async fn summary_partial(
    user: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let page = load_page(&state, &q, &user).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(SummaryTemplate {
        accelerator: d.accelerator,
        year_stats: d.year_stats,
    }))
}

async fn calendar_partial(
    user: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let page = load_page(&state, &q, &user).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(calendar_from_dashboard(d)))
}

async fn payments_partial(
    user: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let page = load_page(&state, &q, &user).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(payments_from_dashboard(d)))
}

async fn improvements_partial(
    user: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let page = load_page(&state, &q, &user).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(improvements_from_dashboard(d)))
}

async fn chart_partial(
    user: AuthUser,
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let page = load_page(&state, &q, &user).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(ChartTemplate {
        profile_id: d.profile_id,
        chart: d.chart,
    }))
}

async fn create_profile_handler(
    user: AuthUser,
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<ProfileForm>,
) -> Response {
    match create_profile_inner(&state, user, form).await {
        Ok(_) => hx_redirect(&headers, "/"),
        Err(err) => HtmlTemplate(ErrorPartial {
            message: err.to_string(),
        })
        .into_response(),
    }
}

async fn create_profile_inner(
    state: &AppState,
    user: AuthUser,
    form: ProfileForm,
) -> AppResult<()> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Enter a profile name.".into()));
    }
    let start = parse_date(&form.start_date)?;
    validate_loan(form.principal, form.rate, form.term)?;
    create_profile(
        &state.pool,
        user.id,
        name,
        form.principal,
        form.rate,
        form.term,
        start,
    )
    .await?;
    Ok(())
}

async fn update_profile_handler(
    user: AuthUser,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<ProfileForm>,
) -> Response {
    match update_profile_inner(&state, user, &id, form).await {
        Ok(_) => hx_redirect(&headers, "/"),
        Err(err) => HtmlTemplate(ErrorPartial {
            message: err.to_string(),
        })
        .into_response(),
    }
}

async fn update_profile_inner(
    state: &AppState,
    user: AuthUser,
    id: &str,
    form: ProfileForm,
) -> AppResult<()> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Enter a profile name.".into()));
    }
    let start = parse_date(&form.start_date)?;
    validate_loan(form.principal, form.rate, form.term)?;
    update_profile_loan(
        &state.pool,
        user.id,
        id,
        name,
        form.principal,
        form.rate,
        form.term,
        start,
    )
    .await?;
    set_active_profile(&state.pool, user.id, Some(id)).await?;
    Ok(())
}

async fn rename_profile_handler(
    user: AuthUser,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<RenameForm>,
) -> Response {
    let name = form.name.trim();
    if name.is_empty() {
        return HtmlTemplate(ErrorPartial {
            message: "Enter a profile name.".into(),
        })
        .into_response();
    }
    match rename_profile(&state.pool, user.id, &id, name).await {
        Ok(_) => hx_redirect(&headers, "/"),
        Err(err) => HtmlTemplate(ErrorPartial {
            message: err.to_string(),
        })
        .into_response(),
    }
}

async fn delete_profile_handler(
    user: AuthUser,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Response> {
    delete_profile(&state.pool, user.id, &id).await?;
    Ok(hx_redirect(&headers, "/"))
}

async fn switch_profile(
    user: AuthUser,
    State(state): State<AppState>,
    Form(form): Form<SwitchForm>,
) -> AppResult<impl IntoResponse> {
    set_active_profile(&state.pool, user.id, Some(&form.profile_id)).await?;
    let page = load_page(
        &state,
        &IndexQuery {
            tab: form.tab,
            year: form.year,
            filter: form.filter,
            grain: form.grain,
        },
        &user,
    )
    .await?;
    Ok(HtmlTemplate(DashboardTemplate {
        empty: page.empty,
        dashboard: page.dashboard,
    }))
}

async fn clear_paid_handler(
    user: AuthUser,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Response> {
    clear_paid(&state.pool, user.id, &id).await?;
    if is_htmx(&headers) {
        Ok(hx_redirect(&headers, "/"))
    } else {
        Ok(Redirect::to("/").into_response())
    }
}

async fn mark_due_handler(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<IndexQuery>,
) -> AppResult<Response> {
    let profile = load_profile(&state.pool, user.id, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("Profile not found".into()))?;
    let extras = list_extras(&state.pool, &id).await?;
    let paid: std::collections::HashSet<String> = list_paid_keys(&state.pool, &id)
        .await?
        .into_iter()
        .collect();
    let today = state.today();

    let loan = profile
        .loan()
        .ok_or_else(|| AppError::BadRequest("No loan".into()))?;
    let extra_inputs = crate::models::extras_as_inputs(&extras, &paid);
    let built = crate::models::build_schedule(
        loan.principal,
        loan.rate,
        loan.term_years,
        loan.start_date,
        &extra_inputs,
    );
    let keys: Vec<String> = built
        .rows
        .iter()
        .filter(|r| {
            r.due <= today
                || (r.due.year() == today.year() && r.due.month() == today.month())
        })
        .map(|r| r.pay_key.clone())
        .collect();
    mark_due_paid(&state.pool, user.id, &id, &keys).await?;
    let page = load_page(&state, &q, &user).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(panel_update(payments_from_dashboard(d), "payments", true))
}

async fn toggle_paid_handler(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<ToggleForm>,
) -> AppResult<Response> {
    let tab = form.tab.clone().unwrap_or_else(|| "calendar".into());
    let q = IndexQuery {
        tab: form.tab,
        year: form.year,
        filter: form.filter,
        grain: form.grain,
    };
    set_paid(&state.pool, user.id, &id, &form.pay_key, form.paid).await?;
    let is_extra = form.pay_key.starts_with("extra:");
    // Payments tab: always refresh so total balance / interest summary stay in sync.
    // Extras/recasts also change amortization — refresh whichever panel is active.
    if tab == "payments" || is_extra {
        let page = load_page(&state, &q, &user).await?;
        let d = page
            .dashboard
            .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
        return if tab == "payments" {
            Ok(panel_update(
                payments_from_dashboard(d),
                "payments",
                is_extra,
            ))
        } else {
            Ok(panel_update(calendar_from_dashboard(d), "calendar", true))
        };
    }
    Ok(panel_trigger("calendar", false))
}

async fn upsert_note_handler(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<NoteForm>,
) -> AppResult<Response> {
    let q = IndexQuery {
        tab: Some("payments".into()),
        year: form.year,
        filter: form.filter,
        grain: form.grain,
    };
    upsert_payment_note(&state.pool, user.id, &id, &form.pay_key, &form.note).await?;
    let page = load_page(&state, &q, &user).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(panel_update(payments_from_dashboard(d), "payments", false))
}

async fn add_extra_handler(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<ExtraForm>,
) -> AppResult<Response> {
    let date = parse_date(&form.date)?;
    let q = IndexQuery {
        tab: Some("payments".into()),
        year: None,
        filter: form.filter,
        grain: None,
    };
    add_extra(
        &state.pool,
        user.id,
        &id,
        date,
        form.amount,
        form.recast,
    )
    .await?;
    let page = load_page(&state, &q, &user).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(panel_update(payments_from_dashboard(d), "payments", false))
}

async fn delete_extra_handler(
    user: AuthUser,
    State(state): State<AppState>,
    Path((id, extra_id)): Path<(String, String)>,
) -> AppResult<Response> {
    let q = IndexQuery {
        tab: Some("payments".into()),
        year: None,
        filter: None,
        grain: None,
    };
    delete_extra(&state.pool, user.id, &id, &extra_id).await?;
    let page = load_page(&state, &q, &user).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(panel_update(payments_from_dashboard(d), "payments", true))
}

async fn add_improvement_handler(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<ImprovementForm>,
) -> AppResult<Response> {
    let date = parse_date(&form.date)?;
    let q = IndexQuery {
        tab: Some("improvements".into()),
        year: None,
        filter: None,
        grain: None,
    };
    add_improvement(
        &state.pool,
        user.id,
        &id,
        date,
        form.amount,
        &form.note,
        &form.detail,
    )
    .await?;
    let page = load_page(&state, &q, &user).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(panel_update(
        improvements_from_dashboard(d),
        "improvements",
        false,
    ))
}

async fn delete_improvement_handler(
    user: AuthUser,
    State(state): State<AppState>,
    Path((id, improvement_id)): Path<(String, String)>,
) -> AppResult<Response> {
    let q = IndexQuery {
        tab: Some("improvements".into()),
        year: None,
        filter: None,
        grain: None,
    };
    delete_improvement(&state.pool, user.id, &id, &improvement_id).await?;
    let page = load_page(&state, &q, &user).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(panel_update(
        improvements_from_dashboard(d),
        "improvements",
        false,
    ))
}

async fn update_improvement_handler(
    user: AuthUser,
    State(state): State<AppState>,
    Path((id, improvement_id)): Path<(String, String)>,
    Form(form): Form<UpdateImprovementForm>,
) -> AppResult<Response> {
    let date = parse_date(&form.date)?;
    let q = IndexQuery {
        tab: Some("improvements".into()),
        year: None,
        filter: None,
        grain: None,
    };
    update_improvement(
        &state.pool,
        user.id,
        &id,
        &improvement_id,
        date,
        form.amount,
        &form.note,
        &form.detail,
    )
    .await?;
    let page = load_page(&state, &q, &user).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(panel_update(
        improvements_from_dashboard(d),
        "improvements",
        false,
    ))
}

async fn load_page(
    state: &AppState,
    q: &IndexQuery,
    user: &AuthUser,
) -> AppResult<IndexTemplate> {
    let bundle = load_page_bundle(&state.pool, user.id).await?;
    let profiles = bundle.profiles;
    let active = bundle.active;
    let active_id = active.as_ref().map(|p| p.id.clone());

    let today = state.today();
    let view_year = q.year.unwrap_or_else(|| today.year());
    let filter = PaymentFilter::parse(q.filter.as_deref().unwrap_or("all"));
    let grain = q.grain.as_deref().unwrap_or("monthly");
    let tab = TabId::parse(
        q.tab
            .as_deref()
            .unwrap_or_else(|| user.default_tab.as_str()),
    );

    let dashboard = if let Some(profile) = &active {
        if profile.has_loan() {
            let notes = bundle
                .notes
                .into_iter()
                .map(|n| (n.pay_key, n.note))
                .collect();
            build_dashboard(
                profile,
                &bundle.paid,
                &bundle.extras,
                &notes,
                &bundle.improvements,
                view_year,
                filter,
                grain,
                tab,
                today,
                PaymentsYearExpand::parse(&user.payments_year_expand),
            )
        } else {
            None
        }
    } else {
        None
    };

    let user_key = user.id.to_string();
    let profile_opts: Vec<ProfileOption> = profiles
        .iter()
        .map(|p| ProfileOption {
            id: p.id.clone(),
            name: p.name.clone(),
            selected: active_id.as_deref() == Some(p.id.as_str()),
            is_shared: p.user_id != user_key,
        })
        .collect();

    let is_owner = match &active {
        Some(p) => p.user_id == user_key,
        None => true,
    };

    let default_start = {
        let d = today;
        let month = if d.month() == 12 {
            1
        } else {
            d.month() + 1
        };
        let year = if d.month() == 12 {
            d.year() + 1
        } else {
            d.year()
        };
        NaiveDate::from_ymd_opt(year, month, 1)
            .unwrap_or(d)
            .format("%Y-%m-%d")
            .to_string()
    };

    Ok(IndexTemplate {
        csrf_token: String::new(),
        app_name: state.app_name.clone(),
        has_profiles: !profiles.is_empty(),
        profiles: profile_opts,
        is_owner,
        empty: empty_state(active.as_ref()),
        dashboard,
        default_start,
        error: String::new(),
        user_email: user.email.clone(),
        avatar_src: avatar_src(&user.id, user.avatar.as_deref()),
    })
}

fn parse_date(s: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("Invalid date".into()))
}

fn validate_loan(principal: f64, rate: f64, term: i32) -> AppResult<()> {
    if !(principal > 0.0) || !(term > 0) || rate < 0.0 {
        return Err(AppError::BadRequest(
            "Enter a valid loan amount, rate (≥ 0), term, and first payment date.".into(),
        ));
    }
    Ok(())
}

fn calendar_from_dashboard(d: crate::models::DashboardView) -> CalendarTemplate {
    CalendarTemplate {
        profile_id: d.profile_id,
        view_year: d.view_year,
        months: d.months,
        payment_filter: d.payment_filter,
        grain: d.chart.grain,
    }
}

fn payments_from_dashboard(d: crate::models::DashboardView) -> PaymentsTemplate {
    PaymentsTemplate {
        profile_id: d.profile_id,
        payment_years: d.payment_years,
        payments_year_expand: d.payments_year_expand,
        payment_filter: d.payment_filter,
        summary: d.summary,
        extra_date_default: d.extra_date_default,
        view_year: d.view_year,
        grain: d.chart.grain,
    }
}

fn improvements_from_dashboard(d: crate::models::DashboardView) -> ImprovementsTemplate {
    ImprovementsTemplate {
        profile_id: d.profile_id,
        extra_date_default: d.extra_date_default,
        improvements: d.improvements,
        improvements_total: d.improvements_total,
    }
}
