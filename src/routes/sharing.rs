//! Profile share-link create/accept and collaborator management.

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Redirect, Response},
    routing::{get, post},
    Router,
};
use tower_sessions::Session;

use crate::app_state::AppState;
use crate::auth::{
    encode_query_value, get_user_id, hx_redirect, set_pending_share, take_pending_share, AuthUser,
    HOME_PATH,
};
use crate::error::AppResult;
use crate::models::{
    accept_share_link, active_share_link, create_share_link, get_active_profile_id, leave_profile,
    list_collaborators, list_profiles, remove_collaborator, require_profile_access,
    set_active_profile, CollaboratorRow, ProfileRole, ShareLinkStatus,
};
use crate::templates::{HtmlTemplate, SharePanelTemplate};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/share/{token}", get(accept_share))
        .route("/profiles/{id}/share", post(create_share))
        .route("/profiles/{id}/share-panel", get(share_panel))
        .route(
            "/profiles/{id}/collaborators/{user_id}/remove",
            post(remove_collaborator_handler),
        )
        .route("/profiles/{id}/leave", post(leave_profile_handler))
}

async fn accept_share(
    State(state): State<AppState>,
    session: Session,
    Path(token): Path<String>,
) -> AppResult<Response> {
    let Some(user_id) = get_user_id(&session).await? else {
        set_pending_share(&session, &token).await?;
        let next = format!("/share/{token}");
        let location = format!("/register?next={}", encode_query_value(&next));
        return Ok(Redirect::to(&location).into_response());
    };

    let profile_id = accept_share_link(&state.pool, user_id, &token).await?;
    let _ = take_pending_share(&session).await;
    set_active_profile(&state.pool, user_id, Some(&profile_id)).await?;
    Ok(Redirect::to(HOME_PATH).into_response())
}

async fn create_share(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let created = create_share_link(&state.pool, user.id, &id).await?;
    let active = Some(ShareLinkStatus {
        id: String::new(),
        expires_at: created.expires_at.clone(),
    });
    let collaborators = list_collaborators(&state.pool, user.id, &id).await?;
    Ok(HtmlTemplate(share_panel_template(
        &id,
        true,
        active.as_ref(),
        &collaborators,
        Some(created.path),
        Some(created.expires_at),
    )))
}

async fn share_panel(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> AppResult<impl IntoResponse> {
    let role = require_profile_access(&state.pool, user.id, &id).await?;
    if role == ProfileRole::Owner {
        let active = active_share_link(&state.pool, user.id, &id).await?;
        let collaborators = list_collaborators(&state.pool, user.id, &id).await?;
        Ok(HtmlTemplate(share_panel_template(
            &id,
            true,
            active.as_ref(),
            &collaborators,
            None,
            None,
        )))
    } else {
        Ok(HtmlTemplate(share_panel_template(
            &id,
            false,
            None,
            &[],
            None,
            None,
        )))
    }
}

async fn remove_collaborator_handler(
    user: AuthUser,
    State(state): State<AppState>,
    Path((id, user_id)): Path<(String, String)>,
) -> AppResult<impl IntoResponse> {
    remove_collaborator(&state.pool, user.id, &id, &user_id).await?;
    let active = active_share_link(&state.pool, user.id, &id).await?;
    let collaborators = list_collaborators(&state.pool, user.id, &id).await?;
    Ok(HtmlTemplate(share_panel_template(
        &id,
        true,
        active.as_ref(),
        &collaborators,
        None,
        None,
    )))
}

async fn leave_profile_handler(
    user: AuthUser,
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> AppResult<Response> {
    leave_profile(&state.pool, user.id, &id).await?;
    let active = get_active_profile_id(&state.pool, user.id).await?;
    if active.as_deref() == Some(id.as_str()) {
        let next = list_profiles(&state.pool, user.id).await?;
        set_active_profile(&state.pool, user.id, next.first().map(|p| p.id.as_str())).await?;
    }
    Ok(hx_redirect(&headers, HOME_PATH))
}

fn share_panel_template(
    profile_id: &str,
    is_owner: bool,
    active: Option<&ShareLinkStatus>,
    collaborators: &[CollaboratorRow],
    fresh_path: Option<String>,
    fresh_expires_at: Option<String>,
) -> SharePanelTemplate {
    SharePanelTemplate {
        profile_id: profile_id.to_string(),
        is_owner,
        has_active_invite: active.is_some() || fresh_path.is_some(),
        active_expires_at: fresh_expires_at
            .or_else(|| active.map(|a| a.expires_at.clone()))
            .unwrap_or_default(),
        fresh_invite_url: fresh_path.unwrap_or_default(),
        collaborators: collaborators
            .iter()
            .map(|c| crate::templates::CollaboratorView {
                user_id: c.user_id.clone(),
                email: c.email.clone(),
            })
            .collect(),
    }
}
