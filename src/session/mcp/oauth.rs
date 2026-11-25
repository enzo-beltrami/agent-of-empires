//! OAuth sign-in for HTTP MCP servers. Tokens live in `<app_dir>/mcp_oauth.json`, keyed by
//! server URL. Host sessions reach a signed-in server through the daemon's `/mcp-proxy/{key}`,
//! which adds a current token to every request, so a re-sign-in reaches running agents.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use agent_client_protocol::v1::schema::{HttpHeader, McpServer};
use anyhow::{anyhow, Context};
use rmcp::transport::auth::{
    AuthError, AuthorizationManager, AuthorizationRequest, AuthorizationSession, CredentialStore,
    StoredCredentials,
};
use serde::Serialize;

const STORE_FILE: &str = "mcp_oauth.json";
const SECRET_FILE: &str = "mcp_proxy_secret";
/// Carries the proxy secret, so only the agents AoE configured can use the proxy.
pub const PROXY_HEADER: &str = "x-aoe-mcp-proxy";
pub const CALLBACK_PATH: &str = "/api/mcp/oauth/callback";
const PENDING_TTL: Duration = Duration::from_secs(15 * 60);

static STORE_LOCK: Mutex<()> = Mutex::new(());
static PROXY_BASE: OnceLock<String> = OnceLock::new();
static SECRET: OnceLock<String> = OnceLock::new();

type Credentials = BTreeMap<String, StoredCredentials>;

fn store_path() -> anyhow::Result<PathBuf> {
    Ok(crate::session::get_app_dir()?.join(STORE_FILE))
}

fn load_all() -> anyhow::Result<Credentials> {
    let path = store_path()?;
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).with_context(|| format!("{}", path.display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Credentials::new()),
        Err(e) => Err(e.into()),
    }
}

fn write_private(path: &std::path::Path, bytes: &[u8]) -> anyhow::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

fn update(edit: impl FnOnce(&mut Credentials)) -> anyhow::Result<()> {
    let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut all = load_all()?;
    edit(&mut all);
    write_private(&store_path()?, &serde_json::to_vec_pretty(&all)?)
}

fn stored(url: &str) -> Option<StoredCredentials> {
    let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    match load_all() {
        Ok(mut all) => all.remove(url),
        Err(e) => {
            tracing::warn!(target: "session.mcp_oauth", error = %format!("{e:#}"), "token store unreadable");
            None
        }
    }
}

/// One server's entry in the shared token file.
struct UrlStore(String);

#[async_trait::async_trait]
impl CredentialStore for UrlStore {
    async fn load(&self) -> Result<Option<StoredCredentials>, AuthError> {
        Ok(stored(&self.0))
    }

    async fn save(&self, credentials: StoredCredentials) -> Result<(), AuthError> {
        update(|all| {
            all.insert(self.0.clone(), credentials);
        })
        .map_err(store_error)
    }

    async fn clear(&self) -> Result<(), AuthError> {
        update(|all| {
            all.remove(&self.0);
        })
        .map_err(store_error)
    }
}

fn store_error(e: anyhow::Error) -> AuthError {
    AuthError::InternalError(format!("token store: {e:#}"))
}

/// The token fields AoE reads; `StoredCredentials` keeps them behind the `oauth2` types.
struct Token {
    access: String,
    expires_at: Option<u64>,
    refreshable: bool,
}

fn token(credentials: &StoredCredentials) -> Option<Token> {
    let value = serde_json::to_value(credentials.token_response.as_ref()?).ok()?;
    let access = value.get("access_token")?.as_str()?.to_string();
    let expires_at = value
        .get("expires_in")
        .and_then(|v| v.as_u64())
        .zip(credentials.token_received_at)
        .map(|(lifetime, received)| received + lifetime);
    let refreshable = value
        .get("refresh_token")
        .is_some_and(|v| v.as_str().is_some_and(|s| !s.is_empty()));
    Some(Token {
        access,
        expires_at,
        refreshable,
    })
}

