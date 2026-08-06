//! Environment-backed runtime settings.

use tower_sessions::cookie::SameSite;

const DEFAULT_APP_NAME: &str = "MortgageTrack";

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub secure: bool,
    pub same_site: SameSite,
    pub http_only: bool,
}

impl SessionConfig {
    pub fn from_env() -> Self {
        let secure = env_bool("SESSION_SECURE", false);
        let same_site = parse_same_site(&env_string("SESSION_SAME_SITE", "Lax"), secure);
        Self {
            secure,
            same_site,
            http_only: true,
        }
    }
}

/// Product name shown in HTML (titles, brand marks). Defaults to MortgageTrack.
pub fn app_name_from_env() -> String {
    env_string("APP_NAME", DEFAULT_APP_NAME)
}

fn env_string(key: &str, default: &str) -> String {
    std::env::var(key)
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub fn env_bool(key: &str, default: bool) -> bool {
    match std::env::var(key) {
        Ok(raw) => matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

fn parse_same_site(raw: &str, secure: bool) -> SameSite {
    match raw.trim().to_ascii_lowercase().as_str() {
        "strict" => SameSite::Strict,
        "none" => {
            if !secure {
                tracing::warn!(
                    "SESSION_SAME_SITE=None requires Secure cookies; falling back to Lax \
                     (set SESSION_SECURE=true)"
                );
                SameSite::Lax
            } else {
                SameSite::None
            }
        }
        "lax" => SameSite::Lax,
        other => {
            tracing::warn!(%other, "unknown SESSION_SAME_SITE; using Lax");
            SameSite::Lax
        }
    }
}
