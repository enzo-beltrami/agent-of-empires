//! `POST /api/assistant`: the dashboard assistant's session, created on first use.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use super::api_error;
use crate::server::AppState;

#[derive(Serialize)]
pub struct AssistantResponse {
    pub session_id: String,
}

/// Serializes ensures, so two dashboards opening the pane at once create one session.
static ENSURE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn ensure_assistant(State(state): State<Arc<AppState>>) -> Response {
    if state.read_only {
        return super::read_only_response();
    }
    // The worker reaches the daemon's tools through `/mcp` on the owner socket.
    if !state.web_config.mcp_enabled {
        return api_error(
            StatusCode::CONFLICT,
            "mcp_disabled",
            "The assistant needs the MCP server. Turn on Settings > MCP server, then restart aoe serve.",
        );
    }
    let _ensure = ENSURE.lock().await;
    let dir = match tokio::task::spawn_blocking(crate::session::assistant::provision).await {
        Ok(Ok(dir)) => dir,
        Ok(Err(error)) => {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "assistant_dir",
                format!("Could not prepare the assistant directory: {error:#}"),
            )
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let path = dir.to_string_lossy().into_owned();
    if let Some(session_id) = existing(&state, &path).await {
        return Json(AssistantResponse { session_id }).into_response();
    }
    let body = match serde_json::from_value(serde_json::json!({
        "path": path,
        "tool": "claude",
        "title": "Assistant",
        "title_typed": true,
        "view": "structured",
    })) {
        Ok(body) => body,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let reference = format!("assistant-{}", uuid::Uuid::new_v4());
    let response =
        super::sessions::create_session_with(state.clone(), true, &reference, false, body, None)
            .await;
    if !response.status().is_success() {
        return response;
    }
    match existing(&state, &path).await {
        Some(session_id) => Json(AssistantResponse { session_id }).into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// The newest live structured session in the assistant directory.
async fn existing(state: &AppState, path: &str) -> Option<String> {
    state
        .instances
        .read()
        .await
        .iter()
        .filter(|inst| {
            inst.project_path == path
                && inst.is_structured()
                && !inst.is_trashed()
                && !inst.is_archived()
        })
        .max_by_key(|inst| inst.created_at)
        .map(|inst| inst.id.clone())
}
