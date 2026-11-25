//! Dashboard sign-in for OAuth MCP servers, and the proxy that adds the stored token to agents'
//! requests. The callback and proxy sit outside the auth layers: the browser returning from the
//! authorization server carries no `SameSite=Strict` cookie, so the callback is authorized by
//! its single-use `state`, and the proxy by the secret only AoE-launched agents hold.

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use axum::{
    body::Body,
    extract::{Path, Query, Request, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::Deserialize;

use super::api::api_error;
use super::AppState;
use crate::session::mcp::oauth;

const MAX_PROXY_BODY: usize = 4 * 1024 * 1024;
const FORWARDED_REQUEST_HEADERS: &[&str] = &[
    "accept",
    "content-type",
    "last-event-id",
    "mcp-protocol-version",
    "mcp-session-id",
];
const FORWARDED_RESPONSE_HEADERS: &[&str] = &["cache-control", "content-type", "mcp-session-id"];

#[derive(Deserialize)]
pub struct ServerUrl {
    url: String,
}

fn valid_server_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| matches!(u.scheme(), "http" | "https"))
}

/// The dashboard origin the browser is on, which the authorization server redirects back to.
fn dashboard_origin(headers: &HeaderMap) -> Option<String> {
    let value = |name| headers.get(name).and_then(|v| v.to_str().ok());
    value(header::ORIGIN)
        .filter(|origin| *origin != "null")
        .map(str::to_string)
        .or_else(|| value(header::HOST).map(|host| format!("http://{host}")))
}

/// `POST /api/mcp/oauth/start {url}`: the authorization URL for the browser to open.
pub async fn start(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(body): Json<ServerUrl>,
) -> Response {
    if let Some(resp) = super::api::cityhall_block(&state) {
        return resp;
    }
    if !valid_server_url(&body.url) {
        return api_error(StatusCode::BAD_REQUEST, "invalid_url", "not an http(s) URL");
    }
    let Some(origin) = dashboard_origin(&headers) else {
        return api_error(
            StatusCode::BAD_REQUEST,
            "no_origin",
            "request has no origin",
        );
    };
    let redirect = format!("{origin}{}", oauth::CALLBACK_PATH);
    match oauth::start_sign_in(&body.url, &redirect).await {
        Ok(url) => Json(serde_json::json!({ "authorizationUrl": url })).into_response(),
        Err(e) => {
            tracing::warn!(target: "server.mcp_oauth", error = %format!("{e:#}"), "sign-in did not start");
            api_error(
                StatusCode::BAD_GATEWAY,
                "sign_in_failed",
                format!("Could not start sign-in: {e:#}"),
            )
        }
    }
}

/// `POST /api/mcp/oauth/sign-out {url}`.
pub async fn sign_out(State(state): State<Arc<AppState>>, Json(body): Json<ServerUrl>) -> Response {
    if let Some(resp) = super::api::cityhall_block(&state) {
        return resp;
    }
    match oauth::sign_out(&body.url).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "sign_out_failed",
            format!("{e:#}"),
        ),
    }
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    iss: Option<String>,
}

fn page(heading: &str, detail: &str) -> Html<String> {
    Html(format!(
        "<!doctype html><meta charset=\"utf-8\"><title>Agent of Empires</title>\
         <body style=\"font-family:system-ui;margin:3rem\"><h2>{heading}</h2><p>{detail}</p>"
    ))
}

/// `GET /api/mcp/oauth/callback`: where the authorization server sends the browser back.
pub async fn callback(Query(query): Query<CallbackQuery>) -> Response {
    let (Some(code), Some(state)) = (query.code, query.state) else {
        return (
            StatusCode::BAD_REQUEST,
            page(
                "Sign-in did not finish",
                "The server sent no authorization code. Close this tab and try again.",
            ),
        )
            .into_response();
    };
    match oauth::finish_sign_in(&code, &state, query.iss.as_deref()).await {
        Ok(url) => {
            tracing::info!(target: "server.mcp_oauth", key = %oauth::proxy_key(&url), "signed in");
            page(
                "Signed in",
                "Agent of Empires can now use this MCP server. You can close this tab.",
            )
            .into_response()
        }
        Err(e) => {
            tracing::warn!(target: "server.mcp_oauth", error = %format!("{e:#}"), "sign-in callback failed");
            (
                StatusCode::BAD_REQUEST,
                page(
                    "Sign-in failed",
                    "Close this tab and start the sign-in again from the dashboard.",
                ),
            )
                .into_response()
        }
    }
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .expect("reqwest client")
    })
}

fn proxy_error(status: StatusCode, message: &str) -> Response {
    api_error(status, "mcp_proxy", message)
}

