use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Redirect, Response},
    routing::{delete, get, post},
    Form, Router,
};
use chrono::{Datelike, NaiveDate};
use serde::Deserialize;

use crate::app_state::AppState;
use crate::error::{AppError, AppResult};
use crate::models::{
    add_extra, build_dashboard, clear_paid, create_profile, delete_extra, delete_profile,
    empty_state, get_active_profile_id, list_extras, list_paid_keys, list_profiles, load_profile,
    mark_due_paid, rename_profile, set_active_profile, toggle_paid, update_profile_loan,
    PaymentFilter, ProfileOption, TabId,
};
use crate::templates::{
    CalendarTemplate, ChartTemplate, DashboardTemplate, ErrorPartial, HtmlTemplate, IndexTemplate,
    ItemListTemplate, PanelsSyncTemplate, PaymentsTemplate,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(index))
        .route("/partials/dashboard", get(dashboard_partial))
        .route("/partials/calendar", get(calendar_partial))
        .route("/partials/payments", get(payments_partial))
        .route("/partials/chart", get(chart_partial))
        .route("/partials/items", get(item_list_partial))
        .route("/profiles", post(create_profile_handler))
        .route("/profiles/switch", post(switch_profile))
        .route("/profiles/{id}", post(update_profile_handler))
        .route("/profiles/{id}/rename", post(rename_profile_handler))
        .route("/profiles/{id}/delete", post(delete_profile_handler))
        .route("/profiles/{id}/clear-paid", post(clear_paid_handler))
        .route("/profiles/{id}/mark-due", post(mark_due_handler))
        .route("/profiles/{id}/toggle-paid", post(toggle_paid_handler))
        .route("/profiles/{id}/extras", post(add_extra_handler))
        .route(
            "/profiles/{id}/extras/{extra_id}",
            delete(delete_extra_handler).post(delete_extra_handler),
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
}

#[derive(Debug, Deserialize)]
pub struct ToggleForm {
    pub pay_key: String,
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
}

async fn index(
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let page = load_page(&state, &q).await?;
    Ok(HtmlTemplate(page))
}

async fn dashboard_partial(
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let page = load_page(&state, &q).await?;
    Ok(HtmlTemplate(DashboardTemplate {
        empty: page.empty,
        dashboard: page.dashboard,
    }))
}

async fn item_list_partial(
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let page = load_page(&state, &q).await?;
    let dashboard = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(ItemListTemplate { dashboard }))
}

async fn calendar_partial(
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let page = load_page(&state, &q).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(calendar_from_dashboard(d, false)))
}

async fn payments_partial(
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let page = load_page(&state, &q).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(payments_from_dashboard(d, false)))
}

async fn chart_partial(
    State(state): State<AppState>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let page = load_page(&state, &q).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(ChartTemplate {
        profile_id: d.profile_id,
        chart: d.chart,
    }))
}

async fn create_profile_handler(
    State(state): State<AppState>,
    Form(form): Form<ProfileForm>,
) -> Response {
    match create_profile_inner(&state, form).await {
        Ok(_) => Redirect::to("/").into_response(),
        Err(err) => HtmlTemplate(ErrorPartial {
            message: err.to_string(),
        })
        .into_response(),
    }
}

async fn create_profile_inner(state: &AppState, form: ProfileForm) -> AppResult<()> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Enter a profile name.".into()));
    }
    let start = parse_date(&form.start_date)?;
    validate_loan(form.principal, form.rate, form.term)?;
    create_profile(
        &state.pool,
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
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<ProfileForm>,
) -> Response {
    match update_profile_inner(&state, &id, form).await {
        Ok(_) => Redirect::to("/").into_response(),
        Err(err) => HtmlTemplate(ErrorPartial {
            message: err.to_string(),
        })
        .into_response(),
    }
}

async fn update_profile_inner(state: &AppState, id: &str, form: ProfileForm) -> AppResult<()> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(AppError::BadRequest("Enter a profile name.".into()));
    }
    let start = parse_date(&form.start_date)?;
    validate_loan(form.principal, form.rate, form.term)?;
    update_profile_loan(
        &state.pool,
        id,
        name,
        form.principal,
        form.rate,
        form.term,
        start,
    )
    .await?;
    set_active_profile(&state.pool, Some(id)).await?;
    Ok(())
}

async fn rename_profile_handler(
    State(state): State<AppState>,
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
    match rename_profile(&state.pool, &id, name).await {
        Ok(_) => Redirect::to("/").into_response(),
        Err(err) => HtmlTemplate(ErrorPartial {
            message: err.to_string(),
        })
        .into_response(),
    }
}

async fn delete_profile_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Redirect> {
    delete_profile(&state.pool, &id).await?;
    Ok(Redirect::to("/"))
}

async fn switch_profile(
    State(state): State<AppState>,
    Form(form): Form<SwitchForm>,
) -> AppResult<impl IntoResponse> {
    set_active_profile(&state.pool, Some(&form.profile_id)).await?;
    let page = load_page(
        &state,
        &IndexQuery {
            tab: None,
            year: None,
            filter: None,
            grain: None,
        },
    )
    .await?;
    Ok(HtmlTemplate(DashboardTemplate {
        empty: page.empty,
        dashboard: page.dashboard,
    }))
}

