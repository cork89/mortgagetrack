mod app_state;
mod auth;
mod config;
mod csrf;
mod db;
mod error;
mod models;
mod redis;
mod routes;
mod templates;

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::middleware;
use axum::Router;
use chrono::NaiveDate;
use time::Duration;
use tower_http::services::ServeDir;
use tower_sessions::{Expiry, SessionManagerLayer};
use tower_sessions_redis_store::RedisStore;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use app_state::AppState;
use config::SessionConfig;
use db::{execute, get_conn, DbPool};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "mortgage=debug,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let pool = db::connect_db().await?;
    run_migrations(&pool).await?;
    models::ensure_profiles_belong_to_users(&pool).await?;
    models::ensure_profile_version(&pool).await?;
    models::ensure_extra_recast(&pool).await?;
    models::ensure_improvement_detail(&pool).await?;
    models::ensure_user_avatar(&pool).await?;
    models::ensure_user_default_tab(&pool).await?;
    models::ensure_user_payments_year_expand(&pool).await?;
    auth::ensure_test_user(&pool).await?;

    let redis_url = std::env::var("REDIS_URL")
        .map_err(|_| {
            "REDIS_URL is required (Upstash TCP URL, e.g. rediss://default:...@....upstash.io:6379)"
        })?
        .trim()
        .trim_matches('"')
        .to_string();
    if redis_url.is_empty() {
        return Err(
            "REDIS_URL is empty (check .env and that your shell is not exporting an empty REDIS_URL)"
                .into(),
        );
    }
    let redis = redis::connect(&redis_url)
        .await
        .map_err(|err| format!("failed to connect to Redis: {err}"))?;
    tracing::info!("connected to Redis");

    let session_store = RedisStore::new(redis.clone());
    let session_cfg = SessionConfig::from_env();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(session_cfg.secure)
        .with_http_only(session_cfg.http_only)
        .with_same_site(session_cfg.same_site)
        .with_expiry(Expiry::OnInactivity(Duration::minutes(60)));

    let today_override = parse_current_date_override()?;
    let state = AppState {
        pool,
        redis,
        today_override,
    };

    let static_dir = std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static"));
    let app = Router::new()
        .merge(routes::router())
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(middleware::from_fn(csrf::protect))
        .layer(session_layer)
        .with_state(state);

    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(3000);
    let addr: SocketAddr = format!("{host}:{port}").parse()?;

    if !session_cfg.secure && host != "127.0.0.1" && host != "localhost" && host != "::1" {
        tracing::warn!(
            %host,
            "SESSION_SECURE is false while binding on a non-loopback host; \
             set SESSION_SECURE=true behind HTTPS"
        );
    }
    if let Some(today) = today_override {
        tracing::warn!("CURRENT_DATE override active: {today}");
    }
    tracing::info!(
        secure = session_cfg.secure,
        "Homeabell listening on http://{addr}"
    );
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await?;
    Ok(())
}

fn parse_current_date_override() -> Result<Option<NaiveDate>, Box<dyn std::error::Error>> {
    let Some(raw) = std::env::var("CURRENT_DATE").ok() else {
        return Ok(None);
    };
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    let date = NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|e| format!("CURRENT_DATE must be YYYY-MM-DD, got {raw:?}: {e}"))?;
    Ok(Some(date))
}

async fn run_migrations(pool: &DbPool) -> Result<(), Box<dyn std::error::Error>> {
    let conn = get_conn(pool).await?;
    for sql in [
        include_str!("../migrations/001_init.sql"),
        include_str!("../migrations/002_auth.sql"),
        include_str!("../migrations/003_profiles_user.sql"),
        include_str!("../migrations/004_payment_notes.sql"),
        include_str!("../migrations/005_profile_sharing.sql"),
        include_str!("../migrations/006_home_improvements.sql"),
    ] {
        for stmt in sql.split(';') {
            let stmt = stmt.trim();
            if stmt.is_empty() {
                continue;
            }
            execute(&conn, stmt, ()).await?;
        }
    }
    Ok(())
}
