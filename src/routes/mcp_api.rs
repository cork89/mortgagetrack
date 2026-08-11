//! JSON API for the Worker MCP server (Pro feature; auth via Worker identity headers).

use std::collections::HashSet;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::app_state::AppState;
use crate::auth::AuthUser;
use crate::error::{AppError, AppResult};
use crate::models::{
    add_extra, add_improvement, build_schedule, clear_paid, count_owned_profiles, create_profile,
    delete_extra, delete_improvement, delete_profile, extras_as_inputs, list_extras,
    list_improvements, list_paid_keys, list_payment_notes, list_profiles, load_profile,
    mark_due_paid, payment_status, rename_profile, require_profile_access, set_paid,
    update_improvement, update_profile_loan, upsert_payment_note, Profile, Recurrence, RowKind,
    ScheduleBuilt,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/api/mcp/profiles",
            get(list_profiles_api).post(create_profile_api),
        )
        .route(
            "/api/mcp/profiles/{id}",
            get(get_profile_api).post(update_profile_api),
        )
        .route("/api/mcp/profiles/{id}/rename", post(rename_profile_api))
        .route("/api/mcp/profiles/{id}/delete", post(delete_profile_api))
        .route("/api/mcp/profiles/{id}/summary", get(get_summary_api))
        .route("/api/mcp/profiles/{id}/payments", get(list_payments_api))
        .route(
            "/api/mcp/profiles/{id}/extras",
            get(list_extras_api).post(add_extra_api),
        )
        .route(
            "/api/mcp/profiles/{id}/extras/{extra_id}",
            delete(delete_extra_api).post(delete_extra_api),
        )
        .route(
            "/api/mcp/profiles/{id}/notes",
            get(list_notes_api).post(upsert_note_api),
        )
        .route(
            "/api/mcp/profiles/{id}/improvements",
            get(list_improvements_api).post(add_improvement_api),
        )
        .route(
            "/api/mcp/profiles/{id}/improvements/{improvement_id}",
            delete(delete_improvement_api).post(delete_improvement_api),
        )
        .route(
            "/api/mcp/profiles/{id}/improvements/{improvement_id}/update",
            post(update_improvement_api),
        )
        .route(
            "/api/mcp/profiles/{id}/payments/set-paid",
            post(set_payment_paid_api),
        )
        .route(
            "/api/mcp/profiles/{id}/payments/clear-paid",
            post(clear_paid_api),
        )
        .route(
            "/api/mcp/profiles/{id}/payments/mark-due",
            post(mark_due_api),
        )
}

fn json_ok(data: Value) -> Response {
    (StatusCode::OK, Json(json!({ "ok": true, "data": data }))).into_response()
}

fn json_err(err: AppError) -> Response {
    let (status, message) = match &err {
        AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
        AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
        AppError::TooManyRequests(msg) => (StatusCode::TOO_MANY_REQUESTS, msg.clone()),
        AppError::Database(_) | AppError::Internal(_) | AppError::Template(_) => {
            tracing::error!(error = %err, "mcp api error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal server error".to_string(),
            )
        }
    };
    (status, Json(json!({ "ok": false, "error": message }))).into_response()
}

fn profile_json(p: &Profile) -> Value {
    json!({
        "id": p.id,
        "name": p.name,
        "principal": p.principal,
        "rate": p.rate,
        "term_years": p.term_years,
        "start_date": p.start_date,
        "monthly_payment": p.monthly_payment,
        "total_interest": p.total_interest,
        "auto_mark_due_paid": p.auto_mark_due_paid,
    })
}

fn parse_date(s: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| AppError::BadRequest("Invalid date (use YYYY-MM-DD)".into()))
}

