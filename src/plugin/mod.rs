//! Built-in plugins: compiled-in features the user can disable at runtime.

pub mod registry;
pub mod view;

use std::sync::{Arc, RwLock};

use anyhow::{bail, Result};

pub use registry::{LoadedPlugin, PluginRegistry};
pub use view::PluginView;

/// The Web dashboard's plugin id.
pub const WEB_PLUGIN_ID: &str = "aoe.web";

pub(crate) trait RwLockSafe<T> {
    fn read_safe(&self) -> std::sync::RwLockReadGuard<'_, T>;
    fn write_safe(&self) -> std::sync::RwLockWriteGuard<'_, T>;
}

impl<T> RwLockSafe<T> for RwLock<T> {
    fn read_safe(&self) -> std::sync::RwLockReadGuard<'_, T> {
        self.read().unwrap_or_else(|e| e.into_inner())
    }
    fn write_safe(&self) -> std::sync::RwLockWriteGuard<'_, T> {
        self.write().unwrap_or_else(|e| e.into_inner())
    }
}

static REGISTRY: RwLock<Option<Arc<PluginRegistry>>> = RwLock::new(None);

pub fn registry() -> Arc<PluginRegistry> {
    if let Some(reg) = REGISTRY.read_safe().as_ref() {
        return reg.clone();
    }
    let mut slot = REGISTRY.write_safe();
    if let Some(reg) = slot.as_ref() {
        return reg.clone();
    }
    let config = crate::session::Config::load_or_warn();
    let reg = Arc::new(PluginRegistry::load(&config));
    *slot = Some(reg.clone());
    reg
}

pub fn reload_registry() -> Arc<PluginRegistry> {
    let config = crate::session::Config::load_or_warn();
    let reg = Arc::new(PluginRegistry::load(&config));
    *REGISTRY.write_safe() = Some(reg.clone());
    reg
}

#[cfg(all(test, feature = "web"))]
pub(crate) struct ReloadRegistryOnDrop;

#[cfg(all(test, feature = "web"))]
impl Drop for ReloadRegistryOnDrop {
    fn drop(&mut self) {
        let _lock = crate::session::test_support::EnvGuard::unset(&[]);
        reload_registry();
    }
}

/// Whether global configuration disables the registered Web plugin.
pub fn web_disabled(config: &crate::session::Config) -> bool {
    config
        .plugins
        .get(WEB_PLUGIN_ID)
        .is_some_and(|plugin| !plugin.enabled)
        && registry().get(WEB_PLUGIN_ID).is_some()
}

pub fn set_enabled(plugin_id: &str, enabled: bool) -> Result<()> {
    if registry().get(plugin_id).is_none() {
        bail!("unknown plugin {plugin_id:?}; see `aoe plugin list`");
    }
    crate::session::update_config(|config| {
        config
            .plugins
            .entry(plugin_id.to_string())
            .or_default()
            .enabled = enabled;
    })?;
    reload_registry();
    tracing::info!(target: "plugin", plugin = %plugin_id, enabled, "plugin toggled");
    Ok(())
}

pub(crate) const WEB_TOGGLE_RECOVERY: &str =
    "Local transport/404: run `aoe serve --stop`, inspect state, then retry";

/// Points a failed Web toggle that never reached a Daemon at the recovery steps.
pub(crate) fn web_toggle_error(error: anyhow::Error) -> anyhow::Error {
    use crate::acp::client::HttpError;
    use crate::daemon::DaemonClientError;
    let daemon_recoverable = |error: &DaemonClientError| {
        matches!(
            error,
            DaemonClientError::UnixTransport
                | DaemonClientError::Transport
                | DaemonClientError::Timeout
        ) || matches!(error, DaemonClientError::Status { status, .. } if *status == reqwest::StatusCode::NOT_FOUND)
    };
    let recoverable = error.chain().any(|cause| {
        cause
            .downcast_ref::<HttpError>()
            .is_some_and(|error| match error {
                HttpError::Transport => true,
                HttpError::Server { status, .. } => *status == reqwest::StatusCode::NOT_FOUND,
                HttpError::Daemon(error) => daemon_recoverable(error),
                _ => false,
            })
            || cause
                .downcast_ref::<DaemonClientError>()
                .is_some_and(daemon_recoverable)
    });
    if recoverable {
        error.context(WEB_TOGGLE_RECOVERY)
    } else {
        error
    }
}

#[cfg(test)]
mod tests {
    use super::web_toggle_error;

    #[test]
    fn web_recovery_guidance_preserves_authority_refusals() {
        use crate::acp::client::HttpError;
        use crate::daemon::DaemonClientError;
        use reqwest::StatusCode;
        for (error, guidance) in [
            (HttpError::Transport, true),
            (HttpError::Daemon(DaemonClientError::UnixTransport), true),
            (HttpError::Daemon(DaemonClientError::Timeout), true),
            (HttpError::Daemon(DaemonClientError::PeerIdentity), false),
            (HttpError::Unauthorized, false),
            (HttpError::ReadOnly, false),
            (
                HttpError::Server {
                    status: StatusCode::FORBIDDEN,
                    body: "policy refused".into(),
                },
                false,
            ),
            (
                HttpError::Server {
                    status: StatusCode::CONFLICT,
                    body: "lifecycle_locked".into(),
                },
                false,
            ),
            (
                HttpError::Server {
                    status: StatusCode::NOT_FOUND,
                    body: "missing route".into(),
                },
                true,
            ),
        ] {
            let original = error.to_string();
            let result = web_toggle_error(error.into());
            assert!(result.downcast_ref::<HttpError>().is_some());
            assert_eq!(format!("{result:#}").contains("aoe serve --stop"), guidance);
            if !guidance {
                assert_eq!(result.to_string(), original);
            }
        }
    }
}
