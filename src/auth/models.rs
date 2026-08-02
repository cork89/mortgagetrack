//! Auth domain models and database helpers.

use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::task::spawn_blocking;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub email: String,
    pub created_at: String,
    pub avatar: Option<String>,
    pub default_tab: String,
}

impl User {
    pub fn uuid(&self) -> AppResult<Uuid> {
        Uuid::parse_str(&self.id)
            .map_err(|_| AppError::Internal("invalid user id in database".into()))
    }
}

pub async fn hash_password(password: &str) -> AppResult<String> {
    let password = password.to_string();
    spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|err| {
                tracing::error!(error = %err, "failed to hash password");
                AppError::Internal("Failed to hash password".into())
            })
    })
    .await
    .map_err(|err| AppError::Internal(format!("hash task failed: {err}")))?
}

pub async fn verify_password(password: &str, password_hash: &str) -> AppResult<bool> {
    let password = password.to_string();
    let password_hash = password_hash.to_string();
    spawn_blocking(move || {
        let parsed = PasswordHash::new(&password_hash).map_err(|err| {
            tracing::error!(error = %err, "invalid password hash in database");
            AppError::Internal("Invalid stored password hash".into())
        })?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|err| AppError::Internal(format!("verify task failed: {err}")))?
}

pub fn validate_email(email: &str) -> AppResult<&str> {
    let email = email.trim();
    if email.is_empty() || !email.contains('@') || email.len() > 255 {
        return Err(AppError::BadRequest("Enter a valid email address.".into()));
    }
    Ok(email)
}

pub fn validate_password(password: &str) -> AppResult<()> {
    if password.is_empty() {
        return Err(AppError::BadRequest("Enter a password.".into()));
    }
    if password.len() < 8 {
        return Err(AppError::BadRequest(
            "Password must be at least 8 characters.".into(),
        ));
    }
    if password.len() > 128 {
        return Err(AppError::BadRequest("Password is too long.".into()));
    }
    Ok(())
}

pub async fn find_user_by_id(pool: &SqlitePool, id: Uuid) -> AppResult<Option<User>> {
    let row = sqlx::query_as::<_, User>(
        r#"
        SELECT id, email, created_at, avatar, default_tab
        FROM users
        WHERE id = ?
        "#,
    )
    .bind(id.to_string())
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn find_user_by_email(pool: &SqlitePool, email: &str) -> AppResult<Option<(User, String)>> {
    let row = sqlx::query_as::<_, (String, String, String, Option<String>, String, String)>(
        r#"
        SELECT u.id, u.email, u.created_at, u.avatar, u.default_tab, c.password_hash
        FROM users u
        INNER JOIN local_credentials c ON c.user_id = u.id
        WHERE u.email = ? COLLATE NOCASE
        "#,
    )
    .bind(email)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|(id, email, created_at, avatar, default_tab, password_hash)| {
        (
            User {
                id,
                email,
                created_at,
                avatar,
                default_tab,
            },
            password_hash,
        )
    }))
}

pub async fn update_default_tab(
    pool: &SqlitePool,
    user_id: Uuid,
    default_tab: &str,
) -> AppResult<()> {
    let result = sqlx::query("UPDATE users SET default_tab = ? WHERE id = ?")
        .bind(default_tab)
        .bind(user_id.to_string())
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Account not found.".into()));
    }
    Ok(())
}

pub async fn create_user(pool: &SqlitePool, email: &str, password: &str) -> AppResult<User> {
    if find_user_by_email(pool, email).await?.is_some() {
        return Err(AppError::BadRequest(
            "An account with that email already exists.".into(),
        ));
    }

    let id = Uuid::new_v4();
    let password_hash = hash_password(password).await?;

    let mut tx = pool.begin().await?;

    sqlx::query(
        r#"
        INSERT INTO users (id, email)
        VALUES (?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(email)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
        INSERT INTO local_credentials (user_id, password_hash)
        VALUES (?, ?)
        "#,
    )
    .bind(id.to_string())
    .bind(&password_hash)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    find_user_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::Internal("User vanished after create".into()))
}

pub async fn password_hash_for_user(pool: &SqlitePool, user_id: Uuid) -> AppResult<Option<String>> {
    let row: Option<(String,)> = sqlx::query_as(
        r#"
        SELECT password_hash
        FROM local_credentials
        WHERE user_id = ?
        "#,
    )
    .bind(user_id.to_string())
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(hash,)| hash))
}

pub async fn update_password(
    pool: &SqlitePool,
    user_id: Uuid,
    new_password: &str,
) -> AppResult<()> {
    let password_hash = hash_password(new_password).await?;
    let result = sqlx::query(
        r#"
        UPDATE local_credentials
        SET password_hash = ?
        WHERE user_id = ?
        "#,
    )
    .bind(&password_hash)
    .bind(user_id.to_string())
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::Internal("No credentials for user".into()));
    }
    Ok(())
}

pub async fn delete_user(pool: &SqlitePool, user_id: Uuid) -> AppResult<()> {
    let result = sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(user_id.to_string())
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Account not found.".into()));
    }
    Ok(())
}
