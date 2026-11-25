//! The `models` session field and `session/set_model`: the model channel that
//! predates config options and is still the only one some agents (Kiro)
//! advertise. The schema no longer carries it, so aoe reads it from the raw
//! response and presents it as a `model` config option under a reserved id;
//! the picker, defaults and resets then need no second path.

use agent_client_protocol::v1::schema::{
    SessionConfigKind, SessionConfigOption, SessionConfigOptionCategory, SessionConfigSelectOption,
    SessionConfigValueId, SessionId,
};
use agent_client_protocol::{JsonRpcMessage, JsonRpcRequest, JsonRpcResponse, UntypedMessage};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};

/// Reserved so it cannot collide with an id the agent chose.
pub(super) const SESSION_MODEL_CONFIG_ID: &str = "aoe_session_model";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SessionModelState {
    current_model_id: String,
    available_models: Vec<ModelInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelInfo {
    model_id: String,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

impl SessionModelState {
    fn config_option(&self) -> SessionConfigOption {
        let choices: Vec<_> = self
            .available_models
            .iter()
            .map(|m| {
                SessionConfigSelectOption::new(m.model_id.clone(), m.name.clone())
                    .description(m.description.clone())
            })
            .collect();
        SessionConfigOption::select(
            SESSION_MODEL_CONFIG_ID,
            "Model",
            self.current_model_id.clone(),
            choices,
        )
        .category(SessionConfigOptionCategory::Model)
    }
}

/// `options` plus `models` as a model select. A real `model` config option
/// wins, as the spec makes config options authoritative.
pub(super) fn merge_session_models(
    options: Option<Vec<SessionConfigOption>>,
    models: Option<&SessionModelState>,
) -> Option<Vec<SessionConfigOption>> {
    let Some(models) = models else {
        return options;
    };
    let mut options = options.unwrap_or_default();
    if !options
        .iter()
        .any(|o| o.category == Some(SessionConfigOptionCategory::Model))
    {
        options.push(models.config_option());
    }
    Some(options)
}

/// `options` with the session-model option switched to `value`; `None` when
/// `options` carries no session-model option.
pub(super) fn with_session_model_value(
    options: &[SessionConfigOption],
    value: &str,
) -> Option<Vec<SessionConfigOption>> {
    let mut options = options.to_vec();
    let SessionConfigKind::Select(select) = &mut options
        .iter_mut()
        .find(|o| o.id.0.as_ref() == SESSION_MODEL_CONFIG_ID)?
        .kind
    else {
        return None;
    };
    select.current_value = SessionConfigValueId::new(value.to_string());
    Some(options)
}

#[derive(Debug, Clone, Serialize, Deserialize, agent_client_protocol::JsonRpcRequest)]
#[request(method = "session/set_model", response = serde_json::Value)]
#[serde(rename_all = "camelCase")]
pub(super) struct SetSessionModelRequest {
    session_id: SessionId,
    model_id: String,
}

impl SetSessionModelRequest {
    pub(super) fn new(session_id: SessionId, model_id: String) -> Self {
        Self {
            session_id,
            model_id,
        }
    }
}

/// A session response with the `models` field the typed schema drops.
#[derive(Debug, Clone)]
pub(super) struct WithModels<R> {
    pub(super) response: R,
    pub(super) models: Option<SessionModelState>,
}

/// Splits `models` off a raw response. A malformed value is ignored, leaving
/// the session on config options alone.
fn split_models(mut value: serde_json::Value) -> (serde_json::Value, Option<SessionModelState>) {
    let models = value
        .as_object_mut()
        .and_then(|fields| fields.remove("models"))
        .and_then(|raw| serde_json::from_value(raw).ok());
    (value, models)
}

impl<'de, R: DeserializeOwned> Deserialize<'de> for WithModels<R> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (value, models) = split_models(serde_json::Value::deserialize(deserializer)?);
        let response = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(Self { response, models })
    }
}

impl<R: JsonRpcResponse> JsonRpcResponse for WithModels<R> {
    fn into_json(self, method: &str) -> Result<serde_json::Value, agent_client_protocol::Error> {
        let mut value = self.response.into_json(method)?;
        if let (Some(fields), Some(models)) = (value.as_object_mut(), self.models) {
            fields.insert("models".into(), serde_json::to_value(models)?);
        }
        Ok(value)
    }

