use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

use super::amort::build_schedule;

const ACTIVE_PROFILE_KEY: &str = "active_profile_id";

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
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
}

#[derive(Debug, Clone)]
pub struct Loan {
    pub principal: f64,
    pub rate: f64,
    pub term_years: i32,
    pub start_date: NaiveDate,
    pub payment: f64,
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

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExtraPayment {
    pub id: String,
    #[allow(dead_code)] // selected by sqlx::FromRow
    pub profile_id: String,
    pub date: String,
    pub amount: f64,
}

fn user_key(user_id: Uuid) -> String {
    user_id.to_string()
}

pub async fn require_owned_profile(
    pool: &SqlitePool,
    user_id: Uuid,
    profile_id: &str,
) -> AppResult<()> {
    let owned: Option<(String,)> =
        sqlx::query_as("SELECT id FROM profiles WHERE id = ? AND user_id = ?")
            .bind(profile_id)
            .bind(user_key(user_id))
            .fetch_optional(pool)
            .await?;

    if owned.is_none() {
        return Err(AppError::NotFound("Profile not found".into()));
    }
    Ok(())
}

pub async fn list_profiles(pool: &SqlitePool, user_id: Uuid) -> AppResult<Vec<Profile>> {
    let rows = sqlx::query_as::<_, Profile>(
        r#"
        SELECT id, user_id, name, principal, rate, term_years, start_date, monthly_payment, total_interest
        FROM profiles
        WHERE user_id = ?
        ORDER BY name COLLATE NOCASE
        "#,
    )
    .bind(user_key(user_id))
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn load_profile(
    pool: &SqlitePool,
    user_id: Uuid,
    id: &str,
) -> AppResult<Option<Profile>> {
    let row = sqlx::query_as::<_, Profile>(
        r#"
        SELECT id, user_id, name, principal, rate, term_years, start_date, monthly_payment, total_interest
        FROM profiles
        WHERE id = ? AND user_id = ?
        "#,
    )
    .bind(id)
    .bind(user_key(user_id))
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn get_active_profile_id(pool: &SqlitePool, user_id: Uuid) -> AppResult<Option<String>> {
    let value: Option<(String,)> = sqlx::query_as(
        "SELECT value FROM user_settings WHERE user_id = ? AND key = ?",
    )
    .bind(user_key(user_id))
    .bind(ACTIVE_PROFILE_KEY)
    .fetch_optional(pool)
    .await?;
    Ok(value.map(|v| v.0).filter(|s| !s.is_empty()))
}

pub async fn set_active_profile(
    pool: &SqlitePool,
    user_id: Uuid,
    id: Option<&str>,
) -> AppResult<()> {
    if let Some(profile_id) = id {
        require_owned_profile(pool, user_id, profile_id).await?;
    }

    sqlx::query(
        r#"
        INSERT INTO user_settings (user_id, key, value) VALUES (?, ?, ?)
        ON CONFLICT(user_id, key) DO UPDATE SET value = excluded.value
        "#,
    )
    .bind(user_key(user_id))
    .bind(ACTIVE_PROFILE_KEY)
    .bind(id.unwrap_or(""))
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_paid_keys(pool: &SqlitePool, profile_id: &str) -> AppResult<Vec<String>> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT pay_key FROM paid_keys WHERE profile_id = ?")
            .bind(profile_id)
            .fetch_all(pool)
            .await?;
    Ok(rows.into_iter().map(|r| r.0).collect())
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PaymentNote {
    pub pay_key: String,
    pub note: String,
}

pub async fn list_payment_notes(
    pool: &SqlitePool,
    profile_id: &str,
) -> AppResult<Vec<PaymentNote>> {
    let rows = sqlx::query_as::<_, PaymentNote>(
        "SELECT pay_key, note FROM payment_notes WHERE profile_id = ?",
    )
    .bind(profile_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn upsert_payment_note(
    pool: &SqlitePool,
    user_id: Uuid,
    profile_id: &str,
    pay_key: &str,
    note: &str,
) -> AppResult<()> {
    require_owned_profile(pool, user_id, profile_id).await?;
    let trimmed = note.trim();
    if trimmed.is_empty() {
        sqlx::query("DELETE FROM payment_notes WHERE profile_id = ? AND pay_key = ?")
            .bind(profile_id)
            .bind(pay_key)
            .execute(pool)
            .await?;
        return Ok(());
    }
    sqlx::query(
        r#"
        INSERT INTO payment_notes (profile_id, pay_key, note) VALUES (?, ?, ?)
        ON CONFLICT(profile_id, pay_key) DO UPDATE SET note = excluded.note
        "#,
    )
    .bind(profile_id)
    .bind(pay_key)
    .bind(trimmed)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_extras(pool: &SqlitePool, profile_id: &str) -> AppResult<Vec<ExtraPayment>> {
    let rows = sqlx::query_as::<_, ExtraPayment>(
        r#"
        SELECT id, profile_id, date, amount
        FROM extras
        WHERE profile_id = ?
        ORDER BY date
        "#,
    )
    .bind(profile_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

pub async fn create_profile(
    pool: &SqlitePool,
    user_id: Uuid,
    name: &str,
    principal: f64,
    rate: f64,
    term_years: i32,
    start_date: NaiveDate,
) -> AppResult<Profile> {
    ensure_unique_name(pool, user_id, name, None).await?;
    let id = Uuid::new_v4().to_string();
    let built = build_schedule(principal, rate, term_years, start_date, &[]);
    let start = start_date.format("%Y-%m-%d").to_string();

    sqlx::query(
        r#"
        INSERT INTO profiles (id, user_id, name, principal, rate, term_years, start_date, monthly_payment, total_interest)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(&id)
    .bind(user_key(user_id))
    .bind(name)
    .bind(principal)
    .bind(rate)
    .bind(term_years)
    .bind(&start)
    .bind(built.payment)
    .bind(built.total_interest)
    .execute(pool)
    .await?;

    set_active_profile(pool, user_id, Some(&id)).await?;
    load_profile(pool, user_id, &id)
        .await?
        .ok_or_else(|| AppError::NotFound("Profile not found after create".into()))
}

pub async fn update_profile_loan(
    pool: &SqlitePool,
    user_id: Uuid,
    id: &str,
    name: &str,
    principal: f64,
    rate: f64,
    term_years: i32,
    start_date: NaiveDate,
) -> AppResult<Profile> {
    require_owned_profile(pool, user_id, id).await?;
    ensure_unique_name(pool, user_id, name, Some(id)).await?;
    let extras = list_extras(pool, id).await?;
    let extra_tuples: Vec<(String, NaiveDate, f64)> = extras
        .iter()
        .filter_map(|ex| {
            let date = NaiveDate::parse_from_str(&ex.date, "%Y-%m-%d").ok()?;
            Some((ex.id.clone(), date, ex.amount))
        })
        .collect();
    let built = build_schedule(principal, rate, term_years, start_date, &extra_tuples);
    let start = start_date.format("%Y-%m-%d").to_string();

    let result = sqlx::query(
        r#"
        UPDATE profiles
        SET name = ?, principal = ?, rate = ?, term_years = ?, start_date = ?,
            monthly_payment = ?, total_interest = ?
        WHERE id = ? AND user_id = ?
        "#,
    )
    .bind(name)
    .bind(principal)
    .bind(rate)
    .bind(term_years)
    .bind(&start)
    .bind(built.payment)
    .bind(built.total_interest)
    .bind(id)
    .bind(user_key(user_id))
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Profile not found".into()));
    }

    load_profile(pool, user_id, id)
        .await?
        .ok_or_else(|| AppError::NotFound("Profile not found".into()))
}

pub async fn rename_profile(
    pool: &SqlitePool,
    user_id: Uuid,
    id: &str,
    name: &str,
) -> AppResult<()> {
    require_owned_profile(pool, user_id, id).await?;
    ensure_unique_name(pool, user_id, name, Some(id)).await?;
    let result = sqlx::query("UPDATE profiles SET name = ? WHERE id = ? AND user_id = ?")
        .bind(name)
        .bind(id)
        .bind(user_key(user_id))
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Profile not found".into()));
    }
    Ok(())
}

pub async fn delete_profile(pool: &SqlitePool, user_id: Uuid, id: &str) -> AppResult<()> {
    require_owned_profile(pool, user_id, id).await?;

    sqlx::query("DELETE FROM paid_keys WHERE profile_id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM payment_notes WHERE profile_id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM extras WHERE profile_id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    let result = sqlx::query("DELETE FROM profiles WHERE id = ? AND user_id = ?")
        .bind(id)
        .bind(user_key(user_id))
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Profile not found".into()));
    }

    let active = get_active_profile_id(pool, user_id).await?;
    if active.as_deref() == Some(id) {
        let next = list_profiles(pool, user_id).await?;
        set_active_profile(pool, user_id, next.first().map(|p| p.id.as_str())).await?;
    }
    Ok(())
}

pub async fn toggle_paid(
    pool: &SqlitePool,
    user_id: Uuid,
    profile_id: &str,
    pay_key: &str,
) -> AppResult<bool> {
    require_owned_profile(pool, user_id, profile_id).await?;

    let existing: Option<(String,)> =
        sqlx::query_as("SELECT pay_key FROM paid_keys WHERE profile_id = ? AND pay_key = ?")
            .bind(profile_id)
            .bind(pay_key)
            .fetch_optional(pool)
            .await?;

    if existing.is_some() {
        sqlx::query("DELETE FROM paid_keys WHERE profile_id = ? AND pay_key = ?")
            .bind(profile_id)
            .bind(pay_key)
            .execute(pool)
            .await?;
        Ok(false)
    } else {
        sqlx::query("INSERT INTO paid_keys (profile_id, pay_key) VALUES (?, ?)")
            .bind(profile_id)
            .bind(pay_key)
            .execute(pool)
            .await?;
        Ok(true)
    }
}

pub async fn clear_paid(pool: &SqlitePool, user_id: Uuid, profile_id: &str) -> AppResult<()> {
    require_owned_profile(pool, user_id, profile_id).await?;
    sqlx::query("DELETE FROM paid_keys WHERE profile_id = ?")
        .bind(profile_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn mark_due_paid(
    pool: &SqlitePool,
    user_id: Uuid,
    profile_id: &str,
    pay_keys: &[String],
) -> AppResult<()> {
    require_owned_profile(pool, user_id, profile_id).await?;
    for key in pay_keys {
        sqlx::query("INSERT OR IGNORE INTO paid_keys (profile_id, pay_key) VALUES (?, ?)")
            .bind(profile_id)
            .bind(key)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn add_extra(
    pool: &SqlitePool,
    user_id: Uuid,
    profile_id: &str,
    date: NaiveDate,
    amount: f64,
) -> AppResult<ExtraPayment> {
    require_owned_profile(pool, user_id, profile_id).await?;
    if amount <= 0.0 {
        return Err(AppError::BadRequest("Amount must be positive".into()));
    }
    let id = Uuid::new_v4().to_string();
    let date_str = date.format("%Y-%m-%d").to_string();
    sqlx::query("INSERT INTO extras (id, profile_id, date, amount) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(profile_id)
        .bind(&date_str)
        .bind(amount)
        .execute(pool)
        .await?;

    let pay_key = format!("extra:{id}");
    sqlx::query("INSERT OR IGNORE INTO paid_keys (profile_id, pay_key) VALUES (?, ?)")
        .bind(profile_id)
        .bind(&pay_key)
        .execute(pool)
        .await?;

    refresh_loan_totals(pool, user_id, profile_id).await?;

    Ok(ExtraPayment {
        id,
        profile_id: profile_id.to_string(),
        date: date_str,
        amount,
    })
}

pub async fn delete_extra(
    pool: &SqlitePool,
    user_id: Uuid,
    profile_id: &str,
    extra_id: &str,
) -> AppResult<()> {
    require_owned_profile(pool, user_id, profile_id).await?;
    let pay_key = format!("extra:{extra_id}");
    sqlx::query("DELETE FROM paid_keys WHERE profile_id = ? AND pay_key = ?")
        .bind(profile_id)
        .bind(&pay_key)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM payment_notes WHERE profile_id = ? AND pay_key = ?")
        .bind(profile_id)
        .bind(&pay_key)
        .execute(pool)
        .await?;
    let result = sqlx::query("DELETE FROM extras WHERE id = ? AND profile_id = ?")
        .bind(extra_id)
        .bind(profile_id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Extra payment not found".into()));
    }
    refresh_loan_totals(pool, user_id, profile_id).await?;
    Ok(())
}

async fn refresh_loan_totals(
    pool: &SqlitePool,
    user_id: Uuid,
    profile_id: &str,
) -> AppResult<()> {
    let profile = load_profile(pool, user_id, profile_id)
        .await?
        .ok_or_else(|| AppError::NotFound("Profile not found".into()))?;
    let Some(loan) = profile.loan() else {
        return Ok(());
    };
    let extras = list_extras(pool, profile_id).await?;
    let extra_tuples: Vec<(String, NaiveDate, f64)> = extras
        .iter()
        .filter_map(|ex| {
            let date = NaiveDate::parse_from_str(&ex.date, "%Y-%m-%d").ok()?;
            Some((ex.id.clone(), date, ex.amount))
        })
        .collect();
    let built = build_schedule(
        loan.principal,
        loan.rate,
        loan.term_years,
        loan.start_date,
        &extra_tuples,
    );
    sqlx::query(
        "UPDATE profiles SET monthly_payment = ?, total_interest = ? WHERE id = ? AND user_id = ?",
    )
    .bind(built.payment)
    .bind(built.total_interest)
    .bind(profile_id)
    .bind(user_key(user_id))
    .execute(pool)
    .await?;
    Ok(())
}

async fn ensure_unique_name(
    pool: &SqlitePool,
    user_id: Uuid,
    name: &str,
    except_id: Option<&str>,
) -> AppResult<()> {
    let existing: Option<(String,)> = if let Some(id) = except_id {
        sqlx::query_as(
            "SELECT id FROM profiles WHERE user_id = ? AND lower(name) = lower(?) AND id != ?",
        )
        .bind(user_key(user_id))
        .bind(name)
        .bind(id)
        .fetch_optional(pool)
        .await?
    } else {
        sqlx::query_as(
            "SELECT id FROM profiles WHERE user_id = ? AND lower(name) = lower(?)",
        )
        .bind(user_key(user_id))
        .bind(name)
        .fetch_optional(pool)
        .await?
    };

    if existing.is_some() {
        return Err(AppError::BadRequest(
            "A profile with that name already exists.".into(),
        ));
    }
    Ok(())
}