async fn clear_paid_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<Redirect> {
    clear_paid(&state.pool, &id).await?;
    Ok(Redirect::to("/"))
}

async fn mark_due_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<IndexQuery>,
) -> AppResult<impl IntoResponse> {
    let profile = load_profile(&state.pool, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("Profile not found".into()))?;
    let extras = list_extras(&state.pool, &id).await?;
    let today = chrono::Local::now().date_naive();

    let loan = profile
        .loan()
        .ok_or_else(|| AppError::BadRequest("No loan".into()))?;
    let extra_tuples: Vec<(String, NaiveDate, f64)> = extras
        .iter()
        .filter_map(|ex| {
            let date = NaiveDate::parse_from_str(&ex.date, "%Y-%m-%d").ok()?;
            Some((ex.id.clone(), date, ex.amount))
        })
        .collect();
    let built = crate::models::build_schedule(
        loan.principal,
        loan.rate,
        loan.term_years,
        loan.start_date,
        &extra_tuples,
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
    mark_due_paid(&state.pool, &id, &keys).await?;

    let page = load_page(&state, &q).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(PanelsSyncTemplate { dashboard: d }))
}

async fn toggle_paid_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<ToggleForm>,
) -> AppResult<impl IntoResponse> {
    toggle_paid(&state.pool, &id, &form.pay_key).await?;
    let q = IndexQuery {
        tab: form.tab.clone(),
        year: form.year,
        filter: form.filter,
        grain: form.grain,
    };
    let page = load_page(&state, &q).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(PanelsSyncTemplate { dashboard: d }))
}

async fn add_extra_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Form(form): Form<ExtraForm>,
) -> AppResult<impl IntoResponse> {
    let date = parse_date(&form.date)?;
    add_extra(&state.pool, &id, date, form.amount).await?;
    let q = IndexQuery {
        tab: Some("payments".into()),
        year: None,
        filter: form.filter,
        grain: None,
    };
    let page = load_page(&state, &q).await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(PanelsSyncTemplate { dashboard: d }))
}

async fn delete_extra_handler(
    State(state): State<AppState>,
    Path((id, extra_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    delete_extra(&state.pool, &id, &extra_id).await?;
    let page = load_page(
        &state,
        &IndexQuery {
            tab: Some("payments".into()),
            year: None,
            filter: None,
            grain: None,
        },
    )
    .await?;
    let d = page
        .dashboard
        .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
    Ok(HtmlTemplate(PanelsSyncTemplate { dashboard: d }))
}

async fn load_page(state: &AppState, q: &IndexQuery) -> AppResult<IndexTemplate> {
    let profiles = list_profiles(&state.pool).await?;
    let active_id = get_active_profile_id(&state.pool).await?;
    let active = match &active_id {
        Some(id) => load_profile(&state.pool, id).await?,
        None => None,
    };

    let today = chrono::Local::now().date_naive();
    let view_year = q.year.unwrap_or_else(|| today.year());
    let filter = PaymentFilter::parse(q.filter.as_deref().unwrap_or("all"));
    let grain = q.grain.as_deref().unwrap_or("monthly");
    let tab = TabId::parse(q.tab.as_deref().unwrap_or("calendar"));

    let dashboard = if let Some(profile) = &active {
        if profile.has_loan() {
            let paid = list_paid_keys(&state.pool, &profile.id).await?;
            let extras = list_extras(&state.pool, &profile.id).await?;
            build_dashboard(
                profile, &paid, &extras, view_year, filter, grain, tab, today,
            )
        } else {
            None
        }
    } else {
        None
    };

    let profile_opts: Vec<ProfileOption> = profiles
        .iter()
        .map(|p| ProfileOption {
            id: p.id.clone(),
            name: p.name.clone(),
            selected: active_id.as_deref() == Some(p.id.as_str()),
        })
        .collect();

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
        has_profiles: !profiles.is_empty(),
        profiles: profile_opts,
        active_id: active_id.unwrap_or_default(),
        empty: empty_state(active.as_ref()),
        dashboard,
        default_start,
        error: String::new(),
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

fn calendar_from_dashboard(
    d: crate::models::DashboardView,
    include_year_strip: bool,
) -> CalendarTemplate {
    CalendarTemplate {
        profile_id: d.profile_id,
        view_year: d.view_year,
        months: d.months,
        year_stats: include_year_strip.then_some(d.year_stats),
        payment_filter: d.payment_filter,
        grain: d.chart.grain,
    }
}

fn payments_from_dashboard(
    d: crate::models::DashboardView,
    include_year_strip: bool,
) -> PaymentsTemplate {
    PaymentsTemplate {
        profile_id: d.profile_id,
        payment_rows: d.payment_rows,
        payment_filter: d.payment_filter,
        summary: d.summary,
        extra_date_default: d.extra_date_default,
        view_year: d.view_year,
        grain: d.chart.grain,
        year_stats: include_year_strip.then_some(d.year_stats),
    }
}