//! OAuth consent UI for Better Auth MCP / OIDC clients.

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use serde::Deserialize;
use tower_sessions::Session;

use crate::app_state::AppState;
use crate::auth::AuthUser;
use crate::csrf;
use crate::error::AppResult;
use crate::templates::{HtmlTemplate, OAuthConsentTemplate};

pub fn routes() -> Router<AppState> {
    Router::new().route("/oauth/consent", get(consent_page))
}

#[derive(Debug, Deserialize)]
pub struct ConsentQuery {
    pub client_id: Option<String>,
    pub scope: Option<String>,
}

async fn consent_page(
    State(state): State<AppState>,
    session: Session,
    user: AuthUser,
    Query(q): Query<ConsentQuery>,
) -> AppResult<Response> {
    let csrf_token = csrf::ensure_token(&session).await?;
    let client_id = q.client_id.unwrap_or_else(|| "MCP client".into());
    let scopes = q.scope.unwrap_or_default();

    Ok(HtmlTemplate(OAuthConsentTemplate {
        csrf_token,
        app_name: state.app_name.clone(),
        email: user.email,
        client_name: client_id,
        scopes,
    })
    .into_response())
}
