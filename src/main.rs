mod app_state;
mod auth;
mod error;
mod models;
mod routes;
mod templates;

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::Router;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use time::Duration;
use tower_http::services::ServeDir;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_sqlx_store::SqliteStore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use app_state::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "homestead=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let database_url =
        std::env::var("DATABASE_URL").unwrap_or_else(|_| "sqlite:homestead.db".into());
    let pool = connect_db(&database_url).await?;
    run_migrations(&pool).await?;
    models::ensure_profiles_belong_to_users(&pool).await?;
    auth::ensure_test_user(&pool).await?;

    let session_store = SqliteStore::new(pool.clone());
    session_store.migrate().await?;

    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_http_only(true)
        .with_same_site(tower_sessions::cookie::SameSite::Lax)
        .with_expiry(Expiry::OnInactivity(Duration::days(14)));

    let state = AppState { pool };

    let static_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static");
    let app = Router::new()
        .merge(routes::router())
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(session_layer)
        .with_state(state);

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr: SocketAddr = format!("{host}:{port}").parse()?;

    tracing::info!("Homestead listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn connect_db(url: &str) -> Result<SqlitePool, sqlx::Error> {
    let options = url
        .parse::<SqliteConnectOptions>()?
        .create_if_missing(true)
        .foreign_keys(true);

    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
}

async fn run_migrations(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    for sql in [
        include_str!("../migrations/001_init.sql"),
        include_str!("../migrations/002_auth.sql"),
        include_str!("../migrations/003_profiles_user.sql"),
    ] {
        for stmt in sql.split(';') {
            let stmt = stmt.trim();
            if stmt.is_empty() {
                continue;
            }
            sqlx::query(stmt).execute(pool).await?;
        }
    }
    Ok(())
}
