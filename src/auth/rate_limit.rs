//! Database-backed rate limits for login / register.

use axum::http::HeaderMap;
use std::net::SocketAddr;
use time::{Duration, OffsetDateTime};

use crate::db::{execute, get_conn, params, query_optional, DbPool, FromRow};
use crate::error::{AppError, AppResult};

const MAX_ATTEMPTS: i64 = 10;
const WINDOW_SECS: i64 = 15 * 60;

const KEY_LOGIN_IP: &str = "mortgagetrack:rl:login:ip:";
const KEY_LOGIN_EMAIL: &str = "mortgagetrack:rl:login:email:";
const KEY_REGISTER_IP: &str = "mortgagetrack:rl:register:ip:";
const KEY_REGISTER_EMAIL: &str = "mortgagetrack:rl:register:email:";
const KEY_RESET_IP: &str = "mortgagetrack:rl:reset:ip:";
const KEY_RESET_EMAIL: &str = "mortgagetrack:rl:reset:email:";

pub async fn ensure_schema(pool: &DbPool) -> AppResult<()> {
    let conn = get_conn(pool).await?;
    execute(
        &conn,
        r#"
        CREATE TABLE IF NOT EXISTS rate_limits (
            key TEXT PRIMARY KEY NOT NULL,
            count INTEGER NOT NULL,
            expires_at TEXT NOT NULL
        )
        "#,
        (),
    )
    .await?;
    Ok(())
}

/// Best-effort client IP for rate-limit keys.
pub fn client_ip(headers: &HeaderMap, peer: Option<SocketAddr>) -> String {
    if let Some(fwd) = headers.get("x-forwarded-for").and_then(|v| v.to_str().ok()) {
        if let Some(first) = fwd.split(',').next() {
            let trimmed = first.trim();
            if !trimmed.is_empty() {
                return trimmed.to_string();
            }
        }
    }
    if let Some(real) = headers.get("x-real-ip").and_then(|v| v.to_str().ok()) {
        let trimmed = real.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    peer.map(|a| a.ip().to_string())
        .unwrap_or_else(|| "unknown".into())
}

pub async fn check_login(pool: &DbPool, ip: &str, email: &str) -> AppResult<()> {
    bump(pool, &format!("{KEY_LOGIN_IP}{ip}")).await?;
    bump(
        pool,
        &format!("{KEY_LOGIN_EMAIL}{}", normalize_email(email)),
    )
    .await
}

pub async fn clear_login_email(pool: &DbPool, email: &str) -> AppResult<()> {
    let key = format!("{KEY_LOGIN_EMAIL}{}", normalize_email(email));
    let conn = get_conn(pool).await?;
    execute(&conn, "DELETE FROM rate_limits WHERE key = ?", params![key]).await?;
    Ok(())
}

pub async fn check_register(pool: &DbPool, ip: &str, email: &str) -> AppResult<()> {
    bump(pool, &format!("{KEY_REGISTER_IP}{ip}")).await?;
    bump(
        pool,
        &format!("{KEY_REGISTER_EMAIL}{}", normalize_email(email)),
    )
    .await
}

pub async fn check_password_reset(pool: &DbPool, ip: &str, email: &str) -> AppResult<()> {
    bump(pool, &format!("{KEY_RESET_IP}{ip}")).await?;
    bump(
        pool,
        &format!("{KEY_RESET_EMAIL}{}", normalize_email(email)),
    )
    .await
}

struct RateRow {
    count: i64,
    expires_at: String,
}

impl FromRow for RateRow {
    fn from_row(row: &crate::db::Row) -> AppResult<Self> {
        Ok(Self {
            count: row.get(0)?,
            expires_at: row.get(1)?,
        })
    }
}

async fn bump(pool: &DbPool, key: &str) -> AppResult<()> {
    let conn = get_conn(pool).await?;
    let now = OffsetDateTime::now_utc();

    let existing: Option<RateRow> = query_optional(
        &conn,
        "SELECT count, expires_at FROM rate_limits WHERE key = ?",
        params![key],
    )
    .await?;

    let count = if let Some(row) = existing {
        let expires = OffsetDateTime::parse(
            &row.expires_at,
            &time::format_description::well_known::Rfc3339,
        )
        .map_err(|err| AppError::Internal(format!("rate-limit expiry parse failed: {err}")))?;
        if expires <= now {
            let expires_at = format_rfc3339(now + Duration::seconds(WINDOW_SECS))?;
            execute(
                &conn,
                "UPDATE rate_limits SET count = 1, expires_at = ? WHERE key = ?",
                params![expires_at, key],
            )
            .await?;
            1
        } else {
            let next = row.count + 1;
            execute(
                &conn,
                "UPDATE rate_limits SET count = ? WHERE key = ?",
                params![next, key],
            )
            .await?;
            next
        }
    } else {
        let expires_at = format_rfc3339(now + Duration::seconds(WINDOW_SECS))?;
        execute(
            &conn,
            "INSERT INTO rate_limits (key, count, expires_at) VALUES (?, 1, ?)",
            params![key, expires_at],
        )
        .await?;
        1
    };

    if count > MAX_ATTEMPTS {
        return Err(AppError::TooManyRequests(
            "Too many attempts. Try again in a few minutes.".into(),
        ));
    }
    Ok(())
}

fn format_rfc3339(dt: OffsetDateTime) -> AppResult<String> {
    dt.format(&time::format_description::well_known::Rfc3339)
        .map_err(|err| AppError::Internal(err.to_string()))
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}