/// `/mcp-proxy/{key}`: forwards an agent's MCP request with the server's current token.
pub async fn proxy(Path(key): Path<String>, request: Request) -> Response {
    let presented = request
        .headers()
        .get(oauth::PROXY_HEADER)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let authorized =
        oauth::proxy_secret().is_ok_and(|secret| super::auth::constant_time_eq(presented, secret));
    if !authorized {
        return proxy_error(StatusCode::FORBIDDEN, "missing or wrong proxy secret");
    }
    let lookup = key.clone();
    let Ok(Some(url)) = tokio::task::spawn_blocking(move || oauth::url_for_key(&lookup)).await
    else {
        return proxy_error(StatusCode::NOT_FOUND, "not signed in to this server");
    };
    let (parts, body) = request.into_parts();
    let Ok(body) = axum::body::to_bytes(body, MAX_PROXY_BODY).await else {
        return proxy_error(StatusCode::PAYLOAD_TOO_LARGE, "request body too large");
    };
    let target = match parts.uri.query() {
        Some(query) => format!("{url}?{query}"),
        None => url.clone(),
    };
    let send = |token: String| {
        let mut upstream = client()
            .request(parts.method.clone(), &target)
            .bearer_auth(token)
            .body(body.clone());
        for name in FORWARDED_REQUEST_HEADERS {
            if let Some(value) = parts.headers.get(*name) {
                upstream = upstream.header(*name, value.clone());
            }
        }
        upstream.send()
    };

    let token = match oauth::access_token(&url).await {
        Ok(token) => token,
        Err(e) => return needs_sign_in(&key, &e),
    };
    let mut response = match send(token).await {
        Ok(response) => response,
        Err(e) => {
            tracing::warn!(target: "server.mcp_oauth", key, error = %e, "upstream unreachable");
            return proxy_error(StatusCode::BAD_GATEWAY, "MCP server unreachable");
        }
    };
    if response.status() == StatusCode::UNAUTHORIZED {
        let token = match oauth::refreshed_token(&url).await {
            Ok(token) => token,
            Err(e) => return needs_sign_in(&key, &e),
        };
        response = match send(token).await {
            Ok(response) => response,
            Err(_) => return proxy_error(StatusCode::BAD_GATEWAY, "MCP server unreachable"),
        };
    }

    let mut builder = Response::builder().status(response.status());
    for name in FORWARDED_RESPONSE_HEADERS {
        if let Some(value) = response.headers().get(*name) {
            builder = builder.header(*name, value.clone());
        }
    }
    builder
        .body(Body::from_stream(response.bytes_stream()))
        .unwrap_or_else(|_| proxy_error(StatusCode::BAD_GATEWAY, "bad upstream response"))
}

fn needs_sign_in(key: &str, error: &rmcp::transport::AuthError) -> Response {
    tracing::warn!(target: "server.mcp_oauth", key, %error, "token unavailable; sign in again from the dashboard");
    proxy_error(
        StatusCode::UNAUTHORIZED,
        "sign in to this MCP server again from the AoE dashboard",
    )
}

#[cfg(test)]
mod tests {
    use axum::http::{Request, StatusCode};
    use serial_test::serial;
    use tower::ServiceExt;

    use crate::server::test_support;
    use crate::session::mcp::oauth;

    #[tokio::test]
    #[serial]
    async fn callback_and_proxy_skip_dashboard_auth_but_authorize_themselves() {
        let _home = crate::session::test_support::isolate_app_dir();
        let state = test_support::build_test_app_state_with_policy(
            vec![],
            vec!["localhost".into()],
            vec![],
            Some("dashboard-token".into()),
        );
        let secret = oauth::proxy_secret().unwrap().to_string();
        let send = |uri: &str, secret: Option<&str>| {
            let mut request = Request::builder()
                .uri(uri)
                .header("host", "localhost")
                .extension(axum::extract::ConnectInfo(
                    "127.0.0.1:5555".parse::<std::net::SocketAddr>().unwrap(),
                ));
            if let Some(secret) = secret {
                request = request.header(oauth::PROXY_HEADER, secret);
            }
            test_support::build_router_for_test(state.clone())
                .oneshot(request.body(axum::body::Body::empty()).unwrap())
        };
        for (uri, presented, expected) in [
            ("/api/mcp/servers", None, StatusCode::UNAUTHORIZED),
            (
                "/api/mcp/oauth/callback?code=c&state=forged",
                None,
                StatusCode::BAD_REQUEST,
            ),
            ("/mcp-proxy/abc", None, StatusCode::FORBIDDEN),
            ("/mcp-proxy/abc", Some("wrong"), StatusCode::FORBIDDEN),
            (
                "/mcp-proxy/abc",
                Some(secret.as_str()),
                StatusCode::NOT_FOUND,
            ),
        ] {
            let response = send(uri, presented).await.unwrap();
            assert_eq!(response.status(), expected, "{uri} with {presented:?}");
        }
    }
}
