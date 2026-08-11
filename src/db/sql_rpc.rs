//! HTTP SQL RPC client (talks to a gateway that executes SQL remotely).

use std::sync::Arc;

use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::value::{IntoSqlParams, Row, SqlValue};
use crate::error::{AppError, AppResult};

#[derive(Clone)]
pub struct SqlRpcClient {
    inner: Arc<SqlRpcInner>,
}

struct SqlRpcInner {
    http: Client,
    rpc_url: String,
    secret: String,
}

#[derive(Serialize)]
struct RpcRequest<'a> {
    op: &'a str,
    sql: &'a str,
    params: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct RpcResponse {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    changes: Option<u64>,
    #[serde(default)]
    rows: Option<Vec<Vec<serde_json::Value>>>,
}

impl SqlRpcClient {
    pub fn new(rpc_url: String, secret: String) -> Self {
        Self {
            inner: Arc::new(SqlRpcInner {
                http: Client::new(),
                rpc_url: rpc_url.trim_end_matches('/').to_string(),
                secret,
            }),
        }
    }

    async fn rpc(&self, op: &str, sql: &str, params: Vec<SqlValue>) -> AppResult<RpcResponse> {
        let body = RpcRequest {
            op,
            sql,
            params: params.into_iter().map(|p| p.to_json()).collect(),
        };
        let response = self
            .inner
            .http
            .post(format!("{}/_internal/db", self.inner.rpc_url))
            .header("Authorization", format!("Bearer {}", self.inner.secret))
            .json(&body)
            .send()
            .await
            .map_err(|err| AppError::Database(format!("SQL RPC request failed: {err}")))?;

        let status = response.status();
        let parsed: RpcResponse = response.json().await.map_err(|err| {
            AppError::Database(format!("SQL RPC invalid JSON (HTTP {status}): {err}"))
        })?;

        if !parsed.ok {
            return Err(AppError::Database(
                parsed
                    .error
                    .unwrap_or_else(|| format!("SQL RPC error (HTTP {status})")),
            ));
        }
        Ok(parsed)
    }

    pub async fn execute(&self, sql: &str, params: impl IntoSqlParams) -> AppResult<u64> {
        let parsed = self.rpc("execute", sql, params.into_sql_params()).await?;
        Ok(parsed.changes.unwrap_or(0))
    }

    pub async fn query_rows(&self, sql: &str, params: impl IntoSqlParams) -> AppResult<Vec<Row>> {
        let parsed = self.rpc("query", sql, params.into_sql_params()).await?;
        let rows = parsed.rows.unwrap_or_default();
        Ok(rows
            .into_iter()
            .map(|cols| Row::new(cols.iter().map(SqlValue::from_json).collect()))
            .collect())
    }

    pub async fn execute_batch(&self, sql: &str) -> AppResult<()> {
        // Gateway splits on ';' for exec-style batches.
        let _ = self.rpc("batch_sql", sql, Vec::new()).await?;
        Ok(())
    }
}
