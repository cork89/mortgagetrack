mod pages;
mod sharing;

use axum::Router;

use crate::app_state::AppState;
use crate::auth;

pub fn router() -> Router<AppState> {
    Router::new()
        .merge(auth::routes())
        .merge(pages::routes())
        .merge(sharing::routes())
}
