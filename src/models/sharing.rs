//! Profile share links and collaborators.

use argon2::password_hash::rand_core::{OsRng, RngCore};
use crate::db::params;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::db::{
    begin, execute, get_conn, query_all, query_one, query_optional, DbPool, FromRow,
};
use crate::error::{AppError, AppResult};

use super::db::{require_owned_profile, require_profile_access, user_key, ProfileRole};

#[derive(Debug, Clone)]
pub struct ShareLinkStatus {
    #[allow(dead_code)]
    pub id: String,
    pub expires_at: String,
}

impl FromRow for ShareLinkStatus {
    fn from_row(row: &crate::db::Row) -> crate::error::AppResult<Self> {
        Ok(Self {
            id: row.get(0)?,
            expires_at: row.get(1)?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct CollaboratorRow {
    pub user_id: String,
    pub email: String,
    pub avatar: Option<String>,
    #[allow(dead_code)]
    pub created_at: String,
}

impl FromRow for CollaboratorRow {
    fn from_row(row: &crate::db::Row) -> crate::error::AppResult<Self> {
        Ok(Self {
            user_id: row.get(0)?,
            email: row.get(1)?,
            avatar: row.get(2)?,
            created_at: row.get(3)?,
        })
    }
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
    pool: &DbPool,
    owner_id: Uuid,
    profile_id: &str,
) -> AppResult<CreatedShareLink> {
    require_owned_profile(pool, owner_id, profile_id).await?;

    let conn = get_conn(pool).await?;
    execute(
        &conn,
        r#"
        UPDATE profile_share_links
        SET revoked_at = datetime('now')
        WHERE profile_id = ?
          AND used_at IS NULL
          AND revoked_at IS NULL
          AND expires_at > datetime('now')
        "#,
        params![profile_id],
    )
    .await?;

    let id = Uuid::new_v4().to_string();
    let token = generate_token();
    let token_hash = hash_token(&token);

    execute(
        &conn,
        r#"
        INSERT INTO profile_share_links (id, profile_id, created_by, token_hash, expires_at)
        VALUES (?, ?, ?, ?, datetime('now', '+24 hours'))
        "#,
        params![
            id.as_str(),
            profile_id,
            user_key(owner_id),
            token_hash.as_str()
        ],
    )
    .await?;

    let expires_at: (String,) = query_one(
        &conn,
        "SELECT expires_at FROM profile_share_links WHERE id = ?",
        params![id.as_str()],
    )
    .await?;

    Ok(CreatedShareLink {
        path: format!("/share/{token}"),
        expires_at: expires_at.0,
    })
}

pub async fn active_share_link(
    pool: &DbPool,
    owner_id: Uuid,
    profile_id: &str,
) -> AppResult<Option<ShareLinkStatus>> {
    require_owned_profile(pool, owner_id, profile_id).await?;
    let conn = get_conn(pool).await?;
    query_optional(
        &conn,
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
        params![profile_id],
    )
    .await
}

pub async fn list_collaborators(
    pool: &DbPool,
    owner_id: Uuid,
    profile_id: &str,
) -> AppResult<Vec<CollaboratorRow>> {
    require_owned_profile(pool, owner_id, profile_id).await?;
    let conn = get_conn(pool).await?;
    query_all(
        &conn,
        r#"
        SELECT c.user_id, u.email, u.avatar, c.created_at
        FROM profile_collaborators c
        INNER JOIN users u ON u.id = c.user_id
        WHERE c.profile_id = ?
        ORDER BY u.email COLLATE NOCASE
        "#,
        params![profile_id],
    )
    .await
}

/// Accept a share token for the current user. Returns the profile id.
pub async fn accept_share_link(
    pool: &DbPool,
    user_id: Uuid,
    token: &str,
) -> AppResult<String> {
    let token = token.trim();
    if token.is_empty() || token.len() > 128 {
        return Err(AppError::NotFound("Invite not found".into()));
    }
    let token_hash = hash_token(token);

    let conn = get_conn(pool).await?;
    let row: Option<(String, String, String)> = query_optional(
        &conn,
        r#"
        SELECT id, profile_id, created_by
        FROM profile_share_links
        WHERE token_hash = ?
          AND used_at IS NULL
          AND revoked_at IS NULL
          AND expires_at > datetime('now')
        "#,
        params![token_hash.as_str()],
    )
    .await?;

    let Some((link_id, profile_id, created_by)) = row else {
        return Err(AppError::NotFound("Invite not found".into()));
    };

    if created_by == user_key(user_id) {
        return Err(AppError::BadRequest(
            "You already own this profile.".into(),
        ));
    }

    let already: Option<(String,)> = query_optional(
        &conn,
        "SELECT user_id FROM profile_collaborators WHERE profile_id = ? AND user_id = ?",
        params![profile_id.as_str(), user_key(user_id)],
    )
    .await?;

    if already.is_some() {
        // Consume the link anyway so it cannot be reused.
        execute(
            &conn,
            r#"
            UPDATE profile_share_links
            SET used_at = datetime('now')
            WHERE id = ? AND used_at IS NULL
            "#,
            params![link_id.as_str()],
        )
        .await?;
        return Ok(profile_id);
    }

    let tx = begin(&conn).await?;

    let mark = execute(
        &tx,
        r#"
        UPDATE profile_share_links
        SET used_at = datetime('now')
        WHERE id = ?
          AND used_at IS NULL
          AND revoked_at IS NULL
          AND expires_at > datetime('now')
        "#,
        params![link_id.as_str()],
    )
    .await?;

    if mark == 0 {
        return Err(AppError::NotFound("Invite not found".into()));
    }

    execute(
        &tx,
        r#"
        INSERT INTO profile_collaborators (profile_id, user_id, role, invited_by, share_link_id)
        VALUES (?, ?, 'editor', ?, ?)
        "#,
        params![
            profile_id.as_str(),
            user_key(user_id),
            created_by.as_str(),
            link_id.as_str()
        ],
    )
    .await?;

    tx.commit().await?;
    Ok(profile_id)
}

pub async fn remove_collaborator(
    pool: &DbPool,
    owner_id: Uuid,
    profile_id: &str,
    collaborator_user_id: &str,
) -> AppResult<()> {
    require_owned_profile(pool, owner_id, profile_id).await?;
    let conn = get_conn(pool).await?;
    let result = execute(
        &conn,
        "DELETE FROM profile_collaborators WHERE profile_id = ? AND user_id = ?",
        params![profile_id, collaborator_user_id],
    )
    .await?;
    if result == 0 {
        return Err(AppError::NotFound("Collaborator not found".into()));
    }
    Ok(())
}

/// Editor leaves a shared profile.
pub async fn leave_profile(pool: &DbPool, user_id: Uuid, profile_id: &str) -> AppResult<()> {
    let role = require_profile_access(pool, user_id, profile_id).await?;
    if role != ProfileRole::Editor {
        return Err(AppError::BadRequest(
            "Owners cannot leave their own profile.".into(),
        ));
    }
    let conn = get_conn(pool).await?;
    execute(
        &conn,
        "DELETE FROM profile_collaborators WHERE profile_id = ? AND user_id = ?",
        params![profile_id, user_key(user_id)],
    )
    .await?;
    Ok(())
}
