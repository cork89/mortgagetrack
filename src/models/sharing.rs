//! Profile share links and collaborators.

use argon2::password_hash::rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::error::{AppError, AppResult};

use super::db::{require_owned_profile, require_profile_access, user_key, ProfileRole};

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ShareLinkStatus {
    #[allow(dead_code)]
    pub id: String,
    pub expires_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CollaboratorRow {
    pub user_id: String,
    pub email: String,
    pub avatar: Option<String>,
    #[allow(dead_code)]
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct CreatedShareLink {
    pub expires_at: String,
    pub path: String,
}

fn hash_token(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn generate_token() -> String {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Create a new 24h single-use invite. Revokes any prior unused active invite.
pub async fn create_share_link(
    pool: &SqlitePool,
    owner_id: Uuid,
    profile_id: &str,
) -> AppResult<CreatedShareLink> {
    require_owned_profile(pool, owner_id, profile_id).await?;

    sqlx::query(
        r#"
        UPDATE profile_share_links
        SET revoked_at = datetime('now')
        WHERE profile_id = ?
          AND used_at IS NULL
          AND revoked_at IS NULL
          AND expires_at > datetime('now')
        "#,
    )
    .bind(profile_id)
    .execute(pool)
    .await?;

    let id = Uuid::new_v4().to_string();
    let token = generate_token();
    let token_hash = hash_token(&token);

    sqlx::query(
        r#"
        INSERT INTO profile_share_links (id, profile_id, created_by, token_hash, expires_at)
        VALUES (?, ?, ?, ?, datetime('now', '+24 hours'))
        "#,
    )
    .bind(&id)
    .bind(profile_id)
    .bind(user_key(owner_id))
    .bind(&token_hash)
    .execute(pool)
    .await?;

    let expires_at: (String,) =
        sqlx::query_as("SELECT expires_at FROM profile_share_links WHERE id = ?")
            .bind(&id)
            .fetch_one(pool)
            .await?;

    Ok(CreatedShareLink {
        path: format!("/share/{token}"),
        expires_at: expires_at.0,
    })
}

pub async fn active_share_link(
    pool: &SqlitePool,
    owner_id: Uuid,
    profile_id: &str,
) -> AppResult<Option<ShareLinkStatus>> {
    require_owned_profile(pool, owner_id, profile_id).await?;
    let row = sqlx::query_as::<_, ShareLinkStatus>(
        r#"
        SELECT id, expires_at
        FROM profile_share_links
        WHERE profile_id = ?
          AND used_at IS NULL
          AND revoked_at IS NULL
          AND expires_at > datetime('now')
        ORDER BY created_at DESC
        LIMIT 1
        "#,
    )
    .bind(profile_id)
    .fetch_optional(pool)
    .await?;
    Ok(row)
}

pub async fn list_collaborators(
    pool: &SqlitePool,
    owner_id: Uuid,
    profile_id: &str,
) -> AppResult<Vec<CollaboratorRow>> {
    require_owned_profile(pool, owner_id, profile_id).await?;
    let rows = sqlx::query_as::<_, CollaboratorRow>(
        r#"
        SELECT c.user_id, u.email, u.avatar, c.created_at
        FROM profile_collaborators c
        INNER JOIN users u ON u.id = c.user_id
        WHERE c.profile_id = ?
        ORDER BY u.email COLLATE NOCASE
        "#,
    )
    .bind(profile_id)
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Accept a share token for the current user. Returns the profile id.
pub async fn accept_share_link(
    pool: &SqlitePool,
    user_id: Uuid,
    token: &str,
) -> AppResult<String> {
    let token = token.trim();
    if token.is_empty() || token.len() > 128 {
        return Err(AppError::NotFound("Invite not found".into()));
    }
    let token_hash = hash_token(token);

    let row: Option<(String, String, String)> = sqlx::query_as(
        r#"
        SELECT id, profile_id, created_by
        FROM profile_share_links
        WHERE token_hash = ?
          AND used_at IS NULL
          AND revoked_at IS NULL
          AND expires_at > datetime('now')
        "#,
    )
    .bind(&token_hash)
    .fetch_optional(pool)
    .await?;

    let Some((link_id, profile_id, created_by)) = row else {
        return Err(AppError::NotFound("Invite not found".into()));
    };

    if created_by == user_key(user_id) {
        return Err(AppError::BadRequest(
            "You already own this profile.".into(),
        ));
    }

    let already: Option<(String,)> = sqlx::query_as(
        "SELECT user_id FROM profile_collaborators WHERE profile_id = ? AND user_id = ?",
    )
    .bind(&profile_id)
    .bind(user_key(user_id))
    .fetch_optional(pool)
    .await?;

    if already.is_some() {
        // Consume the link anyway so it cannot be reused.
        sqlx::query(
            r#"
            UPDATE profile_share_links
            SET used_at = datetime('now')
            WHERE id = ? AND used_at IS NULL
            "#,
        )
        .bind(&link_id)
        .execute(pool)
        .await?;
        return Ok(profile_id);
    }

    let mut tx = pool.begin().await?;

    let mark = sqlx::query(
        r#"
        UPDATE profile_share_links
        SET used_at = datetime('now')
        WHERE id = ?
          AND used_at IS NULL
          AND revoked_at IS NULL
          AND expires_at > datetime('now')
        "#,
    )
    .bind(&link_id)
    .execute(&mut *tx)
    .await?;

    if mark.rows_affected() == 0 {
        return Err(AppError::NotFound("Invite not found".into()));
    }

    sqlx::query(
        r#"
        INSERT INTO profile_collaborators (profile_id, user_id, role, invited_by, share_link_id)
        VALUES (?, ?, 'editor', ?, ?)
        "#,
    )
    .bind(&profile_id)
    .bind(user_key(user_id))
    .bind(&created_by)
    .bind(&link_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(profile_id)
}

pub async fn remove_collaborator(
    pool: &SqlitePool,
    owner_id: Uuid,
    profile_id: &str,
    collaborator_user_id: &str,
) -> AppResult<()> {
    require_owned_profile(pool, owner_id, profile_id).await?;
    let result = sqlx::query(
        "DELETE FROM profile_collaborators WHERE profile_id = ? AND user_id = ?",
    )
    .bind(profile_id)
    .bind(collaborator_user_id)
    .execute(pool)
    .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Collaborator not found".into()));
    }
    Ok(())
}

/// Editor leaves a shared profile.
pub async fn leave_profile(
    pool: &SqlitePool,
    user_id: Uuid,
    profile_id: &str,
) -> AppResult<()> {
    let role = require_profile_access(pool, user_id, profile_id).await?;
    if role != ProfileRole::Editor {
        return Err(AppError::BadRequest(
            "Owners cannot leave their own profile.".into(),
        ));
    }
    sqlx::query("DELETE FROM profile_collaborators WHERE profile_id = ? AND user_id = ?")
        .bind(profile_id)
        .bind(user_key(user_id))
        .execute(pool)
        .await?;
    Ok(())
}
