use sqlx::SqlitePool;

/// Rebuild `profiles` with a required `user_id` when upgrading from the pre-auth schema.
/// Unowned legacy profiles are dropped.
pub async fn ensure_profiles_belong_to_users(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    if profiles_have_user_id(pool).await? {
        return Ok(());
    }

    tracing::info!("migrating profiles to be owned by users");

    // Child tables keep their schema; clear rows that pointed at old profiles.
    sqlx::query("DELETE FROM paid_keys").execute(pool).await?;
    sqlx::query("DELETE FROM extras").execute(pool).await?;

    // SQLite won't DROP a parent while FK-enabled children reference it.
    sqlx::query("PRAGMA foreign_keys = OFF").execute(pool).await?;
    sqlx::query("DROP TABLE IF EXISTS profiles").execute(pool).await?;
    sqlx::query(
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
    )
    .execute(pool)
    .await?;
    sqlx::query("PRAGMA foreign_keys = ON").execute(pool).await?;

    sqlx::query("DELETE FROM settings WHERE key = 'active_profile_id'")
        .execute(pool)
        .await?;

    Ok(())
}

async fn profiles_have_user_id(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let cols: Vec<(i32, String)> =
        sqlx::query_as("SELECT cid, name FROM pragma_table_info('profiles')")
            .fetch_all(pool)
            .await?;
    Ok(cols.iter().any(|(_, name)| name == "user_id"))
}

/// Add `profiles.version` for optimistic concurrency when upgrading.
pub async fn ensure_profile_version(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    if profiles_have_version(pool).await? {
        return Ok(());
    }
    tracing::info!("adding profiles.version column");
    sqlx::query("ALTER TABLE profiles ADD COLUMN version INTEGER NOT NULL DEFAULT 1")
        .execute(pool)
        .await?;
    Ok(())
}

async fn profiles_have_version(pool: &SqlitePool) -> Result<bool, sqlx::Error> {
    let cols: Vec<(i32, String)> =
        sqlx::query_as("SELECT cid, name FROM pragma_table_info('profiles')")
            .fetch_all(pool)
            .await?;
    Ok(cols.iter().any(|(_, name)| name == "version"))
}
