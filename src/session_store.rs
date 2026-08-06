//! SQLite / SQL-RPC-backed tower-sessions store.

use async_trait::async_trait;
use time::OffsetDateTime;
use tower_sessions::session::{Id, Record};
use tower_sessions::session_store::{self, SessionStore};
use tracing::Instrument;

use crate::db::{execute, get_conn, params, query_optional, DbPool, FromRow};
use crate::error::AppResult;

#[derive(Debug, Clone)]
pub struct DbSessionStore {
    pool: DbPool,
}

impl DbSessionStore {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn migrate(&self) -> AppResult<()> {
        let conn = get_conn(&self.pool).await?;
        execute(
            &conn,
            r#"
            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY NOT NULL,
                data TEXT NOT NULL,
                expiry_date TEXT NOT NULL
            )
            "#,
            (),
        )
        .await?;
        execute(
            &conn,
            "CREATE INDEX IF NOT EXISTS sessions_expiry_date_idx ON sessions (expiry_date)",
            (),
        )
        .await?;
        Ok(())
    }

    pub async fn delete_expired(&self) -> AppResult<()> {
        let conn = get_conn(&self.pool).await?;
        let now = OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|err| crate::error::AppError::Internal(err.to_string()))?;
        execute(
            &conn,
            "DELETE FROM sessions WHERE expiry_date < ?",
            params![now],
        )
        .await?;
        Ok(())
    }
}

struct SessionRow {
    data: String,
    expiry_date: String,
}

impl FromRow for SessionRow {
    fn from_row(row: &crate::db::Row) -> AppResult<Self> {
        Ok(Self {
            data: row.get(0)?,
            expiry_date: row.get(1)?,
        })
    }
}

#[async_trait]
impl SessionStore for DbSessionStore {
    async fn create(&self, record: &mut Record) -> session_store::Result<()> {
        // Retry on rare id collision.
        for _ in 0..8 {
            match self.save_new(record).await {
                Ok(()) => return Ok(()),
                Err(session_store::Error::Backend(msg)) if msg.contains("UNIQUE") => {
                    record.id = Id::default();
                }
                Err(err) => return Err(err),
            }
        }
        Err(session_store::Error::Backend(
            "failed to allocate unique session id".into(),
        ))
    }

    async fn save(&self, record: &Record) -> session_store::Result<()> {
        let id = record.id.to_string();
        let data = serde_json::to_string(&record.data)
            .map_err(|err| session_store::Error::Encode(err.to_string()))?;
        let expiry = record
            .expiry_date
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;

        let conn = get_conn(&self.pool)
            .await
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        execute(
            &conn,
            r#"
            INSERT INTO sessions (id, data, expiry_date) VALUES (?, ?, ?)
            ON CONFLICT(id) DO UPDATE SET
                data = excluded.data,
                expiry_date = excluded.expiry_date
            "#,
            params![id, data, expiry],
        )
        .await
        .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        Ok(())
    }

    async fn load(&self, session_id: &Id) -> session_store::Result<Option<Record>> {
        let conn = get_conn(&self.pool)
            .await
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        let row: Option<SessionRow> = query_optional(
            &conn,
            "SELECT data, expiry_date FROM sessions WHERE id = ?",
            params![session_id.to_string()],
        )
        .await
        .map_err(|err| session_store::Error::Backend(err.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let expiry = OffsetDateTime::parse(
            &row.expiry_date,
            &time::format_description::well_known::Rfc3339,
        )
        .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        if expiry <= OffsetDateTime::now_utc() {
            self.delete(session_id).await?;
            return Ok(None);
        }

        let data = serde_json::from_str(&row.data)
            .map_err(|err| session_store::Error::Decode(err.to_string()))?;
        Ok(Some(Record {
            id: *session_id,
            data,
            expiry_date: expiry,
        }))
    }

    async fn delete(&self, session_id: &Id) -> session_store::Result<()> {
        let conn = get_conn(&self.pool)
            .await
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        execute(
            &conn,
            "DELETE FROM sessions WHERE id = ?",
            params![session_id.to_string()],
        )
        .await
        .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        Ok(())
    }
}

impl DbSessionStore {
    async fn save_new(&self, record: &Record) -> session_store::Result<()> {
        let id = record.id.to_string();
        let data = serde_json::to_string(&record.data)
            .map_err(|err| session_store::Error::Encode(err.to_string()))?;
        let expiry = record
            .expiry_date
            .format(&time::format_description::well_known::Rfc3339)
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;

        let conn = get_conn(&self.pool)
            .await
            .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        execute(
            &conn,
            "INSERT INTO sessions (id, data, expiry_date) VALUES (?, ?, ?)",
            params![id, data, expiry],
        )
        .await
        .map_err(|err| session_store::Error::Backend(err.to_string()))?;
        Ok(())
    }

    pub fn spawn_cleanup_task(self) {
        tokio::spawn(
            async move {
                let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
                loop {
                    interval.tick().await;
                    if let Err(err) = self.delete_expired().await {
                        tracing::warn!(error = %err, "session cleanup failed");
                    }
                }
            }
            .instrument(tracing::info_span!("session_cleanup")),
        );
    }
}
