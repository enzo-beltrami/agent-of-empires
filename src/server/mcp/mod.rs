//! Model Context Protocol server hosted on the daemon's router at `/mcp`.
//!
//! Tool handlers call the same in-process code as the HTTP handlers. Auth, host
//! and origin checks come from the router's middleware, not from `rmcp`.

use std::sync::Arc;

use rmcp::{
    handler::server::{
        router::tool::ToolRouter,
        wrapper::{Json, Parameters},
    },
    model::{CallToolResult, Implementation, ServerCapabilities, ServerConfig},
    transport::streamable_http_server::{
        session::never::NeverSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    },
    ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::api::sessions::{list_sessions_envelope, SessionFilter, SessionsView};
use super::state::AppState;
use crate::daemon::SessionResponse;
use crate::session::SessionScope;

mod control;
mod coordination;
mod lifecycle;

#[derive(Debug, Default, Clone, Copy, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum StateFilter {
    /// Sessions that are neither archived nor trashed.
    #[default]
    Live,
    /// Every session, including archived and trashed ones.
    All,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct ListSessionsParams {
    /// Which sessions to return. Defaults to `live`.
    #[serde(default)]
    state: StateFilter,
    /// Only sessions in this group path or beneath it.
    group: Option<String>,
    /// Only direct children of the session with this id.
    children_of: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
struct WatchParams {
    session_id: Option<String>,
    /// False unwatches `session_id`. Defaults to true.
    #[serde(default = "watch_default")]
    watch: bool,
    /// Watch every session (true) or only the listed ones (false).
    all: Option<bool>,
}

fn watch_default() -> bool {
    true
}

#[derive(Debug, Serialize, JsonSchema)]
struct ProjectEntry {
    name: String,
    path: String,
}

#[derive(Debug, Serialize, JsonSchema)]
struct ProjectsView {
    /// Projects registered with `aoe project add` or the dashboard.
    registered: Vec<ProjectEntry>,
    /// Directories sessions were recently created in, newest first.
    recent: Vec<ProjectEntry>,
}

#[derive(Clone)]
struct McpServer {
    state: Arc<AppState>,
    tool_router: ToolRouter<Self>,
}

#[rmcp::tool_router]
impl McpServer {
    fn new(state: Arc<AppState>) -> Self {
        Self {
            state,
            tool_router: Self::tool_router()
                + Self::control_router()
                + Self::lifecycle_router()
                + Self::coordination_router(),
        }
    }

    #[rmcp::tool(description = "List sessions, in the same shape as `GET /api/sessions`.")]
    async fn list_sessions(
        &self,
        Parameters(params): Parameters<ListSessionsParams>,
    ) -> Result<Json<SessionsView>, rmcp::ErrorData> {
        let filter = SessionFilter {
            scope: Some(match params.state {
                StateFilter::Live => SessionScope::Live,
                StateFilter::All => SessionScope::All,
            }),
            group: params.group,
            children_of: params.children_of,
        };
        list_sessions_envelope(&self.state, filter)
            .await
            .map(Json)
            .map_err(|_| rmcp::ErrorData::internal_error("Session snapshot unavailable", None))
    }

    #[rmcp::tool(
        description = "Projects a session can be created in: the served profile's registered \
                       projects, then recently used directories. Match a project the user names \
                       against `name` and the last path component."
    )]
    async fn list_projects(&self) -> Result<Json<ProjectsView>, rmcp::ErrorData> {
        let snapshot = {
            let _namespace = self.state.profile_namespace.read().await;
            self.state.runtime.publish(&self.state).await.map_err(|_| {
                rmcp::ErrorData::internal_error("Project snapshot unavailable", None)
            })?
        };
        let contents = &snapshot.value.contents;
        let served = self.state.served_profile().clone();
        let profile_projects = contents
            .profiles
            .iter()
            .find(|profile| profile.name == *served)
            .map(|profile| profile.projects.clone())
            .unwrap_or_default();
        let registered: Vec<ProjectEntry> = crate::session::projects::merge_project_scopes(
            contents.global_projects.clone(),
            profile_projects,
            |project| project.path.clone(),
        )
        .into_iter()
        .map(|project| ProjectEntry {
            name: project.name,
            path: project.path,
        })
        .collect();
        let hidden = crate::session::coordinator::coordinator_dir().ok();
        let recent = tokio::task::spawn_blocking(move || {
            crate::session::load_recent_projects()
                .unwrap_or_default()
                .into_iter()
                .filter(|p| std::path::Path::new(&p.path).is_dir())
                .filter(|p| hidden.as_deref() != Some(std::path::Path::new(&p.path)))
                .map(|p| ProjectEntry {
                    name: p.display_name,
                    path: p.path,
                })
                .collect()
        })
        .await
        .unwrap_or_default();
        Ok(Json(ProjectsView { registered, recent }))
    }

    #[rmcp::tool(
        description = "Choose which sessions report their status changes to the coordinator: \
                       it is told when one starts waiting for input, finishes a turn or fails. \
                       Give `session_id` to watch or unwatch one session (`watch: false`), or \
                       `all` to watch every session. Returns the watchlist; with no arguments \
                       it only reads it."
    )]
    async fn watch_session(
        &self,
        Parameters(params): Parameters<WatchParams>,
    ) -> Result<Json<crate::session::coordinator::Watchlist>, CallToolResult> {
        if (params.session_id.is_some() || params.all.is_some()) && self.state.read_only {
            return Err(read_only_refusal(&self.state).unwrap());
        }
        if let Some(id) = &params.session_id {
            if !self
                .state
                .instances
                .read()
                .await
                .iter()
                .any(|i| &i.id == id)
            {
                return Err(tool_error("not_found", format!("No session with id {id}")));
            }
        }
        let watch = params.watch;
        tokio::task::spawn_blocking(move || {
            crate::session::coordinator::update_watchlist(|list| {
                if let Some(all) = params.all {
                    list.all = all;
                }
                if let Some(id) = params.session_id {
                    if watch {
                        list.sessions.insert(id);
                    } else {
                        list.sessions.remove(&id);
                    }
                }
            })
        })
        .await
        .map_err(|e| tool_error("internal", e.to_string()))?
        .map(Json)
        .map_err(|e| tool_error("internal", format!("{e:#}")))
    }

    #[rmcp::tool(
        description = "The calling session, or null when the caller is not a session AoE launched."
    )]
    async fn whoami(&self) -> Json<Option<SessionResponse>> {
        Json(None)
    }
}

