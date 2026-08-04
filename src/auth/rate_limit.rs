//! Redis-backed rate limits for login / register.

use axum::http::HeaderMap;
use std::net::SocketAddr;
use tower_sessions_redis_store::fred::prelude::*;

use crate::error::{AppError, AppResult};
use crate::redis::RedisPool;

const MAX_ATTEMPTS: i64 = 10;
const WINDOW_SECS: i64 = 15 * 60;

const KEY_LOGIN_IP: &str = "homeabell:rl:login:ip:";
const KEY_LOGIN_EMAIL: &str = "homeabell:rl:login:email:";
const KEY_REGISTER_IP: &str = "homeabell:rl:register:ip:";
const KEY_REGISTER_EMAIL: &str = "homeabell:rl:register:email:";

/// Best-effort client IP for rate-limit keys.
pub fn client_ip(headers: &HeaderMap, peer: Option<SocketAddr>) -> String {
    if let Some(fwd) = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
    {
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

pub async fn check_login(redis: &RedisPool, ip: &str, email: &str) -> AppResult<()> {
    bump(redis, &format!("{KEY_LOGIN_IP}{ip}")).await?;
    bump(redis, &format!("{KEY_LOGIN_EMAIL}{}", normalize_email(email))).await
}

pub async fn clear_login_email(redis: &RedisPool, email: &str) -> AppResult<()> {
    let key = format!("{KEY_LOGIN_EMAIL}{}", normalize_email(email));
    let _: () = redis
        .del(key)
        .await
        .map_err(|err| AppError::Internal(format!("redis rate-limit clear failed: {err}")))?;
    Ok(())
}

pub async fn check_register(redis: &RedisPool, ip: &str, email: &str) -> AppResult<()> {
    bump(redis, &format!("{KEY_REGISTER_IP}{ip}")).await?;
    bump(
        redis,
        &format!("{KEY_REGISTER_EMAIL}{}", normalize_email(email)),
    )
    .await
}

async fn bump(redis: &RedisPool, key: &str) -> AppResult<()> {
    let count: i64 = redis
        .incr(key)
        .await
        .map_err(|err| AppError::Internal(format!("redis rate-limit incr failed: {err}")))?;

    if count == 1 {
        let _: () = redis
            .expire::<(), _>(key, WINDOW_SECS, None)
            .await
            .map_err(|err| AppError::Internal(format!("redis rate-limit expire failed: {err}")))?;
    }

    if count > MAX_ATTEMPTS {
        return Err(AppError::TooManyRequests(
            "Too many attempts. Try again in a few minutes.".into(),
        ));
    }
    Ok(())
}

fn normalize_email(email: &str) -> String {
    email.trim().to_ascii_lowercase()
}
