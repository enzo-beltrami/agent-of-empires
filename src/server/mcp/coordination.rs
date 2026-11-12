//! Tools behind the coordinator's dashboard panel (session summary, deploy list) and the
//! channel a session uses to reach the coordinator.

use rmcp::handler::server::common::Extension;
use rmcp::handler::server::wrapper::{Json, Parameters};
use rmcp::model::CallToolResult;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{read_only_refusal, tool_error, McpServer};
use crate::server::api::{deploy_views, session_summary, DeployView, SessionSummary};
use crate::session::coordinator::{self, Deploy, NewDeploy};

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct AddDeployParams {
    /// The pull request's URL.
    url: String,
    title: Option<String>,
    /// The session that produced the pull request.
    session_id: Option<String>,
    /// The ticket it resolves: a key such as `PI-1921`, or the ticket's URL.
    ticket: Option<String>,
    /// What it changes, in a sentence, for whoever deploys it.
    note: Option<String>,
    /// Whether its change has been tested. New entries default to untested.
    tested: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct SetDeployedParams {
    id: String,
    /// False moves it back to pending. Defaults to true unless only `tested` is given.
    deployed: Option<bool>,
    /// Marks it tested or untested.
    tested: Option<bool>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct DeployIdParams {
    id: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct WriteNotesParams {
    /// Markdown. Replaces all the notes unless `append` is set.
    text: String,
    /// Add `text` on new lines at the end instead of replacing the notes.
    #[serde(default)]
    append: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Notes {
    text: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub(super) struct MessageParams {
    /// What you need: the question with its context and the options you see, or the update.
    message: String,
    /// True when you are blocked until someone decides.
    #[serde(default)]
    needs_decision: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct DeployList {
    deploys: Vec<DeployView>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub(super) struct Delivered {
    coordinator_session_id: String,
}

#[rmcp::tool_router(router = coordination_router, vis = "pub(super)")]
impl McpServer {
    #[rmcp::tool(
        description = "Live sessions other than the coordinator, grouped as the dashboard shows \
            them: `wip` (running), `needs_action` (waiting for input or failed) and `idle` \
            (idle or stopped)."
    )]
    async fn session_summary(&self) -> Json<SessionSummary> {
        Json(session_summary(&self.state).await)
    }

    #[rmcp::tool(
        description = "The deploy list: pull requests waiting to be deployed (no `deployed_at`) \
            and ones already deployed."
    )]
    async fn list_deploys(&self) -> Result<Json<DeployList>, CallToolResult> {
        let deploys = deploy_views(&self.state)
            .await
            .ok_or_else(|| tool_error("internal", "Could not read the deploy list"))?;
        Ok(Json(DeployList { deploys }))
    }

    #[rmcp::tool(
        description = "Add a pull request to the deploy list, which the user reads in the \
            dashboard; this tool is the only way onto it. Give a `title`, the producing \
            `session_id`, the `ticket` it resolves when there is one, and a one-line `note` on \
            what it changes. Calling it again for a URL already listed updates the fields you \
            give, for example to link a ticket later."
    )]
    async fn add_deploy(
        &self,
        Parameters(p): Parameters<AddDeployParams>,
    ) -> Result<Json<Deploy>, CallToolResult> {
        self.writable().map_err(|e| *e)?;
        let new = NewDeploy {
            url: p.url,
            title: p.title,
            session_id: p.session_id,
            ticket: p.ticket,
            note: p.note,
            tested: p.tested,
        };
        blocking(move || coordinator::add_deploy(new))
            .await
            .map_err(|e| *e)?
            .map(Json)
            .map_err(|e| tool_error("invalid_request", format!("{e:#}")))
    }

    #[rmcp::tool(
        description = "Mark a deploy as deployed, or as pending with `deployed: false`. Pass \
            `tested` to mark it tested or untested; given alone, it leaves the deployed state."
    )]
    async fn set_deployed(
        &self,
        Parameters(p): Parameters<SetDeployedParams>,
    ) -> Result<Json<Deploy>, CallToolResult> {
        self.writable().map_err(|e| *e)?;
        let id = p.id.clone();
        let change = coordinator::DeployChange {
            deployed: p.deployed.or(p.tested.is_none().then_some(true)),
            tested: p.tested,
        };
        match blocking(move || coordinator::update_deploy(&p.id, change))
            .await
            .map_err(|e| *e)?
        {
            Ok(Some(deploy)) => Ok(Json(deploy)),
            Ok(None) => Err(deploy_not_found(&id)),
            Err(e) => Err(tool_error("internal", format!("{e:#}"))),
        }
    }

    #[rmcp::tool(description = "Remove a pull request from the deploy list.")]
    async fn remove_deploy(
        &self,
        Parameters(p): Parameters<DeployIdParams>,
    ) -> Result<Json<DeployList>, CallToolResult> {
        self.writable().map_err(|e| *e)?;
        let id = p.id.clone();
        match blocking(move || coordinator::remove_deploy(&p.id))
            .await
            .map_err(|e| *e)?
        {
            Ok(true) => self.list_deploys().await,
            Ok(false) => Err(deploy_not_found(&id)),
            Err(e) => Err(tool_error("internal", format!("{e:#}"))),
        }
    }

    #[rmcp::tool(
        description = "The user's notes: the Markdown they keep in the dashboard's Notes tab \
            beside the coordinator, for anything they want to keep track of."
    )]
    async fn read_notes(&self) -> Result<Json<Notes>, CallToolResult> {
        match blocking(coordinator::notes).await.map_err(|e| *e)? {
            Ok(text) => Ok(Json(Notes { text })),
            Err(e) => Err(tool_error("internal", format!("{e:#}"))),
        }
    }

    #[rmcp::tool(
        description = "Change the user's notes. Prefer `append: true` to add an item; replace \
            the whole text only to edit or remove what is there, after `read_notes`, keeping \
            everything the user wrote that you were not asked to change. Returns the notes."
    )]
    async fn write_notes(
        &self,
        Parameters(p): Parameters<WriteNotesParams>,
    ) -> Result<Json<Notes>, CallToolResult> {
        self.writable().map_err(|e| *e)?;
        match blocking(move || coordinator::write_notes(&p.text, p.append))
            .await
            .map_err(|e| *e)?
        {
            Ok(text) => Ok(Json(Notes { text })),
            Err(e) => Err(tool_error("internal", format!("{e:#}"))),
        }
    }

    #[rmcp::tool(
        description = "Message the coordinator, the agent that oversees every session for the \
            user. Use it when you need a decision you should not make alone (scope, behaviour, \
            trade-offs, anything destructive), when you are blocked, or to report a milestone \
            such as an opened pull request. Set `needs_decision` when you cannot continue \
            without an answer, then end your turn: the answer arrives as your next message. \
            Only sessions AoE launched can call this."
    )]
    async fn message_coordinator(
        &self,
        Parameters(p): Parameters<MessageParams>,
        Extension(parts): Extension<axum::http::request::Parts>,
    ) -> Result<Json<Delivered>, CallToolResult> {
        self.writable().map_err(|e| *e)?;
        if p.message.trim().is_empty() {
            return Err(tool_error("invalid_request", "The message is empty"));
        }
        let caller = parts
            .headers
            .get(crate::daemon::MCP_CALLER_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
            .ok_or_else(|| {
                tool_error(
                    "no_caller",
                    "Only a session AoE launched can message the coordinator.",
                )
            })?;
        let title = self
            .state
            .instances
            .read()
            .await
            .iter()
            .find(|i| i.id == caller)
            .map(|i| i.title.clone())
            .ok_or_else(|| tool_error("no_caller", "The calling session no longer exists."))?;
        if crate::server::api::coordinator_session(&self.state)
            .await
            .as_deref()
            == Some(caller.as_str())
        {
            return Err(tool_error(
                "invalid_request",
                "You are the coordinator; reach the user with AskUserQuestion instead.",
            ));
        }
        let watched = caller.clone();
        // A session that reaches out is worth following from then on.
        let _ = blocking(move || {
            coordinator::update_watchlist(|list| {
                list.sessions.insert(watched);
            })
        })
        .await;
        let text = coordinator_message(&title, &caller, &p.message, p.needs_decision);
        crate::server::coordinator::tell_coordinator(&self.state, &text)
            .await
            .map(|coordinator_session_id| {
                Json(Delivered {
                    coordinator_session_id,
                })
            })
            .map_err(|reason| {
                tool_error(
                    reason,
                    match reason {
                        "no_coordinator" => {
                            "There is no coordinator session; the user opens it from the \
                             dashboard. Ask the user directly instead."
                        }
                        _ => "The coordinator could not take the message right now.",
                    },
                )
            })
    }
}

