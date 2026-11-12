//! `aoe plugin`: list and toggle the built-in plugins.

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum PluginCommands {
    /// List every built-in plugin with its version and state
    List,
    /// Enable a plugin
    Enable {
        /// Plugin id, e.g. `aoe.web`
        id: String,
    },
    /// Disable a plugin
    Disable {
        /// Plugin id, e.g. `aoe.web`
        id: String,
    },
}

pub async fn run(command: PluginCommands) -> Result<()> {
    match command {
        PluginCommands::List => run_list(),
        PluginCommands::Enable { id } => run_set_enabled(&id, true).await,
        PluginCommands::Disable { id } => run_set_enabled(&id, false).await,
    }
}

fn run_list() -> Result<()> {
    let registry = crate::plugin::registry();
    if registry.all().is_empty() {
        println!("No plugins.");
        return Ok(());
    }
    println!("{:<20} {:<9} STATE", "ID", "VERSION");
    for plugin in registry.all() {
        println!(
            "{:<20} {:<9} {}",
            plugin.id(),
            plugin.builtin.version,
            if plugin.enabled {
                "enabled"
            } else {
                "disabled"
            },
        );
    }
    Ok(())
}

async fn run_set_enabled(id: &str, enabled: bool) -> Result<()> {
    let result = async {
        let endpoint = crate::acp::client::require_daemon().await?;
        crate::acp::client::HttpClient::new(endpoint)?
            .set_plugin_enabled(id, enabled)
            .await?;
        Ok::<(), anyhow::Error>(())
    }
    .await;
    if id == crate::plugin::WEB_PLUGIN_ID
        && result.is_err()
        && crate::acp::client::discovery::discover_env().is_none()
    {
        result.map_err(crate::plugin::web_toggle_error)?;
    } else {
        result?;
    }
    println!("{} {id}.", if enabled { "Enabled" } else { "Disabled" });
    Ok(())
}
