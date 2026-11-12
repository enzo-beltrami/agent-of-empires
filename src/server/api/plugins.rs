//! Plugin REST API: list the built-in plugins and enable/disable them. The web
//! twin of `aoe plugin`.
//!
//! The toggle is a host mutation, so it requires read-write mode AND an
//! elevated session when login is enabled.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use serde_json::json;

use super::{api_error, AppState};
use crate::plugin;
use crate::server::auth::{handler_elevated, AuthenticatedSession};
use crate::server::principal::{authorize, Capability, Principal};

/// Apply mutation restrictions before resolving the authenticated principal.
async fn mutation_gate(
    state: &AppState,
    session: Option<&AuthenticatedSession>,
    local: Option<&Principal>,
) -> Result<(), Response> {
    if state.read_only {
        return Err(super::read_only_response());
    }
    if let Some(resp) = super::cityhall_block(state) {
        return Err(resp);
    }
    if !handler_elevated(state, session, authorize(local, Capability::SkipElevation)).await {
        return Err(super::api_error_with_code_header(
            crate::daemon::ApiErrorCode::ElevationRequired,
            "Re-enter the passphrase to continue",
        ));
    }
    Ok(())
}

/// `GET /api/plugins`: every built-in plugin.
pub async fn list_plugins(State(state): State<std::sync::Arc<AppState>>) -> Response {
    if let Some(resp) = super::cityhall_block(&state) {
        return resp;
    }
    let registry = plugin::registry();
    Json(json!({
        "plugins": registry.all().iter().map(|p| p.view()).collect::<Vec<_>>(),
    }))
    .into_response()
}

#[derive(Deserialize)]
pub struct SetEnabledBody {
    pub enabled: bool,
}

/// `POST /api/plugins/{id}/enabled`
pub async fn set_plugin_enabled(
    State(state): State<std::sync::Arc<AppState>>,
    session: Option<axum::Extension<AuthenticatedSession>>,
    loopback: Option<axum::Extension<Principal>>,
    Path(id): Path<String>,
    Json(body): Json<SetEnabledBody>,
) -> Response {
    if let Err(resp) = mutation_gate(&state, session.as_deref(), loopback.as_deref()).await {
        return resp;
    }
    let plugin = id.clone();
    let enabled = body.enabled;
    let result = tokio::task::spawn_blocking(move || plugin::set_enabled(&id, enabled)).await;
    match result {
        Ok(Ok(())) => list_plugins(State(state)).await,
        Ok(Err(e)) if e.is::<crate::daemon::lifecycle::TransactionBusy>() => {
            super::api_error_with_code_header(
                crate::daemon::ApiErrorCode::LifecycleLocked,
                format!("{e:#}"),
            )
        }
        Ok(Err(e)) => {
            tracing::warn!(target: "plugin", %plugin, enabled, error = %format!("{e:#}"), "plugin toggle failed");
            api_error(StatusCode::BAD_REQUEST, "plugin_error", format!("{e:#}"))
        }
        Err(e) => {
            tracing::warn!(target: "plugin", %plugin, enabled, error = %e, "plugin toggle task failed");
            api_error(StatusCode::INTERNAL_SERVER_ERROR, "internal", e.to_string())
        }
    }
}
