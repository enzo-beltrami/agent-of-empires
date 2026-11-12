//! The compiled-in plugins and their enabled state from `config.toml`.

use crate::session::Config;

pub struct BuiltinPlugin {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
}

pub static BUILTINS: &[BuiltinPlugin] = &[
    #[cfg(feature = "web")]
    BuiltinPlugin {
        id: super::WEB_PLUGIN_ID,
        name: "Web Dashboard",
        version: "1.0.0",
        description: "The aoe serve web dashboard. While disabled, aoe serve refuses to start.",
    },
];

pub struct LoadedPlugin {
    pub builtin: &'static BuiltinPlugin,
    pub enabled: bool,
}

impl LoadedPlugin {
    pub fn id(&self) -> &str {
        self.builtin.id
    }
}

pub struct PluginRegistry {
    plugins: Vec<LoadedPlugin>,
}

impl PluginRegistry {
    pub fn load(config: &Config) -> Self {
        let plugins = BUILTINS
            .iter()
            .map(|builtin| LoadedPlugin {
                builtin,
                enabled: config
                    .plugins
                    .get(builtin.id)
                    .map(|p| p.enabled)
                    .unwrap_or(true),
            })
            .collect();
        Self { plugins }
    }

    pub fn all(&self) -> &[LoadedPlugin] {
        &self.plugins
    }

    pub fn get(&self, plugin_id: &str) -> Option<&LoadedPlugin> {
        self.plugins.iter().find(|p| p.id() == plugin_id)
    }
}
