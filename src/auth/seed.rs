use super::models::{create_user, find_user_by_email, validate_email, validate_password};
use crate::config::env_bool;
use crate::db::DbPool;
use crate::error::AppResult;

const TEST_USERS: &[(&str, &str)] = &[
    ("TEST_USER_EMAIL", "TEST_USER_PASSWORD"),
    ("TEST_USER_2_EMAIL", "TEST_USER_2_PASSWORD"),
];

/// Create test/seed users from env vars when enabled.
///
/// Requires `ALLOW_DEV_SEED_USERS=true` plus both email and password for each user.
/// Skips emails that already exist. Intended for local development only.
pub async fn ensure_test_user(pool: &DbPool) -> AppResult<()> {
    if !env_bool("ALLOW_DEV_SEED_USERS", false) {
        return Ok(());
    }
    tracing::warn!("ALLOW_DEV_SEED_USERS is enabled; seeding development accounts from env");
    for &(email_key, password_key) in TEST_USERS {
        ensure_one(pool, email_key, password_key).await?;
    }
    Ok(())
}

async fn ensure_one(pool: &DbPool, email_key: &str, password_key: &str) -> AppResult<()> {
    let email = match std::env::var(email_key) {
        Ok(value) if !value.trim().is_empty() => value,
        _ => return Ok(()),
    };
    let password = match std::env::var(password_key) {
        Ok(value) if !value.is_empty() => value,
        _ => {
            tracing::warn!(
                "{email_key} is set but {password_key} is missing; skipping seed user"
            );
            return Ok(());
        }
    };

    let email = validate_email(&email)?.to_string();
    validate_password(&password)?;

    if find_user_by_email(pool, &email).await?.is_some() {
        tracing::debug!(%email, "test user already exists");
        return Ok(());
    }

    create_user(pool, &email, &password).await?;
    tracing::info!(%email, "seeded test user from .env");
    Ok(())
}
