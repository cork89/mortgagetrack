use std::collections::HashSet;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{
    begin, execute, get_conn, params, query_all, query_optional, DbConn, DbPool, DbTx, FromRow,
};
use crate::error::{AppError, AppResult};

use super::amort::{build_schedule, recurring_extra_dates, ExtraInput, Recurrence, RowKind};

const ACTIVE_PROFILE_KEY: &str = "active_profile_id";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub user_id: String,
    pub name: String,
    pub principal: Option<f64>,
    pub rate: Option<f64>,
    pub term_years: Option<i64>,
    pub start_date: Option<String>,
    pub monthly_payment: Option<f64>,
    pub total_interest: Option<f64>,
    pub version: i64,
    pub auto_mark_due_paid: bool,
}

impl FromRow for Profile {
    fn from_row(row: &crate::db::Row) -> crate::error::AppResult<Self> {
        Ok(Self {
            id: row.get(0)?,
            user_id: row.get(1)?,
            name: row.get(2)?,
            principal: row.get(3)?,
            rate: row.get(4)?,
            term_years: row.get(5)?,
            start_date: row.get(6)?,
            monthly_payment: row.get(7)?,
            total_interest: row.get(8)?,
            version: row.get(9)?,
            auto_mark_due_paid: row.get::<i64>(10)? != 0,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Loan {
    pub principal: f64,
    pub rate: f64,
    pub term_years: i32,
    pub start_date: NaiveDate,
    /// Cached on the profile; refreshed when paid extras/recasts change.
    #[allow(dead_code)]
    pub payment: f64,
    /// Cached on the profile; refreshed when paid extras/recasts change.
    #[allow(dead_code)]
    pub total_interest: f64,
}

impl Profile {
    pub fn loan(&self) -> Option<Loan> {
        let principal = self.principal?;
        let rate = self.rate?;
        let term_years = self.term_years? as i32;
        let start_date = NaiveDate::parse_from_str(self.start_date.as_ref()?, "%Y-%m-%d").ok()?;
        Some(Loan {
            principal,
            rate,
            term_years,
            start_date,
            payment: self.monthly_payment.unwrap_or(0.0),
            total_interest: self.total_interest.unwrap_or(0.0),
        })
    }

    pub fn has_loan(&self) -> bool {
        self.loan().is_some()
    }
}

#[derive(Debug, Clone)]
pub struct ExtraPayment {
    pub id: String,
    #[allow(dead_code)]
    pub profile_id: String,
    pub date: String,
    pub amount: f64,
    /// When true, schedule is recast so future monthly payments drop instead of term shortening.
    pub recast: bool,
}

impl FromRow for ExtraPayment {
    fn from_row(row: &crate::db::Row) -> crate::error::AppResult<Self> {
        Ok(Self {
            id: row.get(0)?,
            profile_id: row.get(1)?,
            date: row.get(2)?,
            amount: row.get(3)?,
            recast: row.get(4)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct HomeImprovement {
    pub id: String,
    #[allow(dead_code)]
    pub profile_id: String,
    pub date: String,
    pub amount: f64,
    pub note: String,
    pub detail: String,
}

impl FromRow for HomeImprovement {
    fn from_row(row: &crate::db::Row) -> crate::error::AppResult<Self> {
        Ok(Self {
            id: row.get(0)?,
            profile_id: row.get(1)?,
            date: row.get(2)?,
            amount: row.get(3)?,
            note: row.get(4)?,
            detail: row.get(5)?,
        })
    }
}

pub(crate) fn user_key(user_id: Uuid) -> String {
    user_id.to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileRole {
    Owner,
    Editor,
}

pub async fn require_owned_profile(
    pool: &DbPool,
    user_id: Uuid,
    profile_id: &str,
) -> AppResult<()> {
    let conn = get_conn(pool).await?;
    require_owned_profile_conn(&conn, user_id, profile_id).await
}

async fn require_owned_profile_conn(
    conn: &DbConn,
    user_id: Uuid,
    profile_id: &str,
) -> AppResult<()> {
    let owned: Option<(String,)> = query_optional(
        conn,
        "SELECT id FROM profiles WHERE id = ? AND user_id = ?",
        params![profile_id, user_key(user_id)],
    )
    .await?;

    if owned.is_none() {
        return Err(AppError::NotFound("Profile not found".into()));
    }
    Ok(())
}

pub async fn require_profile_access(
    pool: &DbPool,
    user_id: Uuid,
    profile_id: &str,
) -> AppResult<ProfileRole> {
    let conn = get_conn(pool).await?;
    require_profile_access_conn(&conn, user_id, profile_id).await
}

async fn require_profile_access_conn(
    conn: &DbConn,
    user_id: Uuid,
    profile_id: &str,
) -> AppResult<ProfileRole> {
    let key = user_key(user_id);
    let owned: Option<(String,)> = query_optional(
        conn,
        "SELECT id FROM profiles WHERE id = ? AND user_id = ?",
        params![profile_id, key.as_str()],
    )
    .await?;
    if owned.is_some() {
        return Ok(ProfileRole::Owner);
    }

    let collab: Option<(String,)> = query_optional(
        conn,
        "SELECT user_id FROM profile_collaborators WHERE profile_id = ? AND user_id = ?",
        params![profile_id, key.as_str()],
    )
    .await?;

    if collab.is_some() {
        return Ok(ProfileRole::Editor);
    }

    Err(AppError::NotFound("Profile not found".into()))
}

pub async fn list_profiles(pool: &DbPool, user_id: Uuid) -> AppResult<Vec<Profile>> {
    let conn = get_conn(pool).await?;
    list_profiles_conn(&conn, user_id).await
}

/// Number of profiles owned by the user (excludes shared collaborator profiles).
pub async fn count_owned_profiles(pool: &DbPool, user_id: Uuid) -> AppResult<usize> {
    let conn = get_conn(pool).await?;
    let rows: Vec<(String,)> = query_all(
        &conn,
        "SELECT id FROM profiles WHERE user_id = ?",
        params![user_key(user_id).as_str()],
    )
    .await?;
    Ok(rows.len())
}

async fn list_profiles_conn(conn: &DbConn, user_id: Uuid) -> AppResult<Vec<Profile>> {
    let key = user_key(user_id);
    query_all(
        conn,
        r#"
        SELECT id, user_id, name, principal, rate, term_years, start_date, monthly_payment, total_interest, version, auto_mark_due_paid, created_at
        FROM profiles
        WHERE user_id = ?
        UNION
        SELECT p.id, p.user_id, p.name, p.principal, p.rate, p.term_years, p.start_date,
               p.monthly_payment, p.total_interest, p.version, p.auto_mark_due_paid, p.created_at
        FROM profiles p
        INNER JOIN profile_collaborators c ON c.profile_id = p.id
        WHERE c.user_id = ?
        ORDER BY created_at ASC
        "#,
        params![key.as_str(), key.as_str()],
    )
    .await
}

pub async fn load_profile(pool: &DbPool, user_id: Uuid, id: &str) -> AppResult<Option<Profile>> {
    let conn = get_conn(pool).await?;
    load_profile_conn(&conn, user_id, id).await
}

async fn load_profile_conn(conn: &DbConn, user_id: Uuid, id: &str) -> AppResult<Option<Profile>> {
    let key = user_key(user_id);
    query_optional(
        conn,
        r#"
        SELECT id, user_id, name, principal, rate, term_years, start_date, monthly_payment, total_interest, version, auto_mark_due_paid
        FROM profiles
        WHERE id = ?
          AND (
            user_id = ?
            OR EXISTS (
              SELECT 1 FROM profile_collaborators c
              WHERE c.profile_id = profiles.id AND c.user_id = ?
            )
          )
        "#,
        params![id, key.as_str(), key.as_str()],
    )
    .await
}

pub async fn get_active_profile_id(pool: &DbPool, user_id: Uuid) -> AppResult<Option<String>> {
    let conn = get_conn(pool).await?;
    get_active_profile_id_conn(&conn, user_id).await
}

async fn get_active_profile_id_conn(conn: &DbConn, user_id: Uuid) -> AppResult<Option<String>> {
    let value: Option<(String,)> = query_optional(
        conn,
        "SELECT value FROM user_settings WHERE user_id = ? AND key = ?",
        params![user_key(user_id), ACTIVE_PROFILE_KEY],
    )
    .await?;
    Ok(value.map(|v| v.0).filter(|s| !s.is_empty()))
}

pub async fn set_active_profile(pool: &DbPool, user_id: Uuid, id: Option<&str>) -> AppResult<()> {
    let conn = get_conn(pool).await?;
    set_active_profile_conn(&conn, user_id, id).await
}

async fn set_active_profile_conn(conn: &DbConn, user_id: Uuid, id: Option<&str>) -> AppResult<()> {
    if let Some(profile_id) = id {
        require_profile_access_conn(conn, user_id, profile_id).await?;
    }

    execute(
        conn,
        r#"
        INSERT INTO user_settings (user_id, key, value) VALUES (?, ?, ?)
        ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value
        "#,
        params![user_key(user_id), ACTIVE_PROFILE_KEY, id.unwrap_or("")],
    )
    .await?;
    Ok(())
}

pub async fn list_paid_keys(pool: &DbPool, profile_id: &str) -> AppResult<Vec<String>> {
    let conn = get_conn(pool).await?;
    list_paid_keys_conn(&conn, profile_id).await
}

async fn list_paid_keys_conn(conn: &DbConn, profile_id: &str) -> AppResult<Vec<String>> {
    let rows: Vec<(String,)> = query_all(
        conn,
        "SELECT pay_key FROM paid_keys WHERE profile_id = ?",
        params![profile_id],
    )
    .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

/// Map extras into schedule inputs. Only paid extras affect balance / payment / interest.
pub fn extras_as_inputs(extras: &[ExtraPayment], paid: &HashSet<String>) -> Vec<ExtraInput> {
    extras
        .iter()
        .filter_map(|ex| {
            let date = NaiveDate::parse_from_str(&ex.date, "%Y-%m-%d").ok()?;
            Some(ExtraInput {
                id: ex.id.clone(),
                date,
                amount: ex.amount,
                recast: ex.recast,
                applied: paid.contains(&format!("extra:{}", ex.id)),
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct PaymentNote {
    pub pay_key: String,
    pub note: String,
}

impl FromRow for PaymentNote {
    fn from_row(row: &crate::db::Row) -> crate::error::AppResult<Self> {
        Ok(Self {
            pay_key: row.get(0)?,
            note: row.get(1)?,
        })
    }
}

pub async fn list_payment_notes(pool: &DbPool, profile_id: &str) -> AppResult<Vec<PaymentNote>> {
    let conn = get_conn(pool).await?;
    list_payment_notes_conn(&conn, profile_id).await
}

async fn list_payment_notes_conn(conn: &DbConn, profile_id: &str) -> AppResult<Vec<PaymentNote>> {
    query_all(
        conn,
        "SELECT pay_key, note FROM payment_notes WHERE profile_id = ?",
        params![profile_id],
    )
    .await
}

/// Bundle of data needed to render the main dashboard page in few round-trips.
pub struct PageBundle {
    pub profiles: Vec<Profile>,
    pub active: Option<Profile>,
    pub paid: Vec<String>,
    pub extras: Vec<ExtraPayment>,
    pub improvements: Vec<HomeImprovement>,
    pub notes: Vec<PaymentNote>,
}

/// Load profiles + active profile details with overlapped Turso round-trips.
pub async fn load_page_bundle(pool: &DbPool, user_id: Uuid) -> AppResult<PageBundle> {
    let (profiles, mut active_id) = tokio::try_join!(
        list_profiles(pool, user_id),
        get_active_profile_id(pool, user_id),
    )?;

    let mut active = match &active_id {
        Some(id) => load_profile(pool, user_id, id).await?,
        None => None,
    };

    // Access may have been revoked; fall back to another visible profile.
    if active_id.is_some() && active.is_none() {
        let fallback = profiles.first().map(|p| p.id.as_str());
        set_active_profile(pool, user_id, fallback).await?;
        active_id = fallback.map(|s| s.to_string());
        active = match &active_id {
            Some(id) => load_profile(pool, user_id, id).await?,
            None => None,
        };
    }

    let (paid, extras, improvements, notes) = if let Some(profile) = &active {
        if profile.has_loan() {
            let profile_id = profile.id.as_str();
            tokio::try_join!(
                list_paid_keys(pool, profile_id),
                list_extras(pool, profile_id),
                list_improvements(pool, profile_id),
                list_payment_notes(pool, profile_id),
            )?
        } else {
            (Vec::new(), Vec::new(), Vec::new(), Vec::new())
        }
    } else {
        (Vec::new(), Vec::new(), Vec::new(), Vec::new())
    };

    Ok(PageBundle {
        profiles,
        active,
        paid,
        extras,
        improvements,
        notes,
    })
}

pub async fn upsert_payment_note(
    pool: &DbPool,
    user_id: Uuid,
    profile_id: &str,
    pay_key: &str,
    note: &str,
) -> AppResult<()> {
    require_profile_access(pool, user_id, profile_id).await?;
    let trimmed = note.trim();
    if trimmed.chars().count() > 500 {
        return Err(AppError::BadRequest(
            "Notes are limited to 500 characters.".into(),
        ));
    }
    let conn = get_conn(pool).await?;
    if trimmed.is_empty() {
        execute(
            &conn,
            "DELETE FROM payment_notes WHERE profile_id = ? AND pay_key = ?",
            params![profile_id, pay_key],
        )
        .await?;
    } else {
        execute(
            &conn,
            r#"
            INSERT INTO payment_notes (profile_id, pay_key, note) VALUES (?, ?, ?)
            ON CONFLICT(profile_id, pay_key) DO UPDATE SET note = excluded.note
            "#,
            params![profile_id, pay_key, trimmed],
        )
        .await?;
    }
    Ok(())
}

pub async fn list_extras(pool: &DbPool, profile_id: &str) -> AppResult<Vec<ExtraPayment>> {
    let conn = get_conn(pool).await?;
    list_extras_conn(&conn, profile_id).await
}

async fn list_extras_conn(conn: &DbConn, profile_id: &str) -> AppResult<Vec<ExtraPayment>> {
    query_all(
        conn,
        r#"
        SELECT id, profile_id, date, amount, recast
        FROM extras
        WHERE profile_id = ?
        ORDER BY date
        "#,
        params![profile_id],
    )
    .await
}

pub async fn list_improvements(pool: &DbPool, profile_id: &str) -> AppResult<Vec<HomeImprovement>> {
    let conn = get_conn(pool).await?;
    list_improvements_conn(&conn, profile_id).await
}

async fn list_improvements_conn(
    conn: &DbConn,
    profile_id: &str,
) -> AppResult<Vec<HomeImprovement>> {
    query_all(
        conn,
        r#"
        SELECT id, profile_id, date, amount, note, detail
        FROM home_improvements
        WHERE profile_id = ?
        ORDER BY date, id
        "#,
        params![profile_id],
    )
    .await
}

fn validate_improvement_text(note: &str, detail: &str) -> AppResult<(String, String)> {
    let note = note.trim().to_string();
    let detail = detail.trim().to_string();
    if note.chars().count() > 200 {
        return Err(AppError::BadRequest(
            "Improvement notes are limited to 200 characters.".into(),
        ));
    }
    if detail.chars().count() > 1000 {
        return Err(AppError::BadRequest(
            "Improvement details are limited to 1000 characters.".into(),
        ));
    }
    Ok((note, detail))
}

pub async fn add_improvement(
    pool: &DbPool,
    user_id: Uuid,
    profile_id: &str,
    date: NaiveDate,
    amount: f64,
    note: &str,
    detail: &str,
) -> AppResult<HomeImprovement> {
    require_profile_access(pool, user_id, profile_id).await?;
    if amount <= 0.0 {
        return Err(AppError::BadRequest("Amount must be positive".into()));
    }
    let id = Uuid::new_v4().to_string();
    let date_str = date.format("%Y-%m-%d").to_string();
    let (note, detail) = validate_improvement_text(note, detail)?;
    let conn = get_conn(pool).await?;
    execute(
        &conn,
        "INSERT INTO home_improvements (id, profile_id, date, amount, note, detail) VALUES (?, ?, ?, ?, ?, ?)",
        params![
            id.as_str(),
            profile_id,
            date_str.as_str(),
            amount,
            note.as_str(),
            detail.as_str()
        ],
    )
    .await?;

    Ok(HomeImprovement {
        id,
        profile_id: profile_id.to_string(),
        date: date_str,
        amount,
        note,
        detail,
    })
}

pub async fn update_improvement(
    pool: &DbPool,
    user_id: Uuid,
    profile_id: &str,
    improvement_id: &str,
    date: NaiveDate,
    amount: f64,
    note: &str,
    detail: &str,
) -> AppResult<()> {
    require_profile_access(pool, user_id, profile_id).await?;
    if amount <= 0.0 {
        return Err(AppError::BadRequest("Amount must be positive".into()));
    }
    let date_str = date.format("%Y-%m-%d").to_string();
    let (note, detail) = validate_improvement_text(note, detail)?;
    let conn = get_conn(pool).await?;
    let result = execute(
        &conn,
        r#"
        UPDATE home_improvements
        SET date = ?, amount = ?, note = ?, detail = ?
        WHERE id = ? AND profile_id = ?
        "#,
        params![
            date_str.as_str(),
            amount,
            note.as_str(),
            detail.as_str(),
            improvement_id,
            profile_id
        ],
    )
    .await?;
    if result == 0 {
        return Err(AppError::NotFound("Home improvement not found".into()));
    }
    Ok(())
}

pub async fn delete_improvement(
    pool: &DbPool,
    user_id: Uuid,
    profile_id: &str,
    improvement_id: &str,
) -> AppResult<()> {
    require_profile_access(pool, user_id, profile_id).await?;
    let conn = get_conn(pool).await?;
    let result = execute(
        &conn,
        "DELETE FROM home_improvements WHERE id = ? AND profile_id = ?",
        params![improvement_id, profile_id],
    )
    .await?;
    if result == 0 {
        return Err(AppError::NotFound("Home improvement not found".into()));
    }
    Ok(())
}

pub async fn create_profile(
    pool: &DbPool,
    user_id: Uuid,
    name: &str,
    principal: f64,
    rate: f64,
    term_years: i32,
    start_date: NaiveDate,
    auto_mark_due_paid: bool,
) -> AppResult<Profile> {
    ensure_unique_name(pool, user_id, name, None).await?;
    let id = Uuid::new_v4().to_string();
    let built = build_schedule(principal, rate, term_years, start_date, &[]);
    let start = start_date.format("%Y-%m-%d").to_string();

    let conn = get_conn(pool).await?;
    execute(
        &conn,
        r#"
        INSERT INTO profiles (id, user_id, name, principal, rate, term_years, start_date, monthly_payment, total_interest, version, auto_mark_due_paid)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?)
        "#,
        params![
            id.as_str(),
            user_key(user_id),
            name,
            principal,
            rate,
            term_years,
            start.as_str(),
            built.payment,
            built.total_interest,
            auto_mark_due_paid as i64
        ],
    )
    .await?;

    set_active_profile(pool, user_id, Some(&id)).await?;
    load_profile(pool, user_id, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("Profile not found after create".into()))
}

pub async fn update_profile_loan(
    pool: &DbPool,
    user_id: Uuid,
    id: &str,
    name: &str,
    principal: f64,
    rate: f64,
    term_years: i32,
    start_date: NaiveDate,
    auto_mark_due_paid: bool,
) -> AppResult<Profile> {
    let conn = get_conn(pool).await?;
    require_profile_access_conn(&conn, user_id, id).await?;
    let profile = load_profile_conn(&conn, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Profile not found".into()))?;
    let owner_id = Uuid::parse_str(&profile.user_id)
        .map_err(|_| AppError::Internal("invalid profile owner id".into()))?;
    ensure_unique_name_conn(&conn, owner_id, name, Some(id)).await?;
    let extras = list_extras_conn(&conn, id).await?;
    let paid: HashSet<String> = list_paid_keys_conn(&conn, id).await?.into_iter().collect();
    let extra_inputs = extras_as_inputs(&extras, &paid);
    let built = build_schedule(principal, rate, term_years, start_date, &extra_inputs);
    let start = start_date.format("%Y-%m-%d").to_string();

    let result = execute(
        &conn,
        r#"
        UPDATE profiles
        SET name = ?, principal = ?, rate = ?, term_years = ?, start_date = ?,
            monthly_payment = ?, total_interest = ?, auto_mark_due_paid = ?
        WHERE id = ?
        "#,
        params![
            name,
            principal,
            rate,
            term_years,
            start.as_str(),
            built.payment,
            built.total_interest,
            auto_mark_due_paid as i64,
            id
        ],
    )
    .await?;

    if result == 0 {
        return Err(AppError::NotFound("Profile not found".into()));
    }

    load_profile_conn(&conn, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Profile not found".into()))
}

pub async fn rename_profile(pool: &DbPool, user_id: Uuid, id: &str, name: &str) -> AppResult<()> {
    require_owned_profile(pool, user_id, id).await?;
    ensure_unique_name(pool, user_id, name, Some(id)).await?;
    let conn = get_conn(pool).await?;
    let result = execute(
        &conn,
        "UPDATE profiles SET name = ? WHERE id = ? AND user_id = ?",
        params![name, id, user_key(user_id)],
    )
    .await?;
    if result == 0 {
        return Err(AppError::NotFound("Profile not found".into()));
    }
    Ok(())
}

pub async fn delete_profile(pool: &DbPool, user_id: Uuid, id: &str) -> AppResult<()> {
    require_owned_profile(pool, user_id, id).await?;

    let conn = get_conn(pool).await?;
    execute(
        &conn,
        "DELETE FROM paid_keys WHERE profile_id = ?",
        params![id],
    )
    .await?;
    execute(
        &conn,
        "DELETE FROM payment_notes WHERE profile_id = ?",
        params![id],
    )
    .await?;
    execute(
        &conn,
        "DELETE FROM extras WHERE profile_id = ?",
        params![id],
    )
    .await?;
    execute(
        &conn,
        "DELETE FROM home_improvements WHERE profile_id = ?",
        params![id],
    )
    .await?;
    let result = execute(
        &conn,
        "DELETE FROM profiles WHERE id = ? AND user_id = ?",
        params![id, user_key(user_id)],
    )
    .await?;
    if result == 0 {
        return Err(AppError::NotFound("Profile not found".into()));
    }

    let active = get_active_profile_id(pool, user_id).await?;
    if active.as_deref() == Some(id) {
        let next = list_profiles(pool, user_id).await?;
        set_active_profile(pool, user_id, next.first().map(|p| p.id.as_str())).await?;
    }
    Ok(())
}

/// Set paid status for a pay key. No-ops when already in the desired state.
pub async fn set_paid(
    pool: &DbPool,
    user_id: Uuid,
    profile_id: &str,
    pay_key: &str,
    paid: bool,
) -> AppResult<bool> {
    require_profile_access(pool, user_id, profile_id).await?;
    let conn = get_conn(pool).await?;

    let existing: Option<(String,)> = query_optional(
        &conn,
        "SELECT pay_key FROM paid_keys WHERE profile_id = ? AND pay_key = ?",
        params![profile_id, pay_key],
    )
    .await?;
    let currently_paid = existing.is_some();
    if currently_paid == paid {
        return Ok(paid);
    }

    let tx = begin(&conn).await?;
    if paid {
        execute(
            &tx,
            "INSERT INTO paid_keys (profile_id, pay_key) VALUES (?, ?)",
            params![profile_id, pay_key],
        )
        .await?;
    } else {
        execute(
            &tx,
            "DELETE FROM paid_keys WHERE profile_id = ? AND pay_key = ?",
            params![profile_id, pay_key],
        )
        .await?;
    }
    refresh_loan_totals_tx(&tx, user_id, profile_id).await?;
    tx.commit().await?;
    Ok(paid)
}

pub async fn clear_paid(pool: &DbPool, user_id: Uuid, profile_id: &str) -> AppResult<()> {
    require_profile_access(pool, user_id, profile_id).await?;
    let conn = get_conn(pool).await?;
    let tx = begin(&conn).await?;
    execute(
        &tx,
        "DELETE FROM paid_keys WHERE profile_id = ?",
        params![profile_id],
    )
    .await?;
    refresh_loan_totals_tx(&tx, user_id, profile_id).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn mark_due_paid(
    pool: &DbPool,
    user_id: Uuid,
    profile_id: &str,
    pay_keys: &[String],
) -> AppResult<()> {
    require_profile_access(pool, user_id, profile_id).await?;
    let conn = get_conn(pool).await?;
    let tx = begin(&conn).await?;
    for key in pay_keys {
        execute(
            &tx,
            "INSERT OR IGNORE INTO paid_keys (profile_id, pay_key) VALUES (?, ?)",
            params![profile_id, key.as_str()],
        )
        .await?;
    }
    refresh_loan_totals_tx(&tx, user_id, profile_id).await?;
    tx.commit().await?;
    Ok(())
}

/// When enabled on the profile, mark scheduled payments due on or before `today` as paid.
pub async fn auto_mark_due_paid_if_enabled(
    pool: &DbPool,
    user_id: Uuid,
    profile: &Profile,
    today: NaiveDate,
) -> AppResult<Vec<String>> {
    if !profile.auto_mark_due_paid || !profile.has_loan() {
        return list_paid_keys(pool, &profile.id).await;
    }
    let loan = profile
        .loan()
        .ok_or_else(|| AppError::Internal("profile missing loan".into()))?;
    let extras = list_extras(pool, &profile.id).await?;
    let paid_keys = list_paid_keys(pool, &profile.id).await?;
    let paid: HashSet<String> = paid_keys.iter().cloned().collect();
    let extra_inputs = extras_as_inputs(&extras, &paid);
    let built = build_schedule(
        loan.principal,
        loan.rate,
        loan.term_years,
        loan.start_date,
        &extra_inputs,
    );
    let keys: Vec<String> = built
        .rows
        .iter()
        .filter(|r| r.kind == RowKind::Scheduled && r.due <= today && !paid.contains(&r.pay_key))
        .map(|r| r.pay_key.clone())
        .collect();
    if keys.is_empty() {
        return Ok(paid_keys);
    }
    mark_due_paid(pool, user_id, &profile.id, &keys).await?;
    list_paid_keys(pool, &profile.id).await
}

pub async fn add_extra(
    pool: &DbPool,
    user_id: Uuid,
    profile_id: &str,
    date: NaiveDate,
    amount: f64,
    recast: bool,
    recurrence: Option<Recurrence>,
) -> AppResult<Vec<ExtraPayment>> {
    require_profile_access(pool, user_id, profile_id).await?;
    if amount <= 0.0 {
        return Err(AppError::BadRequest("Amount must be positive".into()));
    }
    let recast = recast && recurrence.is_none();
    let dates = if let Some(recurrence) = recurrence {
        let profile = load_profile(pool, user_id, profile_id)
            .await?
            .ok_or_else(|| AppError::NotFound("Profile not found".into()))?;
        let loan = profile
            .loan()
            .ok_or_else(|| AppError::BadRequest("No active loan".into()))?;
        recurring_extra_dates(date, loan.start_date, loan.term_years, recurrence)
    } else {
        vec![date]
    };

    let conn = get_conn(pool).await?;
    let tx = begin(&conn).await?;
    let mut created = Vec::with_capacity(dates.len());
    for d in dates {
        let id = Uuid::new_v4().to_string();
        let date_str = d.format("%Y-%m-%d").to_string();
        execute(
            &tx,
            "INSERT INTO extras (id, profile_id, date, amount, recast) VALUES (?, ?, ?, ?, ?)",
            params![id.as_str(), profile_id, date_str.as_str(), amount, recast],
        )
        .await?;
        created.push(ExtraPayment {
            id,
            profile_id: profile_id.to_string(),
            date: date_str,
            amount,
            recast,
        });
    }
    tx.commit().await?;

    // Extras start unpaid: balance, monthly payment, and total interest update only when marked paid.
    Ok(created)
}

pub async fn delete_extra(
    pool: &DbPool,
    user_id: Uuid,
    profile_id: &str,
    extra_id: &str,
) -> AppResult<()> {
    require_profile_access(pool, user_id, profile_id).await?;
    let pay_key = format!("extra:{extra_id}");
    let conn = get_conn(pool).await?;
    let tx = begin(&conn).await?;
    execute(
        &tx,
        "DELETE FROM paid_keys WHERE profile_id = ? AND pay_key = ?",
        params![profile_id, pay_key.as_str()],
    )
    .await?;
    execute(
        &tx,
        "DELETE FROM payment_notes WHERE profile_id = ? AND pay_key = ?",
        params![profile_id, pay_key.as_str()],
    )
    .await?;
    let result = execute(
        &tx,
        "DELETE FROM extras WHERE id = ? AND profile_id = ?",
        params![extra_id, profile_id],
    )
    .await?;
    if result == 0 {
        return Err(AppError::NotFound("Extra payment not found".into()));
    }
    refresh_loan_totals_tx(&tx, user_id, profile_id).await?;
    tx.commit().await?;
    Ok(())
}

/// Delete every extra that has not been marked paid (and any notes on those rows).
pub async fn delete_unpaid_extras(
    pool: &DbPool,
    user_id: Uuid,
    profile_id: &str,
) -> AppResult<u64> {
    require_profile_access(pool, user_id, profile_id).await?;
    let conn = get_conn(pool).await?;
    let tx = begin(&conn).await?;
    execute(
        &tx,
        r#"
        DELETE FROM payment_notes
        WHERE profile_id = ?
          AND pay_key IN (
            SELECT 'extra:' || e.id
            FROM extras e
            WHERE e.profile_id = ?
              AND NOT EXISTS (
                SELECT 1 FROM paid_keys p
                WHERE p.profile_id = e.profile_id
                  AND p.pay_key = 'extra:' || e.id
              )
          )
        "#,
        params![profile_id, profile_id],
    )
    .await?;
    let deleted = execute(
        &tx,
        r#"
        DELETE FROM extras
        WHERE profile_id = ?
          AND NOT EXISTS (
            SELECT 1 FROM paid_keys p
            WHERE p.profile_id = extras.profile_id
              AND p.pay_key = 'extra:' || extras.id
          )
        "#,
        params![profile_id],
    )
    .await?;
    if deleted > 0 {
        refresh_loan_totals_tx(&tx, user_id, profile_id).await?;
    }
    tx.commit().await?;
    Ok(deleted)
}

async fn refresh_loan_totals_tx(tx: &DbTx, user_id: Uuid, profile_id: &str) -> AppResult<()> {
    let profile = load_profile_in_tx(tx, user_id, profile_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Profile not found".into()))?;
    let Some(loan) = profile.loan() else {
        return Ok(());
    };
    let extras = list_extras_in_tx(tx, profile_id).await?;
    let paid: HashSet<String> = list_paid_keys_in_tx(tx, profile_id)
        .await?
        .into_iter()
        .collect();
    let extra_inputs = extras_as_inputs(&extras, &paid);
    let built = build_schedule(
        loan.principal,
        loan.rate,
        loan.term_years,
        loan.start_date,
        &extra_inputs,
    );
    execute(
        tx,
        "UPDATE profiles SET monthly_payment = ?, total_interest = ? WHERE id = ?",
        params![built.payment, built.total_interest, profile_id],
    )
    .await?;
    Ok(())
}

async fn load_profile_in_tx(tx: &DbTx, user_id: Uuid, id: &str) -> AppResult<Option<Profile>> {
    let key = user_key(user_id);
    query_optional(
        tx,
        r#"
        SELECT id, user_id, name, principal, rate, term_years, start_date, monthly_payment, total_interest, version, auto_mark_due_paid
        FROM profiles
        WHERE id = ?
          AND (
            user_id = ?
            OR EXISTS (
              SELECT 1 FROM profile_collaborators c
              WHERE c.profile_id = profiles.id AND c.user_id = ?
            )
          )
        "#,
        params![id, key.as_str(), key.as_str()],
    )
    .await
}

async fn list_extras_in_tx(tx: &DbTx, profile_id: &str) -> AppResult<Vec<ExtraPayment>> {
    query_all(
        tx,
        r#"
        SELECT id, profile_id, date, amount, recast
        FROM extras
        WHERE profile_id = ?
        ORDER BY date
        "#,
        params![profile_id],
    )
    .await
}

async fn list_paid_keys_in_tx(tx: &DbTx, profile_id: &str) -> AppResult<Vec<String>> {
    let rows: Vec<(String,)> = query_all(
        tx,
        "SELECT pay_key FROM paid_keys WHERE profile_id = ?",
        params![profile_id],
    )
    .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

async fn ensure_unique_name(
    pool: &DbPool,
    user_id: Uuid,
    name: &str,
    except_id: Option<&str>,
) -> AppResult<()> {
    let conn = get_conn(pool).await?;
    ensure_unique_name_conn(&conn, user_id, name, except_id).await
}

async fn ensure_unique_name_conn(
    conn: &DbConn,
    user_id: Uuid,
    name: &str,
    except_id: Option<&str>,
) -> AppResult<()> {
    let existing: Option<(String,)> = if let Some(id) = except_id {
        query_optional(
            conn,
            "SELECT id FROM profiles WHERE user_id = ? AND lower(name) = lower(?) AND id != ?",
            params![user_key(user_id), name, id],
        )
        .await?
    } else {
        query_optional(
            conn,
            "SELECT id FROM profiles WHERE user_id = ? AND lower(name) = lower(?)",
            params![user_key(user_id), name],
        )
        .await?
    };

    if existing.is_some() {
        return Err(AppError::BadRequest(
            "A profile with that name already exists.".into(),
        ));
    }
    Ok(())
}
