//! Plugin enable/disable and external install / uninstall.

use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::Stdio;

use anyhow::{anyhow, bail, Context, Result};
use aoe_plugin_api::agent_integration::AgentIntegrationContribution;
use aoe_plugin_api::{BuildStep, PluginId, PluginManifest, RuntimeSpec, UiContribution};
use serde::Serialize;

use crate::session::{update_config, CapabilityGrant, Config, PluginConfig};

use super::featured::FeaturedIndex;
use super::fetch::{self, FetchedPlugin};
use super::lockfile::{LockedPlugin, Lockfile};
use super::registry::ValidationState;
use super::source::PluginSource;
use super::NodeEnv;

pub enum OperationLog {
    Inherit,
    File(std::fs::File),
}

impl OperationLog {
    pub fn file(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating plugin job log dir {}", parent.display()))?;
        }
        let mut opts = std::fs::OpenOptions::new();
        opts.create(true).append(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let file = opts
            .open(path)
            .with_context(|| format!("opening plugin job log {}", path.display()))?;
        Ok(OperationLog::File(file))
    }

    fn line(&self, msg: &str) {
        match self {
            OperationLog::Inherit => eprintln!("{msg}"),
            OperationLog::File(file) => {
                let _ = writeln!(&mut &*file, "{msg}");
            }
        }
    }

    fn child_stdio(&self) -> Result<(Stdio, Stdio)> {
        match self {
            OperationLog::Inherit => Ok((Stdio::inherit(), Stdio::inherit())),
            OperationLog::File(file) => {
                let out = file.try_clone().context("cloning plugin job log handle")?;
                let err = file.try_clone().context("cloning plugin job log handle")?;
                Ok((Stdio::from(out), Stdio::from(err)))
            }
        }
    }
}

pub fn set_enabled(plugin_id: &str, enabled: bool) -> Result<()> {
    let registry = super::registry();
    if registry.get(plugin_id).is_none() {
        bail!("unknown plugin {plugin_id:?}; see `aoe plugin list`");
    }
    enable_in_config(plugin_id, enabled)?;
    super::reload_registry();
    tracing::info!(target: "plugin.host", plugin = %plugin_id, enabled, "plugin toggled");
    Ok(())
}

#[derive(Debug)]
pub enum LiveToggle {
    Daemon,
    Local,
    LocalDaemonStale { reason: String },
}

pub(crate) const WEB_TOGGLE_RECOVERY: &str =
    "Local transport/404: run `aoe serve --stop`, inspect state, then retry";

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

/// Web RPC errors are terminal: idempotent local writes would bypass daemon
/// authorization and live-worker reconciliation. Other plugins retain local fallback.
pub async fn set_enabled_live(plugin_id: &str, enabled: bool) -> Result<LiveToggle> {
    if super::registry().get(plugin_id).is_none() {
        bail!("unknown plugin {plugin_id:?}; see `aoe plugin list`");
    }
    let endpoint = match crate::acp::client::discovery::discover_local() {
        Ok(endpoint) => endpoint,
        Err(_) => {
            set_enabled(plugin_id, enabled)?;
            return Ok(LiveToggle::Local);
        }
    };
    let daemon_result = async {
        let client = crate::acp::client::HttpClient::new(endpoint)?;
        client.set_plugin_enabled(plugin_id, enabled).await
    }
    .await;
    match daemon_result {
        Ok(()) => {
            super::reload_registry();
            Ok(LiveToggle::Daemon)
        }
        Err(e) if plugin_id == "aoe.web" => Err(web_toggle_error(e.into())),
        Err(e) => {
            set_enabled(plugin_id, enabled)?;
            Ok(LiveToggle::LocalDaemonStale {
                reason: format!("{e}"),
            })
        }
    }
}

#[derive(Debug)]
pub enum LiveRestart {
    Daemon,
    NoDaemon,
    DaemonStale { reason: String },
}

pub async fn restart_worker_live(plugin_id: &str) -> LiveRestart {
    let Ok(endpoint) = crate::acp::client::discovery::discover_local() else {
        return LiveRestart::NoDaemon;
    };
    let result = async {
        let client = crate::acp::client::HttpClient::new(endpoint)?;
        client.restart_plugin_worker(plugin_id).await
    }
    .await;
    match result {
        Ok(()) => LiveRestart::Daemon,
        Err(crate::acp::client::HttpError::ReadOnly) => LiveRestart::NoDaemon,
        Err(e) => LiveRestart::DaemonStale {
            reason: format!("{e}"),
        },
    }
}

