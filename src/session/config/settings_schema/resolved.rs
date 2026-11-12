//! Settings resolution with provenance, for `aoe settings explain` and `GET /api/settings/resolved`.
//!
//! A key resolves to the user value, else the schema default.

use serde::Serialize;
use serde_json::Value;

use super::{schema_ref, FieldDescriptor};

/// Where a resolved value came from.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SettingSource {
    User,
    SchemaDefault,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Candidate {
    pub source: SettingSource,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResolvedSetting {
    /// `section.field`.
    pub key: String,
    pub value: Value,
    pub source: SettingSource,
    /// Highest precedence first.
    pub candidates: Vec<Candidate>,
}

fn split_key(key: &str) -> Option<(&str, &str)> {
    key.rsplit_once('.')
}

/// `None` for an unknown key.
pub fn resolve(key: &str) -> Option<ResolvedSetting> {
    let cfg = serde_json::to_value(crate::session::Config::load_or_warn()).ok()?;
    let default_cfg = serde_json::to_value(crate::session::Config::default()).ok()?;
    resolve_with(key, &cfg, &default_cfg, schema_ref())
}

pub fn resolve_all() -> Vec<ResolvedSetting> {
    let cfg = serde_json::to_value(crate::session::Config::load_or_warn()).unwrap_or(Value::Null);
    let default_cfg =
        serde_json::to_value(crate::session::Config::default()).unwrap_or(Value::Null);
    let schema = schema_ref();
    schema
        .iter()
        .filter_map(|d| {
            resolve_with(
                &format!("{}.{}", d.section, d.field),
                &cfg,
                &default_cfg,
                schema,
            )
        })
        .collect()
}

fn resolve_with(
    key: &str,
    cfg: &Value,
    default_cfg: &Value,
    schema: &[FieldDescriptor],
) -> Option<ResolvedSetting> {
    let (section, field) = split_key(key)?;
    if !schema
        .iter()
        .any(|d| d.section == section && d.field == field)
    {
        return None;
    }
    Some(resolve_core(key, section, field, cfg, default_cfg))
}

fn resolve_core(
    key: &str,
    section: &str,
    field: &str,
    cfg: &Value,
    default_cfg: &Value,
) -> ResolvedSetting {
    let schema_default = default_cfg
        .get(section)
        .and_then(|s| s.get(field))
        .cloned()
        .unwrap_or(Value::Null);
    let stored = cfg.get(section).and_then(|s| s.get(field)).cloned();

    let user = stored.filter(|v| *v != schema_default);

    let mut candidates = Vec::new();
    if let Some(v) = &user {
        candidates.push(Candidate {
            source: SettingSource::User,
            value: v.clone(),
        });
    }

    candidates.push(Candidate {
        source: SettingSource::SchemaDefault,
        value: schema_default.clone(),
    });

    let (source, value) = match user {
        Some(v) => (SettingSource::User, v),
        None => (SettingSource::SchemaDefault, schema_default),
    };
    ResolvedSetting {
        key: key.to_string(),
        value,
        source,
        candidates,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_key_takes_last_segment() {
        assert_eq!(
            split_key("acp.default_agent"),
            Some(("acp", "default_agent"))
        );
        assert_eq!(split_key("nodot"), None);
    }

    #[test]
    fn unknown_key_resolves_to_none() {
        assert!(resolve("acp.totally_made_up").is_none());
        assert!(resolve("nodot").is_none());
    }

    #[test]
    fn core_field_falls_back_to_schema_default() {
        let _app = crate::session::test_support::isolate_app_dir();
        let r = resolve("acp.default_agent").expect("known core key");
        assert_eq!(r.source, SettingSource::SchemaDefault);
        assert_eq!(
            r.candidates.last().unwrap().source,
            SettingSource::SchemaDefault
        );
    }
}
