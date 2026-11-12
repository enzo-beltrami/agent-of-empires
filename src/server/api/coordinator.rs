//! `POST /api/coordinator`: the dashboard coordinator's session, created on first use.

use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use super::api_error;
use crate::server::AppState;

#[derive(Serialize)]
pub struct CoordinatorResponse {
    pub session_id: String,
}

/// Serializes ensures, so two dashboards opening the pane at once create one session.
static ENSURE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

pub async fn ensure_coordinator(State(state): State<Arc<AppState>>) -> Response {
    if state.read_only {
        return super::read_only_response();
    }
    // The worker reaches the daemon's tools through `/mcp` on the owner socket.
    if !state.web_config.mcp_enabled {
        return api_error(
            StatusCode::CONFLICT,
            "mcp_disabled",
            "The coordinator needs the MCP server. Turn on Settings > MCP server, then restart aoe serve.",
        );
    }
    let _ensure = ENSURE.lock().await;
    let dir = match tokio::task::spawn_blocking(crate::session::coordinator::provision).await {
        Ok(Ok(dir)) => dir,
        Ok(Err(error)) => {
            return api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "coordinator_dir",
                format!("Could not prepare the coordinator directory: {error:#}"),
            )
        }
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let path = dir.to_string_lossy().into_owned();
    if let Some(session_id) = existing(&state, &path).await {
        return Json(CoordinatorResponse { session_id }).into_response();
    }
    let body = match serde_json::from_value(serde_json::json!({
        "path": path,
        "tool": "claude",
        "title": "Coordinator",
        "title_typed": true,
        "view": "structured",
    })) {
        Ok(body) => body,
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let reference = format!("coordinator-{}", uuid::Uuid::new_v4());
    let response =
        super::sessions::create_session_with(state.clone(), true, &reference, false, body, None)
            .await;
    if !response.status().is_success() {
        return response;
    }
    match existing(&state, &path).await {
        Some(session_id) => Json(CoordinatorResponse { session_id }).into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// The coordinator session, if one exists.
pub(crate) async fn coordinator_session(state: &AppState) -> Option<String> {
    let dir = crate::session::coordinator::coordinator_dir().ok()?;
    existing(state, &dir.to_string_lossy()).await
}

/// The newest live structured session in the coordinator directory.
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Bucket {
    Wip,
    NeedsAction,
    Idle,
}

pub(crate) fn bucket(status: crate::session::Status) -> Option<Bucket> {
    use crate::session::Status::*;
    match status {
        Running | Starting | Creating => Some(Bucket::Wip),
        Waiting | Error => Some(Bucket::NeedsAction),
        Idle | Stopped | Unknown => Some(Bucket::Idle),
        Deleting => None,
    }
}

#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub(crate) struct SummaryRow {
    pub id: String,
    pub title: String,
    pub project_path: String,
    /// Lowercase status, e.g. `running` or `waiting`.
    pub status: String,
}

/// Live sessions other than the coordinator, by what they need from the user.
#[derive(Debug, Default, Serialize, schemars::JsonSchema)]
pub(crate) struct SessionSummary {
    /// Running, starting or being created.
    pub wip: Vec<SummaryRow>,
    /// Waiting for input, or failed.
    pub needs_action: Vec<SummaryRow>,
    /// Idle or stopped.
    pub idle: Vec<SummaryRow>,
}

pub(crate) async fn session_summary(state: &AppState) -> SessionSummary {
    let coordinator = coordinator_session(state).await;
    let mut summary = SessionSummary::default();
    for inst in state.instances.read().await.iter() {
        if inst.is_trashed() || inst.is_archived() || Some(&inst.id) == coordinator.as_ref() {
            continue;
        }
        let Some(bucket) = bucket(inst.status) else {
            continue;
        };
        let row = SummaryRow {
            id: inst.id.clone(),
            title: inst.title.clone(),
            project_path: inst.project_path.clone(),
            status: inst.status.as_str().to_string(),
        };
        match bucket {
            Bucket::Wip => summary.wip.push(row),
            Bucket::NeedsAction => summary.needs_action.push(row),
            Bucket::Idle => summary.idle.push(row),
        }
    }
    summary
}

pub async fn get_coordinator_summary(State(state): State<Arc<AppState>>) -> Response {
    Json(session_summary(&state).await).into_response()
}

/// A deploy with the title of the session that produced it, while that session exists.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub(crate) struct DeployView {
    #[serde(flatten)]
    pub deploy: crate::session::coordinator::Deploy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_title: Option<String>,
}

pub(crate) async fn deploy_views(state: &AppState) -> Option<Vec<DeployView>> {
    let deploys = tokio::task::spawn_blocking(crate::session::coordinator::deploys)
        .await
        .ok()?;
    let instances = state.instances.read().await;
    Some(
        deploys
            .into_iter()
            .map(|deploy| DeployView {
                session_title: deploy.session_id.as_ref().and_then(|id| {
                    instances
                        .iter()
                        .find(|inst| &inst.id == id)
                        .map(|inst| inst.title.clone())
                }),
                deploy,
            })
            .collect(),
    )
}

pub async fn list_coordinator_deploys(State(state): State<Arc<AppState>>) -> Response {
    match deploy_views(&state).await {
        Some(deploys) => Json(deploys).into_response(),
        None => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn update_coordinator_deploy(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
    body: Result<
        Json<crate::session::coordinator::DeployChange>,
        axum::extract::rejection::JsonRejection,
    >,
) -> Response {
    if state.read_only {
        return super::read_only_response();
    }
    let Json(change) = match body {
        Ok(body) => body,
        Err(rejection) => return rejection.into_response(),
    };
    let result = tokio::task::spawn_blocking(move || {
        crate::session::coordinator::update_deploy(&id, change)
    })
    .await;
    match result {
        Ok(Ok(Some(deploy))) => Json(deploy).into_response(),
        Ok(Ok(None)) => deploy_not_found(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn delete_coordinator_deploy(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Response {
    if state.read_only {
        return super::read_only_response();
    }
    match tokio::task::spawn_blocking(move || crate::session::coordinator::remove_deploy(&id)).await
    {
        Ok(Ok(true)) => StatusCode::NO_CONTENT.into_response(),
        Ok(Ok(false)) => deploy_not_found(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

#[derive(Serialize, serde::Deserialize)]
pub struct NotesBody {
    pub text: String,
}

pub async fn get_coordinator_notes() -> Response {
    match tokio::task::spawn_blocking(crate::session::coordinator::notes).await {
        Ok(Ok(text)) => Json(NotesBody { text }).into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

pub async fn put_coordinator_notes(
    State(state): State<Arc<AppState>>,
    body: Result<Json<NotesBody>, axum::extract::rejection::JsonRejection>,
) -> Response {
    if state.read_only {
        return super::read_only_response();
    }
    let Json(NotesBody { text }) = match body {
        Ok(body) => body,
        Err(rejection) => return rejection.into_response(),
    };
    match tokio::task::spawn_blocking(move || crate::session::coordinator::set_notes(&text)).await {
        Ok(Ok(())) => StatusCode::NO_CONTENT.into_response(),
        _ => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

fn deploy_not_found() -> Response {
    api_error(
        StatusCode::NOT_FOUND,
        "deploy_not_found",
        "No deploy with that id",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::Status::*;

    #[test]
    fn buckets_group_statuses_by_what_they_need() {
        for (status, expected) in [
            (Running, Some(Bucket::Wip)),
            (Starting, Some(Bucket::Wip)),
            (Creating, Some(Bucket::Wip)),
            (Waiting, Some(Bucket::NeedsAction)),
            (Error, Some(Bucket::NeedsAction)),
            (Idle, Some(Bucket::Idle)),
            (Stopped, Some(Bucket::Idle)),
            (Unknown, Some(Bucket::Idle)),
            (Deleting, None),
        ] {
            assert_eq!(bucket(status), expected, "{status:?}");
        }
    }
}
