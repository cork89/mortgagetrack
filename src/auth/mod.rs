//! In-house email/password authentication.
//!
//! Module layout follows the auth system design:
//! - [`models`] — users, credentials, password hashing
//! - [`extractor`] — [`AuthUser`] + HTMX-aware rejection
//! - [`handlers`] — register / login / logout routes
//! - [`account`] — change password / delete account
//! - [`identicon`] — GitHub-style account avatars
//! - [`middleware`] — session helpers and HTMX redirect utilities

mod account;
mod extractor;
mod handlers;
mod identicon;
mod middleware;
mod models;
mod next;
mod seed;

use axum::Router;

use crate::app_state::AppState;

pub use extractor::{current_user, AuthUser};
pub use middleware::{
    get_user_id, hx_redirect, is_htmx, purge_session, set_pending_share, set_user_id,
    take_pending_share, HOME_PATH,
};
pub use next::{encode_query_value, is_share_invite_next, safe_next, share_token_from_next};
pub use seed::ensure_test_user;

pub fn routes() -> Router<AppState> {
    Router::new()
        .merge(handlers::routes())
        .merge(account::routes())
}
