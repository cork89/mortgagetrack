//! Local libSQL / SQLite backend.

use std::ops::Deref;
use std::sync::Arc;

use libsql::{Builder, Connection, Database};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::value::{IntoSqlParams, Row, SqlValue};
use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct LocalPool {
    db: Arc<Database>,
    limit: Arc<Semaphore>,
}

pub struct LocalConn {
    conn: Connection,
    _permit: OwnedSemaphorePermit,
}

impl Deref for LocalConn {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.conn
    }
}

pub struct LocalTx {
    tx: libsql::Transaction,
}

impl Deref for LocalTx {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.tx
    }
}

impl LocalTx {
    pub async fn commit(self) -> AppResult<()> {
        self.tx.commit().await?;
        Ok(())
    }
}

pub async fn connect(path: &str) -> Result<LocalPool, Box<dyn std::error::Error>> {
    let database = Builder::new_local(path).build().await?;
    let pool = LocalPool {
        db: Arc::new(database),
        limit: Arc::new(Semaphore::new(5)),
    };
    let conn = get_conn(&pool).await?;
    conn.execute_batch("PRAGMA foreign_keys = ON;").await?;
    Ok(pool)
}

pub async fn get_conn(pool: &LocalPool) -> AppResult<LocalConn> {
    let permit = pool
        .limit
        .clone()
        .acquire_owned()
        .await
        .map_err(|_| AppError::Database("database pool closed".into()))?;
    let conn = pool.db.connect().map_err(AppError::from)?;
    conn.execute_batch("PRAGMA foreign_keys = ON;").await?;
    Ok(LocalConn {
        conn,
        _permit: permit,
    })
}

pub async fn begin(conn: &LocalConn) -> AppResult<LocalTx> {
    Ok(LocalTx {
        tx: conn.transaction().await?,
    })
}

fn to_libsql_params(params: Vec<SqlValue>) -> libsql::params::Params {
    use libsql::Value;
    let values: Vec<Value> = params
        .into_iter()
        .map(|v| match v {
            SqlValue::Null => Value::Null,
            SqlValue::Integer(i) => Value::Integer(i),
            SqlValue::Real(f) => Value::Real(f),
            SqlValue::Text(s) => Value::Text(s),
            SqlValue::Blob(b) => Value::Blob(b),
        })
        .collect();
    libsql::params::Params::Positional(values)
}

fn from_libsql_value(value: libsql::Value) -> SqlValue {
    match value {
        libsql::Value::Null => SqlValue::Null,
        libsql::Value::Integer(i) => SqlValue::Integer(i),
        libsql::Value::Real(f) => SqlValue::Real(f),
        libsql::Value::Text(s) => SqlValue::Text(s),
        libsql::Value::Blob(b) => SqlValue::Blob(b),
    }
}

pub async fn execute(conn: &Connection, sql: &str, params: impl IntoSqlParams) -> AppResult<u64> {
    let params = to_libsql_params(params.into_sql_params());
    Ok(conn.execute(sql, params).await?)
}

pub async fn query_rows(
    conn: &Connection,
    sql: &str,
    params: impl IntoSqlParams,
) -> AppResult<Vec<Row>> {
    let params = to_libsql_params(params.into_sql_params());
    let mut rows = conn.query(sql, params).await?;
    let mut out = Vec::new();
    while let Some(row) = rows.next().await? {
        let col_count = row.column_count();
        let mut values = Vec::with_capacity(col_count as usize);
        for idx in 0..col_count {
            let v: libsql::Value = row.get(idx)?;
            values.push(from_libsql_value(v));
        }
        out.push(Row::new(values));
    }
    Ok(out)
}

pub async fn execute_batch(conn: &Connection, sql: &str) -> AppResult<()> {
    conn.execute_batch(sql).await?;
    Ok(())
}