impl McpServer {
    fn writable(&self) -> Result<(), Box<CallToolResult>> {
        read_only_refusal(&self.state).map_or(Ok(()), |r| Err(Box::new(r)))
    }
}

fn coordinator_message(title: &str, id: &str, message: &str, needs_decision: bool) -> String {
    let (kind, ask) = if needs_decision {
        (
            "needs a decision",
            "It is waiting on you. Settle it with `send` if your workflow covers it; otherwise ask \
             the user, then `send` the session the answer.",
        )
    } else {
        (
            "sent an update",
            "Act on it if your workflow says to; no reply is needed otherwise.",
        )
    };
    format!(
        "[aoe message] \"{title}\" (`{id}`) {kind}:\n\n{}\n\n{ask}",
        message.trim()
    )
}

fn deploy_not_found(id: &str) -> CallToolResult {
    tool_error("not_found", format!("No deploy with id {id}"))
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, Box<CallToolResult>> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|e| Box::new(tool_error("internal", e.to_string())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coordinator_message_names_the_session_and_what_it_needs() {
        let decision = coordinator_message("PI-1921", "abc", " Use v2 API? ", true);
        assert!(decision.starts_with("[aoe message] \"PI-1921\" (`abc`) needs a decision:"));
        assert!(decision.contains("\n\nUse v2 API?\n\n"));
        let update = coordinator_message("PI-1921", "abc", "PR opened", false);
        assert!(update.contains("sent an update") && update.contains("no reply is needed"));
    }
}