fn enable_in_config(plugin_id: &str, enabled: bool) -> Result<()> {
    update_config(|config| {
        config
            .plugins
            .entry(plugin_id.to_string())
            .or_insert_with(PluginConfig::default)
            .enabled = enabled;
    })
}

#[derive(Debug)]
pub struct InstallReport {
    pub id: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub granted: bool,
    pub validation: ValidationState,
}

fn install_validation(featured_verified: bool, dev: bool, source: &str) -> ValidationState {
    if dev {
        ValidationState::Dev
    } else if featured_verified {
        ValidationState::Featured
    } else if source.starts_with("gh:") {
        ValidationState::Community
    } else {
        ValidationState::Local
    }
}

fn trust_label(featured_verified: bool, dev: bool) -> &'static str {
    if dev {
        "dev"
    } else if featured_verified {
        "featured"
    } else {
        "community"
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct UiView {
    pub slot: String,
    pub id: String,
}

struct PreparedInstall {
    persisted_source: String,
    unverified: bool,
    notice: String,
    fetched: FetchedPlugin,
    featured_verified: bool,
    dev: bool,
    id: String,
    capabilities: Vec<String>,
    manifest_hash: String,
    fingerprint: String,
    validation: ValidationState,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstallConsent {
    pub id: String,
    pub version: String,
    pub source: String,
    pub notice: String,
    pub unverified: bool,
    pub validation: String,
    pub capabilities: Vec<String>,
    pub ui: Vec<UiView>,
    pub build_steps: Vec<String>,
    pub agent_integrations: Vec<AgentIntegrationContribution>,
    pub fingerprint: String,
}

async fn prepare_install(input: &str, dev: bool) -> Result<PreparedInstall> {
    let source = PluginSource::parse(input)?;
    if dev && !matches!(source, PluginSource::Local(_)) {
        bail!("--dev requires a local plugin directory");
    }
    let resolved = resolve_source(source, true).await?;
    let fetched = fetch::fetch(&resolved.source).await?;

    let id = fetched.manifest.id.as_str().to_string();
    let featured_verified = !dev && verify_featured(&FeaturedIndex::load()?, &fetched)?;
    reject_reserved_or_builtin(&fetched.manifest, featured_verified || dev)?;
    reject_incompatible_host(&fetched.manifest)?;

    if super::plugins_dir()?.join(&id).exists() {
        bail!("{id} is already installed; run `aoe plugin update {id}` or uninstall it first");
    }

    let capabilities = capability_strings(&fetched)?;
    let manifest_hash = PluginManifest::hash_bytes(&fetched.manifest_bytes);
    let trust = trust_label(featured_verified, dev);
    let fingerprint = fingerprint(&fetched.tree_hash, fetched.asset_sha256.as_deref(), trust);
    let persisted_source = persisted_source(&resolved.source, input);
    let validation = install_validation(featured_verified, dev, &persisted_source);

    Ok(PreparedInstall {
        persisted_source,
        unverified: resolved.unverified,
        notice: resolved.notice,
        fetched,
        featured_verified,
        dev,
        id,
        capabilities,
        manifest_hash,
        fingerprint,
        validation,
    })
}

fn apply_prepared_install(
    p: &PreparedInstall,
    node_env: &NodeEnv,
    log: &OperationLog,
) -> Result<InstallReport> {
    let final_dir = super::plugins_dir()?.join(&p.id);
    if final_dir.exists() {
        bail!(
            "{} is already installed; run `aoe plugin update {}` or uninstall it first",
            p.id,
            p.id
        );
    }

    log.line(&format!(
        "installing {} {}",
        p.id, p.fetched.manifest.version
    ));
    move_into_place(&p.fetched, &final_dir)?;
    if let Err(e) = build_in_place(&p.id, &final_dir, &p.fetched.manifest, node_env, log) {
        let _ = std::fs::remove_dir_all(&final_dir);
        return Err(e);
    }

    let persisted = (|| -> Result<()> {
        persist_install(
            &p.persisted_source,
            &p.id,
            &p.capabilities,
            &p.manifest_hash,
            p.dev,
        )?;
        write_lock(
            &p.id,
            &p.fetched,
            &p.manifest_hash,
            trust_label(p.featured_verified, p.dev),
        )
    })();
    if let Err(e) = persisted {
        // Not `uninstall`: nothing finished installing, so nothing is announced as uninstalled.
        let rollback = match is_installed_external(&p.id) {
            Ok(true) => remove_external(&p.id),
            Ok(false) => std::fs::remove_dir_all(&final_dir)
                .with_context(|| format!("removing {}", final_dir.display())),
            // An unreadable config must not strand the staged tree, or the next
            // install of the same plugin is refused as already installed.
            Err(error) => {
                tracing::warn!(target: "plugin.install", plugin = %p.id, %error, "install rollback: config check failed; removing staged tree");
                std::fs::remove_dir_all(&final_dir)
                    .with_context(|| format!("removing {}", final_dir.display()))
            }
        };
        if let Err(error) = rollback {
            tracing::warn!(target: "plugin.install", plugin = %p.id, %error, "install rollback failed");
        }
        return Err(e);
    }
    super::reload_registry();
    log.line(&format!(
        "installed {} {}",
        p.id, p.fetched.manifest.version
    ));
    tracing::info!(target: "plugin.install", plugin = %p.id, version = %p.fetched.manifest.version, "plugin installed");

    Ok(InstallReport {
        id: p.id.clone(),
        version: p.fetched.manifest.version.clone(),
        capabilities: p.capabilities.clone(),
        granted: true,
        validation: p.validation,
    })
}

pub async fn install(input: &str, assume_yes: bool, dev: bool) -> Result<InstallReport> {
    let prepared = prepare_install(input, dev).await?;
    eprintln!("{}", prepared.notice);
    if prepared.unverified && !assume_yes && !confirm_unverified()? {
        bail!("install cancelled; the unverified source was not approved");
    }
    let build = build_steps(&prepared.fetched.manifest);
    let granted = if assume_yes
        || !install_needs_consent(
            &prepared.capabilities,
            build,
            &prepared.fetched.manifest.ui,
            &prepared.fetched.manifest.agent_integrations,
        ) {
        true
    } else {
        confirm_capabilities(
            &prepared.id,
            &prepared.capabilities,
            &prepared.fetched.manifest.ui,
            build,
            &prepared.fetched.manifest.agent_integrations,
        )?
    };
    if !granted {
        bail!("install cancelled; no capabilities were granted");
    }
    let node_env = apply_node_env(&prepared.fetched.manifest).await;
    apply_prepared_install(&prepared, &node_env, &OperationLog::Inherit)
}

pub async fn preview_install(input: &str) -> Result<InstallConsent> {
    if !input.starts_with("gh:") {
        bail!("web install supports gh: sources only; use `aoe plugin install` for a local path");
    }
    let p = prepare_install(input, false).await?;
    Ok(InstallConsent {
        id: p.id.clone(),
        version: p.fetched.manifest.version.clone(),
        source: p.persisted_source.clone(),
        notice: p.notice.clone(),
        unverified: p.unverified,
        validation: p.validation.as_str().to_string(),
        capabilities: p.capabilities.clone(),
        ui: p
            .fetched
            .manifest
            .ui
            .iter()
            .map(|u| UiView {
                slot: u.slot.as_str().to_string(),
                id: u.id.clone(),
            })
            .collect(),
        build_steps: build_steps(&p.fetched.manifest)
            .iter()
            .map(|s| s.command.join(" "))
            .collect(),
        agent_integrations: disclosure_integrations(&p.fetched.manifest.agent_integrations),
        fingerprint: p.fingerprint.clone(),
    })
}

pub async fn apply_install(
    input: &str,
    expected_fingerprint: &str,
    log: &OperationLog,
) -> Result<InstallReport> {
    if !input.starts_with("gh:") {
        bail!("web install supports gh: sources only; use `aoe plugin install` for a local path");
    }
    let prepared = prepare_install(input, false).await?;
    if prepared.fingerprint != expected_fingerprint {
        bail!(
            "the plugin at {input} changed since it was shown; review it again before installing"
        );
    }
    let node_env = apply_node_env(&prepared.fetched.manifest).await;
    apply_prepared_install(&prepared, &node_env, log)
}

/// The consent structures serialize to the CLI/TUI/web disclosures, so their
/// agent commands carry the display-escaped form (control and bidi characters
/// made visible) rather than the raw bytes. The runnable command in the
/// manifest is never altered.
fn disclosure_integrations(
    integrations: &[AgentIntegrationContribution],
) -> Vec<AgentIntegrationContribution> {
    integrations
        .iter()
        .map(|integration| {
            let mut out = integration.clone();
            if let Some(command) = &out.command {
                out.command = Some(super::agent_integration::display_command(command));
            }
            out
        })
        .collect()
}

fn fingerprint(tree_hash: &str, asset_sha256: Option<&str>, trust: &str) -> String {
    format!("{tree_hash}|{}|{trust}", asset_sha256.unwrap_or(""))
}

#[derive(Debug, Clone, Serialize)]
pub struct ReapproveConsent {
    pub id: String,
    pub version: String,
    pub validation: String,
    pub capabilities: Vec<String>,
    pub ui: Vec<UiView>,
    pub agent_integrations: Vec<AgentIntegrationContribution>,
    pub manifest_hash: String,
}

pub fn reapprove_consent(id: &str) -> Result<ReapproveConsent> {
    let registry = super::registry();
    let plugin = registry
        .get(id)
        .ok_or_else(|| anyhow!("unknown plugin {id:?}; see `aoe plugin list`"))?;
    if plugin.builtin() {
        bail!("{id} is a builtin plugin; it is always granted");
    }
    let unknown: Vec<&str> = plugin
        .manifest
        .capabilities
        .iter()
        .filter(|c| !c.is_known())
        .map(|c| c.as_str())
        .collect();
    if !unknown.is_empty() {
        bail!(
            "plugin requests capabilities this host does not support: {}; upgrade aoe",
            unknown.join(", ")
        );
    }
    Ok(ReapproveConsent {
        id: id.to_string(),
        version: plugin.manifest.version.clone(),
        validation: plugin.validation.as_str().to_string(),
        capabilities: plugin
            .manifest
            .capabilities
            .iter()
            .map(|c| c.as_str().to_string())
            .collect(),
        ui: plugin
            .manifest
            .ui
            .iter()
            .map(|u| UiView {
                slot: u.slot.as_str().to_string(),
                id: u.id.clone(),
            })
            .collect(),
        agent_integrations: disclosure_integrations(&plugin.manifest.agent_integrations),
        manifest_hash: plugin.manifest_hash.clone(),
    })
}

pub fn approve_installed(id: &str, expected_manifest_hash: &str) -> Result<()> {
    super::reload_registry();
    let consent = reapprove_consent(id)?;
    if consent.manifest_hash != expected_manifest_hash {
        bail!("{id} changed on disk since its disclosure was shown; review it again");
    }
    update_config(|config| {
        let Some(entry) = config.plugins.get_mut(id) else {
            bail!("{id} is not an installed external plugin");
        };
        if entry.source.is_none() {
            bail!("{id} is not an installed external plugin");
        }
        entry.grant = Some(CapabilityGrant {
            manifest_hash: consent.manifest_hash.clone(),
            capabilities: consent.capabilities.clone(),
            granted_at: chrono::Utc::now(),
        });
        Ok(())
    })??;
    super::reload_registry();
    Ok(())
}

pub fn nudge_daemon_enabled(changes: Vec<(String, bool)>) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static GENERATION: AtomicU64 = AtomicU64::new(0);
    static IN_FLIGHT: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    if changes.is_empty() {
        return;
    }
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    tokio::spawn(async move {
        let _serialize = IN_FLIGHT.lock().await;
        if GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        let Ok(endpoint) = crate::acp::client::discovery::discover_local() else {
            return;
        };
        let client = match crate::acp::client::HttpClient::new(endpoint) {
            Ok(client) => client,
            Err(e) => {
                tracing::warn!("plugin toggle: daemon client build failed: {e}");
                return;
            }
        };
        for (id, enabled) in changes {
            if GENERATION.load(Ordering::SeqCst) != generation {
                return;
            }
            if let Err(e) = client.set_plugin_enabled(&id, enabled).await {
                tracing::warn!(
                    "plugin toggle: daemon did not reconcile {id} (enabled={enabled}): {e}; \
                         restart the daemon or toggle from the dashboard"
                );
            }
        }
    });
}

pub fn uninstall(id: &str) -> Result<()> {
    PluginId::new(id.to_string()).map_err(|e| anyhow!("{e}"))?;
    if !is_installed_external(id)? {
        bail!("{id} is not an installed external plugin");
    }
    let version = super::registry()
        .get(id)
        .map(|p| p.manifest.version.clone());
    remove_external(id)?;
    tracing::info!(target: "plugin.install", plugin = %id, ?version, "plugin uninstalled");
    Ok(())
}

fn is_installed_external(id: &str) -> Result<bool> {
    let config = Config::load()?;
    Ok(config
        .plugins
        .get(id)
        .and_then(|p| p.source.as_ref())
        .is_some())
}

/// Drop the plugin's directory, config entry and lock entry.
fn remove_external(id: &str) -> Result<()> {
    let dir = super::plugins_dir()?.join(id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
    }
    update_config(|config| config.plugins.remove(id))?;
    let mut lock = Lockfile::load()?;
    if lock.remove(id) {
        lock.save()?;
    }
    super::reload_registry();
    Ok(())
}

pub fn uninstall_logged(id: &str, log: &OperationLog) -> Result<()> {
    log.line(&format!("uninstalling {id}"));
    uninstall(id)?;
    log.line(&format!("uninstalled {id}"));
    Ok(())
}

fn reject_reserved_or_builtin(manifest: &PluginManifest, featured_verified: bool) -> Result<()> {
    let id = manifest.id.as_str();
    if super::registry::is_builtin_id(id) {
        bail!("plugin id {id:?} collides with a builtin plugin");
    }
    if manifest.id.is_reserved_namespace() && !featured_verified {
        bail!("plugin id {id:?} uses a reserved namespace (aoe.* / agent-of-empires.*); only a featured-verified plugin may claim one");
    }
    Ok(())
}

fn reject_incompatible_host(manifest: &PluginManifest) -> Result<()> {
    manifest
        .host_compat(env!("CARGO_PKG_VERSION"))
        .map_err(|msg| anyhow!("{}: {msg}", manifest.id.as_str()))
}

struct ResolvedSource {
    source: PluginSource,
    unverified: bool,
    notice: String,
}

async fn resolve_source(
    source: PluginSource,
    allow_branch_fallback: bool,
) -> Result<ResolvedSource> {
    match &source {
        PluginSource::Local(_) => Ok(ResolvedSource {
            unverified: false,
            notice: "installing from a local directory".to_string(),
            source,
        }),
        PluginSource::Github {
            reference: Some(reference),
            ..
        } => {
            let notice =
                format!("installing the explicit ref {reference:?} (not an audited release)");
            Ok(ResolvedSource {
                unverified: true,
                notice,
                source,
            })
        }
        PluginSource::Github {
            owner,
            repo,
            reference: None,
        } => match fetch::latest_release_tag(owner, repo).await? {
            Some(tag) => {
                let notice = format!("installing the latest release {tag}");
                let source = PluginSource::Github {
                    owner: owner.clone(),
                    repo: repo.clone(),
                    reference: Some(tag),
                };
                Ok(ResolvedSource {
                    unverified: false,
                    notice,
                    source,
                })
            }
            None if allow_branch_fallback => Ok(ResolvedSource {
                unverified: true,
                notice: format!(
                    "{owner}/{repo} has no published release; falling back to the unverified default branch"
                ),
                source,
            }),
            None => bail!(
                "{owner}/{repo} has no published release to update to; the prior version is kept"
            ),
        },
    }
}

fn confirm_unverified() -> Result<bool> {
    if !io::stdin().is_terminal() {
        bail!("this is unverified, un-audited code; re-run with --yes to install it");
    }
    println!(
        "This is unverified, un-audited code: it does not come from a vetted release and is\n\
         not covered by the featured index. Install it only if you trust the source."
    );
    print!("Continue? [y/N] ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn verify_featured(featured: &FeaturedIndex, fetched: &FetchedPlugin) -> Result<bool> {
    let id = fetched.manifest.id.as_str();
    let Some(entry) = featured.get(id) else {
        return Ok(false);
    };
    if matches!(
        fetched.manifest.runtime,
        Some(RuntimeSpec::ReleaseBinary { .. })
    ) {
        bail!("{id} is featured but ships a release-binary worker, which the featured index cannot pin yet; refusing install");
    }
    let slug = fetched.source.slug();
    if !slug.eq_ignore_ascii_case(&entry.source) {
        bail!(
            "{id} is featured from {:?} but you are installing from {slug:?}; refusing install",
            entry.source
        );
    }
    Ok(entry.verifies(&fetched.tree_hash))
}

fn capability_strings(fetched: &FetchedPlugin) -> Result<Vec<String>> {
    let unknown: Vec<&str> = fetched
        .manifest
        .capabilities
        .iter()
        .filter(|c| !c.is_known())
        .map(|c| c.as_str())
        .collect();
    if !unknown.is_empty() {
        bail!(
            "plugin requests capabilities this host does not support: {}; upgrade aoe",
            unknown.join(", ")
        );
    }
    Ok(fetched
        .manifest
        .capabilities
        .iter()
        .map(|c| c.as_str().to_string())
        .collect())
}

fn install_needs_consent(
    capabilities: &[String],
    build: &[BuildStep],
    ui: &[UiContribution],
    integrations: &[AgentIntegrationContribution],
) -> bool {
    !capabilities.is_empty() || !build.is_empty() || !ui.is_empty() || !integrations.is_empty()
}

fn confirm_capabilities(
    id: &str,
    capabilities: &[String],
    ui: &[UiContribution],
    build: &[BuildStep],
    integrations: &[AgentIntegrationContribution],
) -> Result<bool> {
    if !io::stdin().is_terminal() {
        bail!(
            "{id} declares plugin access or executable commands but stdin is not a terminal; re-run with --yes to approve them",
        );
    }
    if !capabilities.is_empty() {
        println!("Plugin {id} requests these capabilities:");
        for capability in capabilities {
            println!("  - {capability}");
        }
    }
    if !ui.is_empty() {
        println!("Plugin {id} will add UI elements to these dashboard slots:");
        for u in ui {
            println!("  - {} ({})", u.slot.as_str(), u.id);
        }
    }
    if !build.is_empty() {
        println!(
            "Plugin {id} will run these build commands now, in its install directory,\n\
             as your user and outside capability enforcement:"
        );
        for step in build {
            println!("  $ {}", step.command.join(" "));
        }
    }
    if !integrations.is_empty() {
        println!("Plugin {id} declares these agent commands (run as your user, unsandboxed):");
        for integration in integrations {
            println!(
                "  {}: {}",
                integration.tool,
                integration
                    .command
                    .as_deref()
                    .map(super::agent_integration::display_command)
                    .unwrap_or_else(|| "No launch command declared".to_string())
            );
        }
    }
    println!(
        "Note: installing trusts this plugin. The host checks capabilities at its API boundary,\n\
         but a plugin worker (and any build step) runs without OS-level sandboxing, so a malicious\n\
         plugin is not contained. Build steps run as your user before any capability gate. Only\n\
         install plugins you trust."
    );
    print!("Grant them and install? [y/N] ");
    io::stdout().flush()?;
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn move_into_place(fetched: &FetchedPlugin, final_dir: &std::path::Path) -> Result<()> {
    if final_dir.exists() {
        std::fs::remove_dir_all(final_dir)
            .with_context(|| format!("replacing {}", final_dir.display()))?;
    }
    std::fs::rename(&fetched.tree, final_dir).with_context(|| {
        format!(
            "moving plugin into {} (cross-device staging?)",
            final_dir.display()
        )
    })
}

fn build_steps(manifest: &PluginManifest) -> &[BuildStep] {
    match &manifest.runtime {
        Some(RuntimeSpec::Command { build, .. }) => build,
        _ => &[],
    }
}

/// Resolve Node once, off the runtime threads, for the sync apply step.
async fn apply_node_env(manifest: &PluginManifest) -> NodeEnv {
    if !super::needs_node(manifest) {
        return NodeEnv::new();
    }
    super::ensure_node().await;
    tokio::task::spawn_blocking(super::node_env)
        .await
        .unwrap_or_else(|error| {
            tracing::warn!(target: "plugin.install", %error, "Node resolution task failed");
            NodeEnv::new()
        })
}

fn build_in_place(
    plugin_id: &str,
    dir: &Path,
    manifest: &PluginManifest,
    node_env: &NodeEnv,
    log: &OperationLog,
) -> Result<()> {
    run_build(plugin_id, dir, build_steps(manifest), node_env, log)?;
    if let Some(RuntimeSpec::Command {
        command,
        system: false,
        ..
    }) = &manifest.runtime
    {
        let resolver = super::launch::OsLaunchResolver::new(node_env.clone());
        super::launch::resolve_command(plugin_id, dir, command, &resolver).with_context(|| {
            format!(
                "plugin {plugin_id}: worker command is not runnable after install \
                     (a build step may have been skipped on this platform, or did not produce it)"
            )
        })?;
    }
    Ok(())
}

fn run_build(
    plugin_id: &str,
    dir: &Path,
    steps: &[BuildStep],
    node_env: &NodeEnv,
    log: &OperationLog,
) -> Result<()> {
    let os = std::env::consts::OS;
    let resolver = super::launch::OsLaunchResolver::new(node_env.clone());
    for (i, step) in steps.iter().enumerate() {
        if !step.platforms.is_empty() && !step.platforms.iter().any(|p| p == os) {
            continue;
        }
        let pretty = step.command.join(" ");
        let (program, args) =
            super::launch::resolve_command(plugin_id, dir, &step.command, &resolver)
                .with_context(|| format!("resolving build step {} ({pretty})", i + 1))?;
        log.line(&format!("  building {plugin_id}: {pretty}"));
        let (stdout, stderr) = log.child_stdio()?;
        let status = std::process::Command::new(&program)
            .args(&args)
            .current_dir(dir)
            .env("AOE_PLUGIN_ID", plugin_id)
            .envs(node_env.iter().map(|(k, v)| (*k, v)))
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr)
            .status()
            .with_context(|| format!("spawning build step {} ({pretty})", i + 1))?;
        if !status.success() {
            bail!("build step {} ({pretty}) failed with {status}", i + 1);
        }
    }
    Ok(())
}

fn persisted_source(source: &PluginSource, input: &str) -> String {
    match source {
        PluginSource::Github { .. } => input.to_string(),
        PluginSource::Local(path) => std::fs::canonicalize(path)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| input.to_string()),
    }
}

fn persist_install(
    source: &str,
    id: &str,
    capabilities: &[String],
    manifest_hash: &str,
    dev: bool,
) -> Result<()> {
    update_config(|config| {
        let entry = config
            .plugins
            .entry(id.to_string())
            .or_insert_with(PluginConfig::default);
        entry.source = Some(source.to_string());
        entry.dev = dev;
        entry.grant = Some(CapabilityGrant {
            manifest_hash: manifest_hash.to_string(),
            capabilities: capabilities.to_vec(),
            granted_at: chrono::Utc::now(),
        });
    })
}

fn write_lock(id: &str, fetched: &FetchedPlugin, manifest_hash: &str, trust: &str) -> Result<()> {
    let mut lock = Lockfile::load()?;
    lock.upsert(
        id,
        LockedPlugin {
            source: fetched.source.slug(),
            requested_ref: fetched.requested_ref.clone(),
            resolved_commit: fetched.resolved_commit.clone(),
            version: fetched.manifest.version.clone(),
            manifest_hash: manifest_hash.to_string(),
            tree_hash: fetched.tree_hash.clone(),
            trust: trust.to_string(),
            release_tag: fetched.release_tag.clone(),
            asset_name: fetched.asset_name.clone(),
            asset_sha256: fetched.asset_sha256.clone(),
        },
    );
    lock.save()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::test_support::isolate_app_dir;
    use aoe_plugin_api::UiSlot;
    use serial_test::serial;

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

    fn ui(slot: UiSlot, id: &str) -> UiContribution {
        UiContribution {
            slot,
            id: id.to_string(),
            title: None,
            icon: None,
            entry: None,
            deep_link: false,
        }
    }

    fn manifest_with_aoe_version(range: Option<&str>) -> PluginManifest {
        let aoe = range
            .map(|r| format!("aoe_version = \"{r}\"\n"))
            .unwrap_or_default();
        PluginManifest::from_toml_str(&format!(
            "id = \"acme.thing\"\nname = \"Thing\"\nversion = \"1.0.0\"\napi_version = 4\n{aoe}"
        ))
        .unwrap()
    }

    #[test]
    fn only_manifests_that_build_or_run_a_non_system_worker_need_node() {
        let cases = [
            (None, false),
            (Some("kind = \"release-binary\"\nasset = \"w\""), false),
            (
                Some("kind = \"command\"\ncommand = [\"w\"]\nsystem = true"),
                false,
            ),
            (Some("kind = \"command\"\ncommand = [\"bin/w\"]"), true),
            (
                Some(
                    "kind = \"command\"\ncommand = [\"w\"]\nsystem = true\n\
                     build = [{ command = [\"npm\", \"ci\"] }]",
                ),
                true,
            ),
        ];
        for (runtime, expected) in cases {
            let runtime = runtime
                .map(|r| format!("capabilities = [\"runtime.worker\"]\n[runtime]\n{r}"))
                .unwrap_or_default();
            let manifest = PluginManifest::from_toml_str(&format!(
                "id = \"acme.thing\"\nname = \"Thing\"\nversion = \"1.0.0\"\napi_version = 4\n{runtime}"
            ))
            .unwrap();
            assert_eq!(crate::plugin::needs_node(&manifest), expected, "{runtime}");
        }
    }

    #[test]
    fn reject_incompatible_host_blocks_out_of_range_and_allows_in_range() {
        let in_range = manifest_with_aoe_version(Some(">=1.0.0, <2.0.0"));
        assert!(reject_incompatible_host(&in_range).is_ok());

        let out = manifest_with_aoe_version(Some(">=2.0.0"));
        let err = reject_incompatible_host(&out).unwrap_err().to_string();
        assert!(err.contains("acme.thing"), "{err}");
        assert!(err.contains("plugin requires aoe"), "{err}");

        assert!(reject_incompatible_host(&manifest_with_aoe_version(None)).is_ok());
    }

    #[test]
    fn install_consent_required_for_caps_build_ui_or_agent_integrations() {
        assert!(!install_needs_consent(&[], &[], &[], &[]));
        assert!(install_needs_consent(&["net".to_string()], &[], &[], &[]));
        assert!(install_needs_consent(
            &[],
            &[],
            &[ui(UiSlot::StatusBar, "s")],
            &[],
        ));
        assert!(install_needs_consent(
            &[],
            &[],
            &[],
            &[AgentIntegrationContribution {
                tool: "agent".into(),
                provider_versions: Vec::new(),
                command: Some("agent --flag".into()),
                replies: false,
                ..Default::default()
            }],
        ));
    }

    #[test]
    fn disclosure_integrations_escapes_commands_for_display_but_keeps_manifest_raw() {
        let raw = AgentIntegrationContribution {
            tool: "session-helper".into(),
            provider_versions: Vec::new(),
            command: Some("x\u{202e}tail".into()),
            replies: false,
            ..Default::default()
        };
        let disclosed = disclosure_integrations(std::slice::from_ref(&raw));
        // The consent that the CLI/TUI/web serialize carries the escaped form.
        assert_eq!(disclosed[0].command.as_deref(), Some("x\\u{202e}tail"));
        // The manifest / launch command stays raw.
        assert_eq!(raw.command.as_deref(), Some("x\u{202e}tail"));
    }

    #[tokio::test]
    async fn web_install_rejects_non_gh_sources() {
        let err = preview_install("/tmp/some/plugin")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("gh:"), "{err}");
        let err = apply_install("./local/dir", "fp", &OperationLog::Inherit)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("gh:"), "{err}");
    }

    /// Uninstall removes only a configured external plugin's directory: a
    /// malformed id or a bare plugins subdirectory is refused untouched, and a
    /// configured plugin whose directory is gone still has its config removed.
    #[test]
    #[serial]
    fn uninstall_only_removes_installed_external_plugins() {
        let _home = isolate_app_dir();
        let plugins = super::super::plugins_dir().unwrap();
        let jobs = plugins.join("jobs");
        let keep = plugins.join("keepdir");
        for dir in [&jobs, &keep] {
            std::fs::create_dir_all(dir).unwrap();
            std::fs::write(dir.join("keep.log"), b"keep").unwrap();
        }

        for (id, needle) in [
            ("jobs/../keepdir", "invalid plugin id"),
            ("jobs", "not an installed external plugin"),
        ] {
            let err = uninstall(id).unwrap_err().to_string();
            assert!(err.contains(needle), "{id}: {err}");
        }
        for dir in [&jobs, &keep] {
            assert!(
                dir.join("keep.log").exists(),
                "{} was removed",
                dir.display()
            );
        }

        update_config(|config| {
            config.plugins.insert(
                "acme.thing".to_string(),
                PluginConfig {
                    source: Some("gh:acme/thing".to_string()),
                    ..PluginConfig::default()
                },
            );
        })
        .unwrap();
        uninstall("acme.thing").unwrap();
        assert!(!Config::load().unwrap().plugins.contains_key("acme.thing"));
    }
}