fn validate_loan(principal: f64, rate: f64, term: i32) -> AppResult<()> {
    if !(principal > 0.0) || !(term > 0) || rate < 0.0 {
        return Err(AppError::BadRequest(
            "Enter a valid loan amount, rate (≥ 0), term, and first payment date.".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ProfileBody {
    pub name: String,
    pub principal: f64,
    pub rate: f64,
    pub term: i32,
    pub start_date: String,
    #[serde(default)]
    pub auto_mark_due_paid: bool,
}

#[derive(Debug, Deserialize)]
pub struct RenameBody {
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct PaymentsQuery {
    pub year: Option<i32>,
    pub limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct SetPaidBody {
    pub pay_key: String,
    pub paid: bool,
}

#[derive(Debug, Deserialize)]
pub struct ExtraBody {
    pub date: String,
    pub amount: f64,
    #[serde(default)]
    pub recast: bool,
    #[serde(default)]
    pub recurrence: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct NoteBody {
    pub pay_key: String,
    pub note: String,
}

#[derive(Debug, Deserialize)]
pub struct ImprovementBody {
    pub date: String,
    pub amount: f64,
    #[serde(default)]
    pub note: String,
    #[serde(default)]
    pub detail: String,
}

#[derive(Debug, Serialize)]
struct PaymentRowJson {
    pay_key: String,
    due: String,
    label: String,
    payment: f64,
    principal: f64,
    interest: f64,
    balance: f64,
    paid: bool,
    status: String,
    is_extra: bool,
    extra_id: Option<String>,
    recast: bool,
}

async fn list_profiles_api(user: AuthUser, State(state): State<AppState>) -> Response {
    match list_profiles(&state.pool, user.id).await {
        Ok(profiles) => json_ok(json!(profiles.iter().map(profile_json).collect::<Vec<_>>())),
        Err(err) => json_err(err),
    }
}

async fn get_profile_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    if let Err(err) = require_profile_access(&state.pool, user.id, &id).await {
        return json_err(err);
    }
    match load_profile(&state.pool, user.id, &id).await {
        Ok(Some(p)) => json_ok(profile_json(&p)),
        Ok(None) => json_err(AppError::NotFound("Profile not found".into())),
        Err(err) => json_err(err),
    }
}

async fn create_profile_api(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<ProfileBody>,
) -> Response {
    let name = body.name.trim();
    if name.is_empty() {
        return json_err(AppError::BadRequest("Enter a profile name.".into()));
    }
    if !user.is_paid() {
        match count_owned_profiles(&state.pool, user.id).await {
            Ok(owned) if owned >= 1 => {
                return json_err(AppError::BadRequest(
                    "Creating more than one profile is a pro feature.".into(),
                ));
            }
            Err(err) => return json_err(err),
            _ => {}
        }
    }
    let start = match parse_date(&body.start_date) {
        Ok(d) => d,
        Err(err) => return json_err(err),
    };
    if let Err(err) = validate_loan(body.principal, body.rate, body.term) {
        return json_err(err);
    }
    match create_profile(
        &state.pool,
        user.id,
        name,
        body.principal,
        body.rate,
        body.term,
        start,
        body.auto_mark_due_paid,
    )
    .await
    {
        Ok(p) => json_ok(profile_json(&p)),
        Err(err) => json_err(err),
    }
}

async fn update_profile_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ProfileBody>,
) -> Response {
    let name = body.name.trim();
    if name.is_empty() {
        return json_err(AppError::BadRequest("Enter a profile name.".into()));
    }
    let start = match parse_date(&body.start_date) {
        Ok(d) => d,
        Err(err) => return json_err(err),
    };
    if let Err(err) = validate_loan(body.principal, body.rate, body.term) {
        return json_err(err);
    }
    if let Err(err) = update_profile_loan(
        &state.pool,
        user.id,
        &id,
        name,
        body.principal,
        body.rate,
        body.term,
        start,
        body.auto_mark_due_paid,
    )
    .await
    {
        return json_err(err);
    }
    match load_profile(&state.pool, user.id, &id).await {
        Ok(Some(p)) => json_ok(profile_json(&p)),
        Ok(None) => json_err(AppError::NotFound("Profile not found".into())),
        Err(err) => json_err(err),
    }
}

async fn rename_profile_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<RenameBody>,
) -> Response {
    let name = body.name.trim();
    if name.is_empty() {
        return json_err(AppError::BadRequest("Enter a profile name.".into()));
    }
    if let Err(err) = rename_profile(&state.pool, user.id, &id, name).await {
        return json_err(err);
    }
    match load_profile(&state.pool, user.id, &id).await {
        Ok(Some(p)) => json_ok(profile_json(&p)),
        Ok(None) => json_err(AppError::NotFound("Profile not found".into())),
        Err(err) => json_err(err),
    }
}

async fn delete_profile_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match delete_profile(&state.pool, user.id, &id).await {
        Ok(()) => json_ok(json!({ "deleted": id })),
        Err(err) => json_err(err),
    }
}

async fn profile_schedule(
    state: &AppState,
    user: &AuthUser,
    id: &str,
) -> AppResult<(Profile, ScheduleBuilt, HashSet<String>)> {
    require_profile_access(&state.pool, user.id, id).await?;
    let profile = load_profile(&state.pool, user.id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Profile not found".into()))?;
    let loan = profile
        .loan()
        .ok_or_else(|| AppError::BadRequest("Profile has no loan".into()))?;
    let extras = list_extras(&state.pool, id).await?;
    let paid: HashSet<String> = list_paid_keys(&state.pool, id).await?.into_iter().collect();
    let extra_inputs = extras_as_inputs(&extras, &paid);
    let built = build_schedule(
        loan.principal,
        loan.rate,
        loan.term_years,
        loan.start_date,
        &extra_inputs,
    );
    Ok((profile, built, paid))
}

async fn get_summary_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match profile_schedule(&state, &user, &id).await {
        Ok((profile, built, paid)) => {
            let today = state.today();
            let remaining_principal = built
                .rows
                .iter()
                .rev()
                .find(|r| paid.contains(&r.pay_key))
                .map(|r| r.balance)
                .unwrap_or_else(|| profile.principal.unwrap_or(0.0));
            let unpaid: Vec<_> = built
                .rows
                .iter()
                .filter(|r| !paid.contains(&r.pay_key))
                .collect();
            let next_due = unpaid.first().map(|r| r.due.format("%Y-%m-%d").to_string());
            let remaining_interest: f64 = unpaid.iter().map(|r| r.interest).sum();
            json_ok(json!({
                "profile": profile_json(&profile),
                "monthly_payment": built.payment,
                "total_interest": built.total_interest,
                "remaining_interest": remaining_interest,
                "balance": remaining_principal,
                "payments_total": built.rows.len(),
                "payments_paid": paid.len(),
                "payments_remaining": unpaid.len(),
                "next_due": next_due,
                "today": today.format("%Y-%m-%d").to_string(),
            }))
        }
        Err(err) => json_err(err),
    }
}

async fn list_payments_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(q): Query<PaymentsQuery>,
) -> Response {
    match profile_schedule(&state, &user, &id).await {
        Ok((_, built, paid)) => {
            let today = state.today();
            let mut rows: Vec<PaymentRowJson> = built
                .rows
                .iter()
                .filter(|r| q.year.map(|y| r.due.year() == y).unwrap_or(true))
                .map(|r| {
                    let is_paid = paid.contains(&r.pay_key);
                    PaymentRowJson {
                        pay_key: r.pay_key.clone(),
                        due: r.due.format("%Y-%m-%d").to_string(),
                        label: r.label.clone(),
                        payment: r.payment,
                        principal: r.principal,
                        interest: r.interest,
                        balance: r.balance,
                        paid: is_paid,
                        status: payment_status(r.due, is_paid, today).to_string(),
                        is_extra: matches!(r.kind, RowKind::Extra),
                        extra_id: r.id.clone(),
                        recast: r.recast,
                    }
                })
                .collect();
            if let Some(limit) = q.limit {
                rows.truncate(limit);
            }
            json_ok(json!({ "payments": rows, "count": rows.len() }))
        }
        Err(err) => json_err(err),
    }
}

async fn list_extras_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    if let Err(err) = require_profile_access(&state.pool, user.id, &id).await {
        return json_err(err);
    }
    match list_extras(&state.pool, &id).await {
        Ok(extras) => json_ok(json!(extras
            .iter()
            .map(|e| json!({
                "id": e.id,
                "date": e.date,
                "amount": e.amount,
                "recast": e.recast,
            }))
            .collect::<Vec<_>>())),
        Err(err) => json_err(err),
    }
}

async fn list_notes_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    if let Err(err) = require_profile_access(&state.pool, user.id, &id).await {
        return json_err(err);
    }
    match list_payment_notes(&state.pool, &id).await {
        Ok(notes) => json_ok(json!(notes
            .iter()
            .map(|n| json!({ "pay_key": n.pay_key, "note": n.note }))
            .collect::<Vec<_>>())),
        Err(err) => json_err(err),
    }
}