fn now() -> u64 {
    chrono::Utc::now().timestamp().max(0) as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SignIn {
    SignedIn,
    /// The token expired and the server issued no refresh token.
    Expired,
}

fn sign_in_of(token: &Token, now: u64) -> SignIn {
    match token.expires_at {
        Some(at) if at <= now && !token.refreshable => SignIn::Expired,
        _ => SignIn::SignedIn,
    }
}

pub fn status(url: &str) -> Option<SignIn> {
    stored(url)
        .and_then(|c| token(&c))
        .map(|t| sign_in_of(&t, now()))
}

/// Path segment naming a signed-in server, so its URL never appears in agent configs or logs.
pub fn proxy_key(url: &str) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(url.as_bytes())[..12]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

pub fn url_for_key(key: &str) -> Option<String> {
    let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    load_all()
        .ok()?
        .into_keys()
        .find(|url| proxy_key(url) == key)
}

/// Where host agents reach the proxy: the daemon's own listener.
pub fn set_proxy_base(base: String) {
    let _ = PROXY_BASE.set(base);
}

/// Persisted so agents still hold a working header after a daemon restart reattaches them.
pub fn proxy_secret() -> anyhow::Result<&'static str> {
    if let Some(secret) = SECRET.get() {
        return Ok(secret);
    }
    let path = crate::session::get_app_dir()?.join(SECRET_FILE);
    let secret = match std::fs::read_to_string(&path) {
        Ok(s) if s.trim().len() >= 32 => s.trim().to_string(),
        _ => {
            use base64::Engine;
            use rand::RngExt;
            let mut bytes = [0u8; 32];
            rand::rng().fill(&mut bytes);
            let s = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes);
            write_private(&path, s.as_bytes())?;
            s
        }
    };
    Ok(SECRET.get_or_init(|| secret))
}

/// Routes signed-in HTTP servers through the proxy. A sandbox cannot reach the host listener,
/// so it gets the current token directly, as does an SSE server.
pub fn authorize_servers(servers: &mut [McpServer], sandboxed: bool) {
    let proxy = PROXY_BASE
        .get()
        .filter(|_| !sandboxed)
        .and_then(|base| proxy_secret().ok().map(|secret| (base, secret)));
    for server in servers {
        let (url, headers, http) = match server {
            McpServer::Http(http) => (&mut http.url, &mut http.headers, true),
            McpServer::Sse(sse) => (&mut sse.url, &mut sse.headers, false),
            _ => continue,
        };
        if headers
            .iter()
            .any(|h| h.name.eq_ignore_ascii_case("authorization"))
        {
            continue;
        }
        let Some(token) = stored(url).and_then(|c| token(&c)) else {
            continue;
        };
        match (proxy, http) {
            (Some((base, secret)), true) => {
                *url = format!("{base}/mcp-proxy/{}", proxy_key(url));
                headers.push(HttpHeader::new(PROXY_HEADER, secret));
            }
            _ => headers.push(HttpHeader::new(
                "Authorization",
                format!("Bearer {}", token.access),
            )),
        }
    }
}

fn managers() -> &'static tokio::sync::Mutex<HashMap<String, Arc<AuthorizationManager>>> {
    static MANAGERS: OnceLock<tokio::sync::Mutex<HashMap<String, Arc<AuthorizationManager>>>> =
        OnceLock::new();
    MANAGERS.get_or_init(Default::default)
}

async fn manager(url: &str) -> Result<Arc<AuthorizationManager>, AuthError> {
    let mut cache = managers().lock().await;
    if let Some(manager) = cache.get(url) {
        return Ok(manager.clone());
    }
    let mut manager = AuthorizationManager::new(url).await?;
    manager.set_credential_store(UrlStore(url.to_string()));
    if !manager.initialize_from_store().await? {
        return Err(AuthError::AuthorizationRequired);
    }
    let manager = Arc::new(manager);
    cache.insert(url.to_string(), manager.clone());
    Ok(manager)
}

async fn forget(url: &str) {
    managers().lock().await.remove(url);
}

/// The current token, refreshed first when it is about to expire.
pub async fn access_token(url: &str) -> Result<String, AuthError> {
    manager(url).await?.get_access_token().await
}

/// A new token after the server rejected the current one.
pub async fn refreshed_token(url: &str) -> Result<String, AuthError> {
    let manager = manager(url).await?;
    manager.refresh_token().await?;
    manager.get_access_token().await
}

struct Pending {
    url: String,
    session: AuthorizationSession,
    started: Instant,
}

fn pending() -> &'static tokio::sync::Mutex<HashMap<String, Pending>> {
    static PENDING: OnceLock<tokio::sync::Mutex<HashMap<String, Pending>>> = OnceLock::new();
    PENDING.get_or_init(Default::default)
}

/// Registers AoE with the server's authorization server when needed and returns the URL the
/// user opens to approve access. `redirect_uri` is the dashboard's callback.
pub async fn start_sign_in(url: &str, redirect_uri: &str) -> anyhow::Result<String> {
    let mut manager = AuthorizationManager::new(url).await?;
    manager.set_credential_store(UrlStore(url.to_string()));
    let metadata = manager.resolve_metadata().await?.metadata;
    manager.set_metadata(metadata);
    let request = AuthorizationRequest::new(redirect_uri).with_client_name("Agent of Empires");
    let session = AuthorizationSession::new(manager, request)
        .await
        .map_err(|(_, e)| e)?;
    let authorization_url = session.get_authorization_url().to_string();
    let state = reqwest::Url::parse(&authorization_url)?
        .query_pairs()
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .ok_or_else(|| anyhow!("authorization URL has no state"))?;
    let mut pending = pending().lock().await;
    pending.retain(|_, p| p.started.elapsed() < PENDING_TTL);
    pending.insert(
        state,
        Pending {
            url: url.to_string(),
            session,
            started: Instant::now(),
        },
    );
    Ok(authorization_url)
}

