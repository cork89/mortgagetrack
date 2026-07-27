//! In-house email/password authentication.
//!
//! Module layout follows the auth system design:
//! - [`models`] — users, credentials, password hashing
//! - [`extractor`] — [`AuthUser`] + HTMX-aware rejection
//! - [`handlers`] — register / login / logout routes
//! - [`middleware`] — session helpers and HTMX redirect utilities

mod extractor;
mod handlers;
mod middleware;
mod models;
mod next;
mod seed;

pub use extractor::AuthUser;
pub use handlers::routes;
pub use middleware::{get_user_id, hx_redirect, HOME_PATH};
pub use seed::ensure_test_user;
