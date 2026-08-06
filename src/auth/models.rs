//! Auth domain models and database helpers.

use argon2::password_hash::{rand_core::OsRng, SaltString};
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use crate::db::params;
use serde::{Deserialize, Serialize};
use tokio::task::spawn_blocking;
use uuid::Uuid;

use crate::db::{begin, execute, get_conn, query_optional, DbPool, FromRow};
use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub email: String,
    pub created_at: String,
    pub avatar: Option<String>,
    pub default_tab: String,
    pub payments_year_expand: String,
}

impl FromRow for User {
    fn from_row(row: &crate::db::Row) -> crate::error::AppResult<Self> {
        Ok(Self {
            id: row.get(0)?,
            email: row.get(1)?,
            created_at: row.get(2)?,
            avatar: row.get(3)?,
            default_tab: row.get(4)?,
            payments_year_expand: row.get(5)?,
        })
    }
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

pub async fn find_user_by_id(pool: &DbPool, id: Uuid) -> AppResult<Option<User>> {
    let conn = get_conn(pool).await?;
    query_optional(
        &conn,
        r#"
        SELECT id, email, created_at, avatar, default_tab, payments_year_expand
        FROM users
        WHERE id = ?
        "#,
        params![id.to_string()],
    )
    .await
}

/// Upsert the domain `users` projection for an edge-authenticated identity.
///
/// Used when Better Auth already created the account (Worker D1) but this DB
/// (e.g. local container SQLite) does not yet have the row.
pub async fn ensure_user_projection(pool: &DbPool, id: Uuid, email: &str) -> AppResult<User> {
    let email = validate_email(email)?;
    if let Some(user) = find_user_by_id(pool, id).await? {
        if user.email != email {
            let conn = get_conn(pool).await?;
            execute(
                &conn,
                "UPDATE users SET email = ? WHERE id = ?",
                params![email, id.to_string()],
            )
            .await?;
            return find_user_by_id(pool, id)
                .await?
                .ok_or_else(|| AppError::Internal("user missing after email update".into()));
        }
        return Ok(user);
    }

    let conn = get_conn(pool).await?;
    execute(
        &conn,
        r#"
        INSERT INTO users (id, email)
        VALUES (?, ?)
        ON CONFLICT(id) DO UPDATE SET email = excluded.email
        "#,
        params![id.to_string(), email],
    )
    .await?;

    find_user_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::Internal("user missing after projection upsert".into()))
}

struct UserWithHash {
    user: User,
    password_hash: String,
}

impl FromRow for UserWithHash {
    fn from_row(row: &crate::db::Row) -> AppResult<Self> {
        Ok(Self {
            user: User {
                id: row.get(0)?,
                email: row.get(1)?,
                created_at: row.get(2)?,
                avatar: row.get(3)?,
                default_tab: row.get(4)?,
                payments_year_expand: row.get(5)?,
            },
            password_hash: row.get(6)?,
        })
    }
}

pub async fn find_user_by_email(pool: &DbPool, email: &str) -> AppResult<Option<(User, String)>> {
    let conn = get_conn(pool).await?;
    let row: Option<UserWithHash> = query_optional(
        &conn,
        r#"
            SELECT u.id, u.email, u.created_at, u.avatar, u.default_tab, u.payments_year_expand,
                   c.password_hash
            FROM users u
            INNER JOIN local_credentials c ON c.user_id = u.id
            WHERE u.email = ? COLLATE NOCASE
            "#,
        params![email],
    )
    .await?;
    Ok(row.map(|r| (r.user, r.password_hash)))
}

pub async fn update_default_tab(
    pool: &DbPool,
    user_id: Uuid,
    default_tab: &str,
) -> AppResult<()> {
    let conn = get_conn(pool).await?;
    let result = execute(
        &conn,
        "UPDATE users SET default_tab = ? WHERE id = ?",
        params![default_tab, user_id.to_string()],
    )
    .await?;
    if result == 0 {
        return Err(AppError::NotFound("Account not found.".into()));
    }
    Ok(())
}

pub async fn update_payments_year_expand(
    pool: &DbPool,
    user_id: Uuid,
    payments_year_expand: &str,
) -> AppResult<()> {
    let conn = get_conn(pool).await?;
    let result = execute(
        &conn,
        "UPDATE users SET payments_year_expand = ? WHERE id = ?",
        params![payments_year_expand, user_id.to_string()],
    )
    .await?;
    if result == 0 {
        return Err(AppError::NotFound("Account not found.".into()));
    }
    Ok(())
}

pub async fn update_avatar(pool: &DbPool, user_id: Uuid, avatar: &str) -> AppResult<()> {
    let conn = get_conn(pool).await?;
    let result = execute(
        &conn,
        "UPDATE users SET avatar = ? WHERE id = ?",
        params![avatar, user_id.to_string()],
    )
    .await?;
    if result == 0 {
        return Err(AppError::NotFound("Account not found.".into()));
    }
    Ok(())
}

pub async fn create_user(pool: &DbPool, email: &str, password: &str) -> AppResult<User> {
    if find_user_by_email(pool, email).await?.is_some() {
        return Err(AppError::BadRequest(
            "An account with that email already exists.".into(),
        ));
    }

    let id = Uuid::new_v4();
    let password_hash = hash_password(password).await?;

    let conn = get_conn(pool).await?;
    let tx = begin(&conn).await?;

    execute(
        &tx,
        r#"
        INSERT INTO users (id, email)
        VALUES (?, ?)
        "#,
        params![id.to_string(), email],
    )
    .await?;

    execute(
        &tx,
        r#"
        INSERT INTO local_credentials (user_id, password_hash)
        VALUES (?, ?)
        "#,
        params![id.to_string(), password_hash.as_str()],
    )
    .await?;

    tx.commit().await?;

    find_user_by_id(pool, id)
        .await?
        .ok_or_else(|| AppError::Internal("User vanished after create".into()))
}

pub async fn password_hash_for_user(pool: &DbPool, user_id: Uuid) -> AppResult<Option<String>> {
    let conn = get_conn(pool).await?;
    let row: Option<(String,)> = query_optional(
        &conn,
        r#"
        SELECT password_hash
        FROM local_credentials
        WHERE user_id = ?
        "#,
        params![user_id.to_string()],
    )
    .await?;
    Ok(row.map(|(hash,)| hash))
}

pub async fn update_password(
    pool: &DbPool,
    user_id: Uuid,
    new_password: &str,
) -> AppResult<()> {
    let password_hash = hash_password(new_password).await?;
    let conn = get_conn(pool).await?;
    let result = execute(
        &conn,
        r#"
        UPDATE local_credentials
        SET password_hash = ?
        WHERE user_id = ?
        "#,
        params![password_hash.as_str(), user_id.to_string()],
    )
    .await?;

    if result == 0 {
        return Err(AppError::Internal("No credentials for user".into()));
    }
    Ok(())
}

pub async fn delete_user(pool: &DbPool, user_id: Uuid) -> AppResult<()> {
    let conn = get_conn(pool).await?;
    let result = execute(
        &conn,
        "DELETE FROM users WHERE id = ?",
        params![user_id.to_string()],
    )
    .await?;

    if result == 0 {
        return Err(AppError::NotFound("Account not found.".into()));
    }
    Ok(())
}