async fn list_improvements_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    if let Err(err) = require_profile_access(&state.pool, user.id, &id).await {
        return json_err(err);
    }
    match list_improvements(&state.pool, &id).await {
        Ok(items) => json_ok(json!(items
            .iter()
            .map(|i| json!({
                "id": i.id,
                "date": i.date,
                "amount": i.amount,
                "note": i.note,
                "detail": i.detail,
            }))
            .collect::<Vec<_>>())),
        Err(err) => json_err(err),
    }
}

async fn set_payment_paid_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<SetPaidBody>,
) -> Response {
    match set_paid(&state.pool, user.id, &id, &body.pay_key, body.paid).await {
        Ok(paid) => json_ok(json!({ "pay_key": body.pay_key, "paid": paid })),
        Err(err) => json_err(err),
    }
}

async fn clear_paid_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match clear_paid(&state.pool, user.id, &id).await {
        Ok(()) => json_ok(json!({ "cleared": true })),
        Err(err) => json_err(err),
    }
}

async fn mark_due_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Response {
    match profile_schedule(&state, &user, &id).await {
        Ok((_, built, paid)) => {
            let today = state.today();
            let keys: Vec<String> = built
                .rows
                .iter()
                .filter(|r| {
                    !paid.contains(&r.pay_key)
                        && (r.due <= today
                            || (r.due.year() == today.year() && r.due.month() == today.month()))
                })
                .map(|r| r.pay_key.clone())
                .collect();
            match mark_due_paid(&state.pool, user.id, &id, &keys).await {
                Ok(()) => json_ok(json!({ "marked": keys.len(), "pay_keys": keys })),
                Err(err) => json_err(err),
            }
        }
        Err(err) => json_err(err),
    }
}