/// Exchanges the callback's code for a token and returns the server URL it is for.
pub async fn finish_sign_in(
    code: &str,
    state: &str,
    issuer: Option<&str>,
) -> anyhow::Result<String> {
    let Pending {
        url,
        session,
        started,
    } = pending()
        .lock()
        .await
        .remove(state)
        .ok_or_else(|| anyhow!("no sign-in is waiting for this callback"))?;
    if started.elapsed() >= PENDING_TTL {
        return Err(anyhow!("the sign-in expired; start it again"));
    }
    session
        .handle_callback_with_issuer(code, state, issuer)
        .await?;
    forget(&url).await;
    Ok(url)
}

pub async fn sign_out(url: &str) -> anyhow::Result<()> {
    update(|all| {
        all.remove(url);
    })?;
    forget(url).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::v1::schema::{McpServerHttp, McpServerStdio};
    use serial_test::serial;

    fn credentials(token: serde_json::Value, received: u64) -> StoredCredentials {
        StoredCredentials::new(
            "client".into(),
            Some(serde_json::from_value(token).unwrap()),
            vec![],
            Some(received),
        )
    }

    #[test]
    fn expiry_only_matters_without_a_refresh_token() {
        let at = |token| self::token(&credentials(token, 1_000)).unwrap();
        let plain =
            serde_json::json!({"access_token": "a", "token_type": "bearer", "expires_in": 60});
        let refreshable = serde_json::json!({"access_token": "a", "token_type": "bearer", "expires_in": 60, "refresh_token": "r"});
        let forever = serde_json::json!({"access_token": "a", "token_type": "bearer"});
        for (token, now, expected) in [
            (plain.clone(), 1_059, SignIn::SignedIn),
            (plain, 1_060, SignIn::Expired),
            (refreshable, 5_000, SignIn::SignedIn),
            (forever, 5_000, SignIn::SignedIn),
        ] {
            assert_eq!(
                sign_in_of(&at(token.clone()), now),
                expected,
                "{token} at {now}"
            );
        }
    }

    #[test]
    #[serial]
    fn signed_in_servers_get_a_token_and_others_are_untouched() {
        let _home = crate::session::test_support::isolate_app_dir();
        let signed_in = "https://mcp.example.com/v1/mcp";
        update(|all| {
            all.insert(
                signed_in.into(),
                credentials(
                    serde_json::json!({"access_token": "tok", "token_type": "bearer"}),
                    now(),
                ),
            );
        })
        .unwrap();
        let servers = || {
            vec![
                McpServer::Http(McpServerHttp::new("ci", signed_in)),
                McpServer::Http(
                    McpServerHttp::new("own", signed_in)
                        .headers(vec![HttpHeader::new("Authorization", "Bearer mine")]),
                ),
                McpServer::Http(McpServerHttp::new("other", "https://other.example.com/mcp")),
                McpServer::Stdio(McpServerStdio::new("local", "cmd")),
            ]
        };
        let headers = |server: &McpServer| match server {
            McpServer::Http(h) => (
                h.url.clone(),
                h.headers
                    .iter()
                    .map(|h| format!("{}: {}", h.name, h.value))
                    .collect::<Vec<_>>(),
            ),
            _ => (String::new(), vec![]),
        };

        let mut sandboxed = servers();
        authorize_servers(&mut sandboxed, true);
        assert_eq!(
            headers(&sandboxed[0]),
            (signed_in.into(), vec!["Authorization: Bearer tok".into()])
        );
        assert_eq!(headers(&sandboxed[1]).1, vec!["Authorization: Bearer mine"]);
        assert!(headers(&sandboxed[2]).1.is_empty());

        set_proxy_base("http://127.0.0.1:8080".into());
        let mut host = servers();
        authorize_servers(&mut host, false);
        let (url, sent) = headers(&host[0]);
        assert_eq!(
            url,
            format!("http://127.0.0.1:8080/mcp-proxy/{}", proxy_key(signed_in))
        );
        assert_eq!(
            sent,
            vec![format!("{PROXY_HEADER}: {}", proxy_secret().unwrap())]
        );
        assert_eq!(
            url_for_key(&proxy_key(signed_in)).as_deref(),
            Some(signed_in)
        );
        assert_eq!(headers(&host[1]).1, vec!["Authorization: Bearer mine"]);
    }
}
