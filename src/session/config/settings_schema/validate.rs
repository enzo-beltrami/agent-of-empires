//! Server-authoritative value validation.
//!
//! The web min/max in [`super::WidgetKind`] is advisory UI metadata; this is
//! the gate the server applies before merging an incoming value. Invalid input
//! is rejected (no silent clamping), so a client always knows whether its value
//! was accepted.

use serde_json::Value;

use super::ValidationKind;

/// A value failed validation for a field. Carries a human-readable reason the
/// server surfaces to the client (HTTP 400).
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub reason: String,
}

impl ValidationError {
    fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

/// Validate `value` against `kind`. The value is the raw JSON the client sent
/// for one field (already typed by serde_json, e.g. a number or string).
pub fn validate_value(kind: &ValidationKind, value: &Value) -> Result<(), ValidationError> {
    match kind {
        ValidationKind::None => Ok(()),
        ValidationKind::RangeU64 { min, max } => {
            let n = value
                .as_u64()
                .ok_or_else(|| ValidationError::new("expected a non-negative integer"))?;
            if n < *min {
                return Err(ValidationError::new(format!("must be at least {min}")));
            }
            if let Some(max) = max {
                if n > *max {
                    return Err(ValidationError::new(format!("must be at most {max}")));
                }
            }
            Ok(())
        }
        ValidationKind::NonEmptyString => {
            let s = value
                .as_str()
                .ok_or_else(|| ValidationError::new("expected a string"))?;
            if s.trim().is_empty() {
                return Err(ValidationError::new("must not be empty"));
            }
            Ok(())
        }
        ValidationKind::MemoryLimit => {
            let s = value
                .as_str()
                .ok_or_else(|| ValidationError::new("expected a string"))?;
            crate::session::validate_memory_limit(s).map_err(ValidationError::new)
        }
        ValidationKind::VolumeList => {
            validate_string_list(value, crate::session::validate_volume_format)
        }
        ValidationKind::EnvList => validate_string_list(value, crate::session::validate_env_format),
        ValidationKind::PortMappingList => {
            validate_string_list(value, crate::session::validate_port_mapping_format)
        }
        ValidationKind::CapabilityList => {
            validate_string_list(value, crate::session::validate_capability_format)
        }
        ValidationKind::SecurityOptList => {
            validate_string_list(value, crate::session::validate_security_opt_format)
        }
        ValidationKind::Network => {
            let s = value
                .as_str()
                .ok_or_else(|| ValidationError::new("expected a string"))?;
            crate::session::validate_network_format(s).map_err(ValidationError::new)
        }
        ValidationKind::SystemdSlice => {
            let s = value
                .as_str()
                .ok_or_else(|| ValidationError::new("expected a string"))?;
            crate::session::validate_systemd_slice(s).map_err(ValidationError::new)
        }
    }
}

/// Validate that `value` is a JSON array of strings and each passes `check`.
fn validate_string_list(
    value: &Value,
    check: impl Fn(&str) -> Result<(), String>,
) -> Result<(), ValidationError> {
    let arr = value
        .as_array()
        .ok_or_else(|| ValidationError::new("expected a list"))?;
    for entry in arr {
        let s = entry
            .as_str()
            .ok_or_else(|| ValidationError::new("list entries must be strings"))?;
        check(s).map_err(ValidationError::new)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn validate_value_accepts_and_rejects_by_kind() {
        use ValidationKind as K;
        let bounded = K::RangeU64 {
            min: 1,
            max: Some(128),
        };
        let unbounded = K::RangeU64 { min: 0, max: None };
        let cases = [
            (&bounded, json!(0), false),
            (&bounded, json!(1), true),
            (&bounded, json!(128), true),
            (&bounded, json!(129), false),
            (&unbounded, json!("nope"), false),
            (&unbounded, json!(-1), false),
            (&K::NonEmptyString, json!("  "), false),
            (&K::NonEmptyString, json!("x"), true),
            (&K::Network, json!("egress-proxy"), true),
            (&K::Network, json!("host"), false),
            (&K::Network, json!(42), false),
            (&K::SystemdSlice, json!("background-aoe.slice"), true),
            (&K::SystemdSlice, json!("aoe.slice"), true),
            (&K::SystemdSlice, json!(".slice"), false),
            (&K::SystemdSlice, json!("aoe"), false),
            (&K::SystemdSlice, json!("a b.slice"), false),
            (&K::SystemdSlice, json!("x.service"), false),
            (
                &K::SystemdSlice,
                json!("background-aoe-{profile}.slice"),
                true,
            ),
            (&K::SystemdSlice, json!("{profile}"), false),
            (&K::SystemdSlice, json!("a-{profile}-b.slice"), true),
            (
                &K::SystemdSlice,
                json!("aoe-{profile}-{session}.slice"),
                true,
            ),
            (&K::SystemdSlice, json!("{session}"), false),
            (&K::SystemdSlice, json!("{prof}.slice"), false),
            (&K::SystemdSlice, json!("-a.slice"), false),
            (&K::SystemdSlice, json!("a-.slice"), false),
            (&K::SystemdSlice, json!("a--b.slice"), false),
            (&K::SystemdSlice, json!("-{profile}.slice"), false),
            (
                &K::SystemdSlice,
                json!(format!("{}.slice", "a".repeat(250))),
                false,
            ),
            (
                &K::SystemdSlice,
                json!(format!("{}.slice", "a".repeat(249))),
                true,
            ),
            (&K::MemoryLimit, json!("512m"), true),
            (&K::MemoryLimit, json!(""), true),
            (&K::MemoryLimit, json!("512mb"), false),
            (&K::VolumeList, json!(["/h:/c"]), true),
            (&K::VolumeList, json!(["bad"]), false),
            (
                &K::EnvList,
                json!(["KEY", "KEY=value", "_K=v", "A1=b"]),
                true,
            ),
            (&K::EnvList, json!(["1BAD=v"]), false),
            (&K::EnvList, json!(["has space"]), false),
            (&K::EnvList, json!("notalist"), false),
            (&K::PortMappingList, json!(["3000:3000", "8080:80"]), true),
            (&K::PortMappingList, json!(["3000"]), false),
            (&K::PortMappingList, json!(["a:b"]), false),
        ];
        for (kind, value, ok) in cases {
            assert_eq!(validate_value(kind, &value).is_ok(), ok, "{kind:?} {value}");
        }
    }
}