#[rmcp::tool_handler(router = self.tool_router)]
impl ServerHandler for McpServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("aoe", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Read, create and drive Agent of Empires sessions. If you are a session working \
                 on a task and need a decision you should not make alone, or are blocked, call \
                 `message_coordinator` with `needs_decision: true` and end your turn; the answer \
                 arrives as your next message. Report milestones such as an opened pull request \
                 the same way, without `needs_decision`.",
            )
    }
}

/// A tool error the model can read and act on: `isError` with a stable `code`.
fn tool_error(code: &str, message: impl Into<String>) -> CallToolResult {
    CallToolResult::structured_error(serde_json::json!({
        "code": code,
        "message": message.into(),
    }))
}

/// `/mcp` has no read-only layer, so each mutating tool refuses here, in the HTTP wording.
fn read_only_refusal(state: &AppState) -> Option<CallToolResult> {
    state
        .read_only
        .then(|| tool_error("read_only", super::api::READ_ONLY_MESSAGE))
}

pub(super) fn service(
    state: Arc<AppState>,
) -> StreamableHttpService<impl ServerHandler, NeverSessionManager> {
    StreamableHttpService::new(
        move || Ok(McpServer::new(state.clone())),
        Arc::default(),
        // `rmcp` reads the body itself, so the router's `DefaultBodyLimit` does not apply.
        StreamableHttpServerConfig::default()
            .with_legacy_session_mode(false)
            .with_json_response(true)
            .disable_allowed_hosts()
            .with_max_request_body_bytes(1024 * 1024),
    )
}

#[cfg(test)]
mod tests;
