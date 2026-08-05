//! libSQL / D1 connection pool and query helpers.

mod d1;
mod local;
mod value;

pub use value::{params, FromRow, IntoSqlParams, Row, SqlValue, ToSql};

use d1::D1Client;
use local::{LocalConn, LocalPool, LocalTx};

use crate::error::{AppError, AppResult};

/// Shared database handle (local SQLite via libSQL, or D1 via Worker RPC).
#[derive(Clone)]
pub enum DbPool {
    Local(LocalPool),
    D1(D1Client),
}

impl std::fmt::Debug for DbPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local(_) => f.write_str("DbPool::Local"),
            Self::D1(_) => f.write_str("DbPool::D1"),
        }
    }
}

/// Checked-out connection (local) or RPC handle (D1).
pub enum DbConn {
    Local(LocalConn),
    D1(D1Client),
}

/// Transaction handle. Local uses a real SQLite transaction; D1 runs statements
/// immediately (auto-commit) because D1 has no interactive transactions over RPC.
pub enum DbTx {
    Local(LocalTx),
    D1(D1Client),
}

impl DbTx {
    pub async fn commit(self) -> AppResult<()> {
        match self {
            Self::Local(tx) => tx.commit().await,
            Self::D1(_) => Ok(()),
        }
    }
}

