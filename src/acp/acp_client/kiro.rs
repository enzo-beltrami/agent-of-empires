//! Kiro's vendor notifications, translated to their standard ACP form.

use agent_client_protocol::v1::schema::SessionNotification;
use serde::Deserialize;

pub(super) const COMMANDS_AVAILABLE_METHOD: &str = "_kiro.dev/commands/available";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandsAvailable {
    session_id: String,
    commands: Vec<KiroCommand>,
}

#[derive(Deserialize)]
struct KiroCommand {
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    meta: KiroCommandMeta,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct KiroCommandMeta {
    /// Handled by Kiro's own TUI, so a prompt cannot run it.
    local: bool,
    hidden: bool,
    hint: String,
    subcommands: Vec<String>,
}

/// `_kiro.dev/commands/available` as an `available_commands_update`. Kiro
/// runs a slash command sent as prompt text, so only the list needs mapping.
pub(super) fn commands_update(
    params: &serde_json::Value,
) -> agent_client_protocol::Result<SessionNotification> {
    let parsed: CommandsAvailable = serde_json::from_value(params.clone())?;
    let commands: Vec<_> = parsed
        .commands
        .into_iter()
        .filter(|c| !c.meta.local && !c.meta.hidden)
        .map(|c| {
            let hint = if c.meta.hint.is_empty() {
                c.meta.subcommands.join(" | ")
            } else {
                c.meta.hint
            };
            let mut command = serde_json::json!({
                "name": c.name.trim_start_matches('/'),
                "description": c.description,
            });
            if !hint.is_empty() {
                command["input"] = serde_json::json!({ "hint": hint });
            }
            command
        })
        .collect();
    Ok(serde_json::from_value(serde_json::json!({
        "sessionId": parsed.session_id,
        "update": {
            "sessionUpdate": "available_commands_update",
            "availableCommands": commands,
        },
    }))?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::v1::schema::{AvailableCommandInput, SessionUpdate};

    #[test]
    fn commands_map_to_standard_update() {
        let params = serde_json::json!({
            "sessionId": "s1",
            "commands": [
                {"name": "/compact", "description": "Compact conversation history"},
                {"name": "/context", "description": "Manage context",
                 "meta": {"inputType": "panel", "hint": "add <path>", "subcommands": ["add"]}},
                {"name": "/agent", "description": "Select an agent",
                 "meta": {"hint": "", "subcommands": ["create", "swap"]}},
                {"name": "/quit", "description": "Quit", "meta": {"local": true}},
                {"name": "/stats", "description": "Timings", "meta": {"hidden": true}}
            ]
        });
        let notification = commands_update(&params).unwrap();
        assert_eq!(notification.session_id.0.as_ref(), "s1");
        let SessionUpdate::AvailableCommandsUpdate(update) = notification.update else {
            panic!("expected an available commands update");
        };
        let summary: Vec<_> = update
            .available_commands
            .iter()
            .map(|c| {
                let hint = match &c.input {
                    Some(AvailableCommandInput::Unstructured(input)) => Some(input.hint.as_str()),
                    _ => None,
                };
                (c.name.as_str(), hint)
            })
            .collect();
        assert_eq!(
            summary,
            [
                ("compact", None),
                ("context", Some("add <path>")),
                ("agent", Some("create | swap")),
            ]
        );
    }
}
