//! libSQL / Turso connection pool and query helpers.

use std::ops::Deref;
use std::sync::Arc;

use libsql::{params::IntoParams, Builder, Connection, Database, Rows};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::error::{AppError, AppResult};

pub type DbTx = libsql::Transaction;

/// Bounded connection factory (no recycle health-check queries).
#[derive(Clone)]
pub struct DbPool {
    db: Arc<Database>,
    limit: Arc<Semaphore>,
    /// Local SQLite needs this per connection; Turso remote does not.
    enable_foreign_keys: bool,
}

/// A checked-out connection; dropping it releases the pool permit.
pub struct DbConn {
    conn: Connection,
    _permit: OwnedSemaphorePermit,
}

impl Deref for DbConn {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

/// Map a libsql row into a typed value.
pub trait FromRow: Sized {
    fn from_row(row: &libsql::Row) -> Result<Self, libsql::Error>;
}

impl FromRow for (String,) {
    fn from_row(row: &libsql::Row) -> Result<Self, libsql::Error> {
        Ok((row.get(0)?,))
    }
}

impl FromRow for (String, String) {
    fn from_row(row: &libsql::Row) -> Result<Self, libsql::Error> {
        Ok((row.get(0)?, row.get(1)?))
    }
}

impl FromRow for (String, String, String) {
    fn from_row(row: &libsql::Row) -> Result<Self, libsql::Error> {
        Ok((row.get(0)?, row.get(1)?, row.get(2)?))
    }
}

impl FromRow for (i32, String) {
    fn from_row(row: &libsql::Row) -> Result<Self, libsql::Error> {
        Ok((row.get(0)?, row.get(1)?))
    }
}

pub async fn connect_db() -> Result<DbPool, Box<dyn std::error::Error>> {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:mortgage.db".into())
        .trim()
        .to_string();
    if url.is_empty() {
        return Err("DATABASE_URL is empty (check .env and that your shell is not exporting an empty DATABASE_URL)".into());
    }
    let remote = is_remote_url(&url);
    let database = if remote {
        let host = remote_host(&url).unwrap_or("");
        if host.is_empty() {
            return Err(format!(
                "DATABASE_URL is missing a hostname (got {url:?}). \
                 Expected libsql://YOUR-DB.turso.io"
            )
            .into());
        }
        let token = std::env::var("DATABASE_TOKEN")
            .map(|t| t.trim().to_string())
            .map_err(|_| {
                "DATABASE_TOKEN is required for remote Turso/libSQL URLs (libsql:// or https://)"
            })?;
        if token.is_empty() {
            return Err("DATABASE_TOKEN is empty".into());
        }
        Builder::new_remote(url.clone(), token)
            .build()
            .await
            .map_err(|err| {
                format!("failed to connect to Turso at {host}: {err}")
            })?
    } else {
        let path = normalize_sqlite_path(&url);
        Builder::new_local(path).build().await?
    };

    let pool = DbPool {
        db: Arc::new(database),
        limit: Arc::new(Semaphore::new(5)),
        enable_foreign_keys: !remote,
    };

    if pool.enable_foreign_keys {
        let conn = get_conn(&pool).await?;
        conn.execute_batch("PRAGMA foreign_keys = ON;").await?;
    }

    Ok(pool)
}

pub fn is_remote_url(url: &str) -> bool {
    let url = url.trim();
    url.starts_with("libsql://") || url.starts_with("https://") || url.starts_with("http://")
}

fn remote_host(url: &str) -> Option<&str> {
    let rest = url
        .strip_prefix("libsql://")
        .or_else(|| url.strip_prefix("https://"))
        .or_else(|| url.strip_prefix("http://"))?;
    let host = rest.split('/').next().unwrap_or("").split('?').next().unwrap_or("");
    if host.is_empty() {
        None
    } else {
        Some(host)
    }
}

fn normalize_sqlite_path(url: &str) -> String {
    let url = url.trim();
    url.strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))
        .unwrap_or(url)
        .to_string()
}

pub async fn get_conn(pool: &DbPool) -> AppResult<DbConn> {
    let permit = pool
        .limit
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::Database("database pool closed".into()))?;
    let conn = pool.db.connect().map_err(AppError::from)?;
    if pool.enable_foreign_keys {
        conn.execute_batch("PRAGMA foreign_keys = ON;").await?;
    }
    Ok(DbConn {
        conn,
        _permit: permit,
    })
}

pub async fn execute(
    conn: &Connection,
    sql: &str,
    params: impl IntoParams,
) -> AppResult<u64> {
    Ok(conn.execute(sql, params).await?)
}

pub async fn query_optional<T: FromRow>(
    conn: &Connection,
    sql: &str,
    params: impl IntoParams,
) -> AppResult<Option<T>> {
    let mut rows = conn.query(sql, params).await?;
    match rows.next().await? {
        Some(row) => Ok(Some(T::from_row(&row)?)),
        None => Ok(None),
    }
}

pub async fn query_one<T: FromRow>(
    conn: &Connection,
    sql: &str,
    params: impl IntoParams,
) -> AppResult<T> {
    query_optional(conn, sql, params)
        .await?
        .ok_or_else(|| AppError::Internal("expected a database row".into()))
}

pub async fn query_all<T: FromRow>(
    conn: &Connection,
    sql: &str,
    params: impl IntoParams,
) -> AppResult<Vec<T>> {
    let rows = conn.query(sql, params).await?;
    collect_rows(rows).await
}

async fn collect_rows<T: FromRow>(mut rows: Rows) -> AppResult<Vec<T>> {
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        out.push(T::from_row(&row)?);
    }
    Ok(out)
}

pub async fn begin(conn: &Connection) -> AppResult<DbTx> {
    Ok(conn.transaction().await?)
}
