mod app_state;
mod auth;
mod config;
mod csrf;
mod db;
mod error;
mod mail;
mod models;
mod routes;
mod session_store;
mod templates;

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::middleware;
use axum::Router;
use chrono::NaiveDate;
use time::Duration;
use tower_http::services::ServeDir;
use tower_sessions::{Expiry, SessionManagerLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use app_state::AppState;
use config::{app_name_from_env, SessionConfig};
use db::{execute_batch, get_conn, DbPool};
use session_store::DbSessionStore;

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
    auth::rate_limit::ensure_schema(&pool).await?;
    auth::ensure_password_reset_schema(&pool).await?;

    let session_store = DbSessionStore::new(pool.clone());
    session_store.migrate().await?;
    session_store.clone().spawn_cleanup_task();

    // Seed after schema is ready, but don't block the listen socket (Argon2 is slow
    // and delays container port-ready checks on hosted runtimes).
    // When AUTH_TRUST_HEADERS is set, Better Auth on the Worker owns credentials/seed.
    if !auth::trust_identity_headers() {
        let pool = pool.clone();
        tokio::spawn(async move {
            if let Err(err) = auth::ensure_test_user(&pool).await {
                tracing::error!(error = %err, "failed to seed test users");
            }
        });
    }

    let session_cfg = SessionConfig::from_env();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(session_cfg.secure)
        .with_http_only(session_cfg.http_only)
        .with_same_site(session_cfg.same_site)
        .with_expiry(Expiry::OnInactivity(Duration::minutes(60)));

    let today_override = parse_current_date_override()?;
    let mailer = mail::Mailer::from_env()?;
    let app_base_url = mail::app_base_url_from_env();
    let app_name = app_name_from_env();
    let state = AppState {
        pool,
        today_override,
        mailer,
        app_base_url,
        app_name: app_name.clone(),
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
        "{app_name} listening on http://{addr}"
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
    // Remote SQL RPC schema is applied by the hosting gateway; skip embedded SQL there.
    if matches!(pool, DbPool::SqlRpc(_)) {
        tracing::info!("skipping embedded SQL migrations (DB_MODE=sql_rpc)");
        return Ok(());
    }

    let conn = get_conn(pool).await?;
    for sql in [
        include_str!("../migrations/001_init.sql"),
        include_str!("../migrations/002_auth.sql"),
        include_str!("../migrations/003_profiles_user.sql"),
        include_str!("../migrations/004_payment_notes.sql"),
        include_str!("../migrations/005_profile_sharing.sql"),
        include_str!("../migrations/006_home_improvements.sql"),
        include_str!("../migrations/007_sessions_rate_limits.sql"),
        include_str!("../migrations/008_password_reset.sql"),
    ] {
        execute_batch(&conn, sql).await?;
    }
    Ok(())
}