async fn upsert_note_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<NoteBody>,
) -> Response {
    match upsert_payment_note(&state.pool, user.id, &id, &body.pay_key, &body.note).await {
        Ok(()) => json_ok(json!({ "pay_key": body.pay_key, "note": body.note.trim() })),
        Err(err) => json_err(err),
    }
}

async fn add_extra_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ExtraBody>,
) -> Response {
    let date = match parse_date(&body.date) {
        Ok(d) => d,
        Err(err) => return json_err(err),
    };
    let recurrence = match body
        .recurrence
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        None => None,
        Some(s) => match Recurrence::parse(s) {
            Some(r) => Some(r),
            None => {
                return json_err(AppError::BadRequest(
                    "Recurrence must be monthly, quarterly, or yearly.".into(),
                ))
            }
        },
    };
    match add_extra(
        &state.pool,
        user.id,
        &id,
        date,
        body.amount,
        body.recast,
        recurrence,
    )
    .await
    {
        Ok(extras) => {
            let extra = &extras[0];
            json_ok(json!({
                "id": extra.id,
                "date": extra.date,
                "amount": extra.amount,
                "recast": extra.recast,
                "count": extras.len(),
            }))
        }
        Err(err) => json_err(err),
    }
}

async fn delete_extra_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path((id, extra_id)): Path<(String, String)>,
) -> Response {
    match delete_extra(&state.pool, user.id, &id, &extra_id).await {
        Ok(()) => json_ok(json!({ "deleted": extra_id })),
        Err(err) => json_err(err),
    }
}

async fn add_improvement_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<ImprovementBody>,
) -> Response {
    let date = match parse_date(&body.date) {
        Ok(d) => d,
        Err(err) => return json_err(err),
    };
    match add_improvement(
        &state.pool,
        user.id,
        &id,
        date,
        body.amount,
        &body.note,
        &body.detail,
    )
    .await
    {
        Ok(item) => json_ok(json!({
            "id": item.id,
            "date": item.date,
            "amount": item.amount,
            "note": item.note,
            "detail": item.detail,
        })),
        Err(err) => json_err(err),
    }
}

async fn update_improvement_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path((id, improvement_id)): Path<(String, String)>,
    Json(body): Json<ImprovementBody>,
) -> Response {
    let date = match parse_date(&body.date) {
        Ok(d) => d,
        Err(err) => return json_err(err),
    };
    if let Err(err) = update_improvement(
        &state.pool,
        user.id,
        &id,
        &improvement_id,
        date,
        body.amount,
        &body.note,
        &body.detail,
    )
    .await
    {
        return json_err(err);
    }
    json_ok(json!({
        "id": improvement_id,
        "date": body.date,
        "amount": body.amount,
        "note": body.note,
        "detail": body.detail,
    }))
}

async fn delete_improvement_api(
    user: AuthUser,
    State(state): State<AppState>,
    Path((id, improvement_id)): Path<(String, String)>,
) -> Response {
    match delete_improvement(&state.pool, user.id, &id, &improvement_id).await {
        Ok(()) => json_ok(json!({ "deleted": improvement_id })),
        Err(err) => json_err(err),
    }
}
