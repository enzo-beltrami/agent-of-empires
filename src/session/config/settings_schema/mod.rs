//! The settings schema: field descriptors emitted by `#[derive(SettingsSection)]`
//! that drive the TUI, web dashboard, server validation, and override merging.
//! See docs/development/adding-settings.md.

use serde::{Deserialize, Serialize};

mod merge;
mod policy;
mod registry;
mod resolved;
mod validate;

pub use merge::{apply_changed_leaves, clear_path, merge_json};
pub use policy::{strip_local_only, validate_patch, validate_patch_with, PatchRejection, Scope};
pub use registry::{descriptor, schema, schema_ref, section_in_schema};
pub use resolved::{resolve, resolve_all, Candidate, ResolvedSetting, SettingSource};
pub use validate::{validate_value, ValidationError};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum WidgetKind {
    Toggle,
    Text {
        #[serde(default)]
        multiline: bool,
        #[serde(default)]
        mono: bool,
    },
    /// Clearing it stores null.
    OptionalText {
        #[serde(default)]
        mono: bool,
    },
    /// Bounds are advisory; [`ValidationKind`] is the server gate.
    Number {
        #[serde(skip_serializing_if = "Option::is_none")]
        min: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        max: Option<i64>,
    },
    Slider {
        min: i64,
        max: i64,
        step: i64,
    },
    Select {
        options: Vec<SelectOption>,
    },
    List,
    /// A bespoke widget registered under `id` on both the web and TUI.
    Custom {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

impl SelectOption {
    pub fn new(value: &str, label: &str) -> Self {
        Self {
            value: value.to_string(),
            label: label.to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "policy", rename_all = "snake_case")]
pub enum WebWritePolicy {
    Allow,
    /// Needs passphrase elevation.
    RequiresElevation {
        reason: String,
    },
    /// A host execution surface the server never accepts from the web.
    LocalOnly {
        reason: String,
    },
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum RepoPolicy {
    Allow,
    /// Default, so an undescribed field fails closed.
    #[default]
    Deny,
}

/// Server-authoritative validation applied before a value is merged.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "snake_case")]
pub enum ValidationKind {
    None,
    /// Inclusive bounds.
    RangeU64 {
        min: u64,
        max: Option<u64>,
    },
    /// Non-empty after trimming.
    NonEmptyString,
    /// Docker memory-limit grammar (`512m`, `2g`); empty allowed.
    MemoryLimit,
    /// `host:container[:options]` entries.
    VolumeList,
    /// Bare `KEY` or `KEY=VALUE` entries.
    EnvList,
    /// `host:container` numeric port entries.
    PortMappingList,
    CapabilityList,
    SecurityOptList,
    /// Empty, `none`, `bridge`, or a named network; `host` would defeat isolation.
    Network,
    /// A systemd slice unit name ending in `.slice`.
    SystemdSlice,
}

/// One configurable field, emitted by the `SettingsSection` derive.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDescriptor {
    /// The `[section]` table in `config.toml` and the profile override key.
    pub section: String,
    pub field: String,
    /// TUI tab.
    pub category: String,
    pub label: String,
    pub description: String,
    /// One user-facing line for the row; the doc-comment `description` stays for developers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    pub widget: WidgetKind,
    pub web_write: WebWritePolicy,
    #[serde(skip)]
    pub repo_policy: RepoPolicy,
    /// `false` for global-only fields.
    pub profile_overridable: bool,
    pub validation: ValidationKind,
    /// Shown under an "Advanced" fold on both surfaces.
    #[serde(default)]
    pub advanced: bool,
}

impl FieldDescriptor {
    /// `section.field`, the stable id in the web payload.
    pub fn path(&self) -> String {
        format!("{}.{}", self.section, self.field)
    }
}
