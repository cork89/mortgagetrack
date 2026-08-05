use crate::db::params;

use crate::db::{execute, get_conn, query_all, query_optional, DbPool};
use crate::error::AppResult;

/// Rebuild `profiles` with a required `user_id` when upgrading from the pre-auth schema.
/// Unowned legacy profiles are dropped.
pub async fn ensure_profiles_belong_to_users(pool: &DbPool) -> AppResult<()> {
    if profiles_have_user_id(pool).await? {
        return Ok(());
    }

    tracing::info!("migrating profiles to be owned by users");

    let conn = get_conn(pool).await?;
    // Child tables keep their schema; clear rows that pointed at old profiles.
    execute(&conn, "DELETE FROM paid_keys", ()).await?;
    execute(&conn, "DELETE FROM extras", ()).await?;

    // SQLite won't DROP a parent while FK-enabled children reference it.
    execute(&conn, "PRAGMA foreign_keys = OFF", ()).await?;
    execute(&conn, "DROP TABLE IF EXISTS profiles", ()).await?;
    execute(
        &conn,
        r#"
        CREATE TABLE profiles (
            id TEXT PRIMARY KEY NOT NULL,
            user_id TEXT NOT NULL,
            name TEXT NOT NULL,
            principal REAL,
            rate REAL,
            term_years INTEGER,
            start_date TEXT,
            monthly_payment REAL,
            total_interest REAL,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE (user_id, name COLLATE NOCASE),
            FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
        )
        "#,
        (),
    )
    .await?;
    execute(&conn, "PRAGMA foreign_keys = ON", ()).await?;

    execute(
        &conn,
        "DELETE FROM settings WHERE key = 'active_profile_id'",
        (),
    )
    .await?;

    Ok(())
}

async fn profiles_have_user_id(pool: &DbPool) -> AppResult<bool> {
    let conn = get_conn(pool).await?;
    let cols: Vec<(i32, String)> = query_all(
        &conn,
        "SELECT cid, name FROM pragma_table_info('profiles')",
        (),
    )
    .await?;
    Ok(cols.iter().any(|(_, name)| name == "user_id"))
}

/// Add `profiles.version` for optimistic concurrency when upgrading.
pub async fn ensure_profile_version(pool: &DbPool) -> AppResult<()> {
    if profiles_have_version(pool).await? {
        return Ok(());
    }
    tracing::info!("adding profiles.version column");
    let conn = get_conn(pool).await?;
    execute(
        &conn,
        "ALTER TABLE profiles ADD COLUMN version INTEGER NOT NULL DEFAULT 1",
        (),
    )
    .await?;
    Ok(())
}

async fn profiles_have_version(pool: &DbPool) -> AppResult<bool> {
    let conn = get_conn(pool).await?;
    let cols: Vec<(i32, String)> = query_all(
        &conn,
        "SELECT cid, name FROM pragma_table_info('profiles')",
        (),
    )
    .await?;
    Ok(cols.iter().any(|(_, name)| name == "version"))
}

/// Add `extras.recast` so lump-sum extras can lower future payments instead of shortening term.
pub async fn ensure_extra_recast(pool: &DbPool) -> AppResult<()> {
    if extras_have_recast(pool).await? {
        return Ok(());
    }
    tracing::info!("adding extras.recast column");
    let conn = get_conn(pool).await?;
    execute(
        &conn,
        "ALTER TABLE extras ADD COLUMN recast INTEGER NOT NULL DEFAULT 0",
        (),
    )
    .await?;
    Ok(())
}

async fn extras_have_recast(pool: &DbPool) -> AppResult<bool> {
    let conn = get_conn(pool).await?;
    let cols: Vec<(i32, String)> =
        query_all(&conn, "SELECT cid, name FROM pragma_table_info('extras')", ()).await?;
    Ok(cols.iter().any(|(_, name)| name == "recast"))
}

/// Add `home_improvements.detail` for longer notes shown only in the edit modal.
pub async fn ensure_improvement_detail(pool: &DbPool) -> AppResult<()> {
    if !table_exists(pool, "home_improvements").await? {
        return Ok(());
    }
    if improvements_have_detail(pool).await? {
        return Ok(());
    }
    tracing::info!("adding home_improvements.detail column");
    let conn = get_conn(pool).await?;
    execute(
        &conn,
        "ALTER TABLE home_improvements ADD COLUMN detail TEXT NOT NULL DEFAULT ''",
        (),
    )
    .await?;
    Ok(())
}

async fn table_exists(pool: &DbPool, name: &str) -> AppResult<bool> {
    let conn = get_conn(pool).await?;
    let row: Option<(String,)> = query_optional(
        &conn,
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name = ?",
        params![name],
    )
    .await?;
    Ok(row.is_some())
}

async fn improvements_have_detail(pool: &DbPool) -> AppResult<bool> {
    let conn = get_conn(pool).await?;
    let cols: Vec<(i32, String)> = query_all(
        &conn,
        "SELECT cid, name FROM pragma_table_info('home_improvements')",
        (),
    )
    .await?;
    Ok(cols.iter().any(|(_, name)| name == "detail"))
}

pub async fn ensure_user_avatar(pool: &DbPool) -> AppResult<()> {
    if users_have_column(pool, "avatar").await? {
        return Ok(());
    }
    tracing::info!("adding users.avatar column");
    let conn = get_conn(pool).await?;
    execute(&conn, "ALTER TABLE users ADD COLUMN avatar TEXT", ()).await?;
    Ok(())
}

pub async fn ensure_user_default_tab(pool: &DbPool) -> AppResult<()> {
    if users_have_column(pool, "default_tab").await? {
        return Ok(());
    }
    tracing::info!("adding users.default_tab column");
    let conn = get_conn(pool).await?;
    execute(
        &conn,
        "ALTER TABLE users ADD COLUMN default_tab TEXT NOT NULL DEFAULT 'calendar'",
        (),
    )
    .await?;
    Ok(())
}

pub async fn ensure_user_payments_year_expand(pool: &DbPool) -> AppResult<()> {
    if users_have_column(pool, "payments_year_expand").await? {
        return Ok(());
    }
    tracing::info!("adding users.payments_year_expand column");
    let conn = get_conn(pool).await?;
    execute(
        &conn,
        "ALTER TABLE users ADD COLUMN payments_year_expand TEXT NOT NULL DEFAULT 'current'",
        (),
    )
    .await?;
    Ok(())
}

async fn users_have_column(pool: &DbPool, column: &str) -> AppResult<bool> {
    let conn = get_conn(pool).await?;
    let cols: Vec<(i32, String)> =
        query_all(&conn, "SELECT cid, name FROM pragma_table_info('users')", ()).await?;
    Ok(cols.iter().any(|(_, name)| name == column))
}