pub async fn connect_db() -> Result<DbPool, Box<dyn std::error::Error>> {
    let mode = std::env::var("DB_MODE")
        .unwrap_or_else(|_| "local".into())
        .trim()
        .to_ascii_lowercase();

    if mode == "d1" {
        let rpc_url = std::env::var("DB_RPC_URL")
            .map_err(|_| "DB_RPC_URL is required when DB_MODE=d1")?
            .trim()
            .to_string();
        if rpc_url.is_empty() {
            return Err("DB_RPC_URL is empty".into());
        }
        let secret = std::env::var("INTERNAL_DB_SECRET")
            .map_err(|_| "INTERNAL_DB_SECRET is required when DB_MODE=d1")?
            .trim()
            .to_string();
        if secret.is_empty() {
            return Err("INTERNAL_DB_SECRET is empty".into());
        }
        tracing::info!(%rpc_url, "using D1 via Worker RPC");
        return Ok(DbPool::D1(D1Client::new(rpc_url, secret)));
    }

    // Legacy Turso remote URLs are no longer supported — use local SQLite or D1.
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:mortgage.db".into())
        .trim()
        .to_string();
    if url.is_empty() {
        return Err(
            "DATABASE_URL is empty (check .env and that your shell is not exporting an empty DATABASE_URL)"
                .into(),
        );
    }
    if is_remote_url(&url) {
        return Err(
            "Remote Turso/libSQL URLs are no longer supported. \
             Use DATABASE_URL=sqlite:mortgage.db locally, or DB_MODE=d1 with DB_RPC_URL for Cloudflare D1."
                .into(),
        );
    }

    let path = normalize_sqlite_path(&url);
    if let Some(parent) = std::path::Path::new(&path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    tracing::info!(%path, "using local SQLite");
    Ok(DbPool::Local(local::connect(&path).await?))
}

fn is_remote_url(url: &str) -> bool {
    let url = url.trim();
    url.starts_with("libsql://") || url.starts_with("https://") || url.starts_with("http://")
}

fn normalize_sqlite_path(url: &str) -> String {
    let url = url.trim();
    url.strip_prefix("sqlite://")
        .or_else(|| url.strip_prefix("sqlite:"))
        .unwrap_or(url)
        .to_string()
}

pub async fn get_conn(pool: &DbPool) -> AppResult<DbConn> {
    match pool {
        DbPool::Local(pool) => Ok(DbConn::Local(local::get_conn(pool).await?)),
        DbPool::D1(client) => Ok(DbConn::D1(client.clone())),
    }
}

pub async fn begin(conn: &DbConn) -> AppResult<DbTx> {
    match conn {
        DbConn::Local(conn) => Ok(DbTx::Local(local::begin(conn).await?)),
        DbConn::D1(client) => Ok(DbTx::D1(client.clone())),
    }
}

pub async fn execute(
    conn: &impl SqlExec,
    sql: &str,
    params: impl IntoSqlParams,
) -> AppResult<u64> {
    conn.sql_execute(sql, params.into_sql_params()).await
}

pub async fn query_optional<T: FromRow>(
    conn: &impl SqlExec,
    sql: &str,
    params: impl IntoSqlParams,
) -> AppResult<Option<T>> {
    let rows = conn.sql_query(sql, params.into_sql_params()).await?;
    match rows.into_iter().next() {
        Some(row) => Ok(Some(T::from_row(&row)?)),
        None => Ok(None),
    }
}

pub async fn query_one<T: FromRow>(
    conn: &impl SqlExec,
    sql: &str,
    params: impl IntoSqlParams,
) -> AppResult<T> {
    query_optional(conn, sql, params)
        .await?
        .ok_or_else(|| AppError::Internal("expected a database row".into()))
}

pub async fn query_all<T: FromRow>(
    conn: &impl SqlExec,
    sql: &str,
    params: impl IntoSqlParams,
) -> AppResult<Vec<T>> {
    let rows = conn.sql_query(sql, params.into_sql_params()).await?;
    rows.iter().map(T::from_row).collect()
}

pub async fn execute_batch(conn: &impl SqlExec, sql: &str) -> AppResult<()> {
    conn.sql_execute_batch(sql).await
}

/// Types that can run SQL (connection or transaction).
pub trait SqlExec: Send + Sync {
    fn sql_execute(
        &self,
        sql: &str,
        params: Vec<SqlValue>,
    ) -> impl std::future::Future<Output = AppResult<u64>> + Send;

    fn sql_query(
        &self,
        sql: &str,
        params: Vec<SqlValue>,
    ) -> impl std::future::Future<Output = AppResult<Vec<Row>>> + Send;

    fn sql_execute_batch(
        &self,
        sql: &str,
    ) -> impl std::future::Future<Output = AppResult<()>> + Send;
}

impl SqlExec for DbConn {
    async fn sql_execute(&self, sql: &str, params: Vec<SqlValue>) -> AppResult<u64> {
        match self {
            Self::Local(conn) => local::execute(conn, sql, params).await,
            Self::D1(client) => client.execute(sql, params).await,
        }
    }

    async fn sql_query(&self, sql: &str, params: Vec<SqlValue>) -> AppResult<Vec<Row>> {
        match self {
            Self::Local(conn) => local::query_rows(conn, sql, params).await,
            Self::D1(client) => client.query_rows(sql, params).await,
        }
    }

    async fn sql_execute_batch(&self, sql: &str) -> AppResult<()> {
        match self {
            Self::Local(conn) => local::execute_batch(conn, sql).await,
            Self::D1(client) => client.execute_batch(sql).await,
        }
    }
}

impl SqlExec for DbTx {
    async fn sql_execute(&self, sql: &str, params: Vec<SqlValue>) -> AppResult<u64> {
        match self {
            Self::Local(tx) => local::execute(tx, sql, params).await,
            Self::D1(client) => client.execute(sql, params).await,
        }
    }

    async fn sql_query(&self, sql: &str, params: Vec<SqlValue>) -> AppResult<Vec<Row>> {
        match self {
            Self::Local(tx) => local::query_rows(tx, sql, params).await,
            Self::D1(client) => client.query_rows(sql, params).await,
        }
    }

    async fn sql_execute_batch(&self, sql: &str) -> AppResult<()> {
        match self {
            Self::Local(tx) => local::execute_batch(tx, sql).await,
            Self::D1(client) => client.execute_batch(sql).await,
        }
    }
}
