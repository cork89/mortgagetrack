//! In-house email/password authentication.
//!
//! Module layout follows the auth system design:
//! - [`models`] — users, credentials, password hashing
//! - [`extractor`] — [`AuthUser`] + HTMX-aware rejection
//! - [`handlers`] — register / login / logout routes
//! - [`password_reset`] — forgot / reset password
//! - [`settings`] — change password / delete account
//! - [`middleware`] — session helpers and HTMX redirect utilities

mod settings;
mod extractor;
mod handlers;
mod middleware;
mod models;
mod next;
mod password_reset;
pub mod rate_limit;
mod seed;

use axum::Router;

use crate::app_state::AppState;
use crate::db::DbPool;
use crate::error::AppResult;

pub use extractor::{current_user, AuthUser};
pub use middleware::{
    get_user_id, hx_redirect, is_htmx, purge_session, set_pending_share, set_user_id,
    take_pending_share, HOME_PATH,
};
pub use settings::avatar_src;
pub use next::{encode_query_value, is_share_invite_next, safe_next, share_token_from_next};
pub use seed::ensure_test_user;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(handlers::routes())
        .merge(password_reset::routes())
        .merge(settings::routes())
}

pub async fn ensure_password_reset_schema(pool: &DbPool) -> AppResult<()> {
    password_reset::ensure_schema(pool).await
}