    fn from_json(
        method: &str,
        value: serde_json::Value,
    ) -> Result<Self, agent_client_protocol::Error> {
        let (value, models) = split_models(value);
        Ok(Self {
            response: R::from_json(method, value)?,
            models,
        })
    }
}

/// Sends `Req` and keeps the `models` of its response.
#[derive(Debug, Clone, Serialize)]
#[serde(transparent)]
pub(super) struct ModelAware<Req>(pub(super) Req);

impl<Req: JsonRpcRequest> JsonRpcMessage for ModelAware<Req> {
    fn matches_method(method: &str) -> bool {
        Req::matches_method(method)
    }

    fn method(&self) -> &str {
        self.0.method()
    }

    fn to_untyped_message(&self) -> Result<UntypedMessage, agent_client_protocol::Error> {
        self.0.to_untyped_message()
    }

    fn parse_message(
        method: &str,
        params: &serde_json::Value,
    ) -> Result<Self, agent_client_protocol::Error> {
        Req::parse_message(method, params).map(Self)
    }
}

impl<Req: JsonRpcRequest> JsonRpcRequest for ModelAware<Req> {
    type Response = WithModels<Req::Response>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::v1::schema::NewSessionResponse;

    fn kiro_new_session() -> serde_json::Value {
        serde_json::json!({
            "sessionId": "s1",
            "models": {
                "currentModelId": "auto",
                "availableModels": [
                    {"modelId": "auto", "name": "auto", "description": "Chosen by task"},
                    {"modelId": "claude-opus-5", "name": "claude-opus-5"}
                ]
            }
        })
    }

    fn select(option: &SessionConfigOption) -> (String, Vec<String>) {
        let SessionConfigKind::Select(select) = &option.kind else {
            panic!("expected a select");
        };
        let agent_client_protocol::v1::schema::SessionConfigSelectOptions::Ungrouped(choices) =
            &select.options
        else {
            panic!("expected ungrouped choices");
        };
        (
            select.current_value.0.to_string(),
            choices.iter().map(|c| c.value.0.to_string()).collect(),
        )
    }

    #[test]
    fn models_become_a_switchable_model_option() {
        // The runner path deserializes with serde, the stdio path through
        // the SDK; both must keep `models`.
        let via_serde: WithModels<NewSessionResponse> =
            serde_json::from_value(kiro_new_session()).unwrap();
        let via_sdk =
            WithModels::<NewSessionResponse>::from_json("session/new", kiro_new_session()).unwrap();
        assert_eq!(via_sdk.response.session_id.0.as_ref(), "s1");
        assert!(via_sdk.models.is_some());

        let options = merge_session_models(None, via_serde.models.as_ref()).unwrap();
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].id.0.as_ref(), SESSION_MODEL_CONFIG_ID);
        assert_eq!(
            options[0].category,
            Some(SessionConfigOptionCategory::Model)
        );
        assert_eq!(
            select(&options[0]),
            ("auto".into(), vec!["auto".into(), "claude-opus-5".into()])
        );

        let switched = with_session_model_value(&options, "claude-opus-5").unwrap();
        assert_eq!(select(&switched[0]).0, "claude-opus-5");
    }

    #[test]
    fn config_option_model_wins_and_bad_models_are_ignored() {
        let real = SessionConfigOption::select(
            "model",
            "Model",
            "opus",
            vec![SessionConfigSelectOption::new("opus", "Opus")],
        )
        .category(SessionConfigOptionCategory::Model);
        let models = WithModels::<NewSessionResponse>::from_json("session/new", kiro_new_session())
            .unwrap()
            .models;
        let merged = merge_session_models(Some(vec![real]), models.as_ref()).unwrap();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].id.0.as_ref(), "model");
        assert!(with_session_model_value(&merged, "x").is_none());

        let malformed = serde_json::json!({"sessionId": "s1", "models": {"bogus": true}});
        let parsed = WithModels::<NewSessionResponse>::from_json("session/new", malformed).unwrap();
        assert!(parsed.models.is_none());
        assert!(merge_session_models(None, parsed.models.as_ref()).is_none());
    }
}
