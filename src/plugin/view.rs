//! The plugin view-model the CLI and the dashboard render.

use serde::Serialize;

use super::registry::LoadedPlugin;

#[derive(Debug, Clone, Serialize)]
pub struct PluginView {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub enabled: bool,
}

impl LoadedPlugin {
    pub fn view(&self) -> PluginView {
        PluginView {
            id: self.id().to_string(),
            name: self.builtin.name.to_string(),
            version: self.builtin.version.to_string(),
            description: self.builtin.description.to_string(),
            enabled: self.enabled,
        }
    }
}
