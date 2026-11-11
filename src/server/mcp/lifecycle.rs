//! Tools that tidy sessions up: stop, archive and trash. Each calls its HTTP handler, so the
//! locking, lifecycle checks and refusals match the dashboard. None of them deletes anything
//! for good: an archived session unarchives and a trashed one restores.

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use super::{control::refusal, read_only_refusal, tool_error, McpServer};
use crate::daemon::{TrashSessionBody, UpdateArchiveBody};
use crate::server::api;

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct SessionParams {
    session_id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct ArchiveParams {
    session_id: String,
    /// False unarchives. Defaults to true.
    #[serde(default = "archive_default")]
    archived: bool,
}

fn archive_default() -> bool {
    true
}

#[rmcp::tool_router(router = lifecycle_router, vis = "pub(super)")]
impl McpServer {
    #[rmcp::tool(
        description = "Stop a session's agent, keeping the session, its transcript and worktree. \
            It can be started again from the dashboard."
    )]
    async fn stop_session(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<Json<Value>, CallToolResult> {
        let state = self.mutable().map_err(|e| *e)?;
        outcome(api::stop_session(State(state), Path(params.session_id)).await)
            .await
            .map_err(|e| *e)
    }

    #[rmcp::tool(
        description = "Archive a session (or unarchive it with `archived: false`): it sinks out \
            of the active list and its agent stops. Worktree, branch and transcript are kept. \
            Prefer this for sessions that are done or unused."
    )]
    async fn archive_session(
        &self,
        Parameters(params): Parameters<ArchiveParams>,
    ) -> Result<Json<Value>, CallToolResult> {
        let state = self.mutable().map_err(|e| *e)?;
        let body = UpdateArchiveBody {
            archived: params.archived,
            kill_pane: true,
        };
        outcome(
            api::update_session_archive(
                State(state),
                Path(params.session_id),
                Ok(axum::Json(body)),
            )
            .await,
        )
        .await
        .map_err(|e| *e)
    }

    #[rmcp::tool(
        description = "Move a session to the trash. It stops and leaves every list; the user can \
            restore it, with its transcript, until the trash is emptied. Only trash a session \
            the user asked to remove."
    )]
    async fn trash_session(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<Json<Value>, CallToolResult> {
        let state = self.mutable().map_err(|e| *e)?;
        let body = TrashSessionBody {
            kill_pane: true,
            expected_generation: None,
        };
        outcome(
            api::trash_session(
                State(state),
                Path(params.session_id),
                Some(axum::Json(body)),
            )
            .await,
        )
        .await
        .map_err(|e| *e)
    }

    #[rmcp::tool(description = "Restore a trashed session to where it was.")]
    async fn restore_session(
        &self,
        Parameters(params): Parameters<SessionParams>,
    ) -> Result<Json<Value>, CallToolResult> {
        let state = self.mutable().map_err(|e| *e)?;
        outcome(api::restore_session(State(state), Path(params.session_id)).await)
            .await
            .map_err(|e| *e)
    }
}

impl McpServer {
    fn mutable(&self) -> Result<std::sync::Arc<crate::server::AppState>, Box<CallToolResult>> {
        match read_only_refusal(&self.state) {
            Some(refusal) => Err(Box::new(refusal)),
            None => Ok(self.state.clone()),
        }
    }
}

/// The handler's JSON on success, or its refusal as a tool error.
async fn outcome(response: impl IntoResponse) -> Result<Json<Value>, Box<CallToolResult>> {
    let response = response.into_response();
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .map_err(|e| Box::new(tool_error("internal", e.to_string())))?;
    if !status.is_success() {
        return Err(Box::new(refusal(status, &bytes)));
    }
    Ok(Json(serde_json::from_slice(&bytes).unwrap_or(Value::Null)))
}
