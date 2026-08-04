//! Redis connection for sessions and auth rate limiting.

use tower_sessions_redis_store::fred::prelude::*;

pub type RedisPool = Pool;

/// Connect a small fred pool to `REDIS_URL` (supports `rediss://` TLS).
pub async fn connect(redis_url: &str) -> Result<RedisPool, Box<dyn std::error::Error>> {
    let config = Config::from_url(redis_url)?;
    // Upstash free tier is connection-limited; keep the pool small.
    let pool = Builder::from_config(config).build_pool(4)?;
    let _handles = pool.connect();
    pool.wait_for_connect().await?;
    Ok(pool)
}
