//! The dashboard coordinator: a structured session in a fixed app-dir directory whose worker
//! gets the daemon's own MCP server, so it can read and drive the other sessions.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use agent_client_protocol::v1::schema::{McpServer, McpServerStdio};
use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Name of the MCP server the coordinator's worker sees; its tools are `mcp__aoe__*`.
pub const MCP_SERVER_NAME: &str = "aoe";

const INSTRUCTIONS: &str = include_str!("coordinator/CLAUDE.md");
const WORKFLOW: &str = include_str!("coordinator/WORKFLOW.md");

/// Claude Code permissions: every `aoe` tool, and edits to its own notes and workflow, run
/// without a prompt.
const SETTINGS: &str = r#"{
  "permissions": {
    "allow": ["mcp__aoe", "Edit(*.md)", "Write(*.md)"]
  }
}
"#;

const WATCHLIST: &str = "watching.json";

/// Sessions whose status changes the daemon reports to the coordinator.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Watchlist {
    /// Every session except the coordinator itself.
    #[serde(default)]
    pub all: bool,
    #[serde(default)]
    pub sessions: BTreeSet<String>,
}

impl Watchlist {
    pub fn watches(&self, id: &str) -> bool {
        self.all || self.sessions.contains(id)
    }
}

/// The saved watchlist; empty when there is none or it cannot be read.
pub fn watchlist() -> Watchlist {
    load(WATCHLIST)
}

pub fn update_watchlist(change: impl FnOnce(&mut Watchlist)) -> Result<Watchlist> {
    update(WATCHLIST, |list| {
        change(list);
        Ok(())
    })
    .map(|(list, ())| list)
}

const DEPLOYS: &str = "deploys.json";

/// A pull request queued for deployment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Deploy {
    pub id: String,
    /// The pull request's URL.
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The session that produced it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    /// The ticket it resolves: a key such as `PI-1921`, or a URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ticket: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub added_at: chrono::DateTime<chrono::Utc>,
    /// Unset while the pull request still waits to be deployed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct DeployList {
    #[serde(default)]
    deploys: Vec<Deploy>,
}

pub struct NewDeploy {
    pub url: String,
    pub title: Option<String>,
    pub session_id: Option<String>,
    pub ticket: Option<String>,
    pub note: Option<String>,
}

/// The deploy list, oldest first.
pub fn deploys() -> Vec<Deploy> {
    load::<DeployList>(DEPLOYS).deploys
}

/// Adds a pull request. For a URL already on the list, the fields given replace its own, so a
/// ticket or title can be linked later; omitted fields are kept.
pub fn add_deploy(new: NewDeploy) -> Result<Deploy> {
    let url = new.url.trim().to_string();
    if url.is_empty() {
        anyhow::bail!("The pull request URL is empty");
    }
    let blank = |s: Option<String>| s.map(|s| s.trim().to_string()).filter(|s| !s.is_empty());
    let (title, session_id, ticket, note) = (
        blank(new.title),
        blank(new.session_id),
        blank(new.ticket),
        blank(new.note),
    );
    update(DEPLOYS, |list: &mut DeployList| {
        if let Some(existing) = list.deploys.iter_mut().find(|d| d.url == url) {
            for (field, value) in [
                (&mut existing.title, title),
                (&mut existing.session_id, session_id),
                (&mut existing.ticket, ticket),
                (&mut existing.note, note),
            ] {
                if value.is_some() {
                    *field = value;
                }
            }
            return Ok(existing.clone());
        }
        let deploy = Deploy {
            id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
            url,
            title,
            session_id,
            ticket,
            note,
            added_at: chrono::Utc::now(),
            deployed_at: None,
        };
        list.deploys.push(deploy.clone());
        Ok(deploy)
    })
    .map(|(_, deploy)| deploy)
}

/// Marks a deploy deployed (or pending again); `None` when there is no such id.
pub fn set_deployed(id: &str, deployed: bool) -> Result<Option<Deploy>> {
    update(DEPLOYS, |list: &mut DeployList| {
        Ok(list.deploys.iter_mut().find(|d| d.id == id).map(|d| {
            d.deployed_at = deployed.then(|| d.deployed_at.unwrap_or_else(chrono::Utc::now));
            d.clone()
        }))
    })
    .map(|(_, deploy)| deploy)
}

/// Removes a deploy; false when there is no such id.
pub fn remove_deploy(id: &str) -> Result<bool> {
    update(DEPLOYS, |list: &mut DeployList| {
        let before = list.deploys.len();
        list.deploys.retain(|d| d.id != id);
        Ok(list.deploys.len() != before)
    })
    .map(|(_, removed)| removed)
}

/// Serializes read-modify-write of the coordinator's files across dashboard and MCP callers.
static FILES: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn load<T: Default + serde::de::DeserializeOwned>(name: &str) -> T {
    coordinator_dir()
        .and_then(|dir| Ok(std::fs::read(dir.join(name))?))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn update<T, R>(name: &str, change: impl FnOnce(&mut T) -> Result<R>) -> Result<(T, R)>
where
    T: Default + Serialize + serde::de::DeserializeOwned,
{
    let _files = FILES
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let dir = coordinator_dir()?;
    std::fs::create_dir_all(&dir)?;
    let mut value: T = load(name);
    let result = change(&mut value)?;
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec_pretty(&value)?)?;
    std::fs::rename(tmp, dir.join(name))?;
    Ok((value, result))
}

pub fn coordinator_dir() -> Result<PathBuf> {
    Ok(super::get_app_dir_path()?.join("coordinator"))
}

/// Create the directory with its default instructions and permissions. Existing files are
/// kept, so the user can edit them.
pub fn provision() -> Result<PathBuf> {
    let dir = coordinator_dir()?;
    std::fs::create_dir_all(dir.join(".claude"))?;
    write_if_missing(&dir.join("CLAUDE.md"), INSTRUCTIONS)?;
    write_if_missing(&dir.join("WORKFLOW.md"), WORKFLOW)?;
    write_if_missing(&dir.join(".claude").join("settings.json"), SETTINGS)?;
    Ok(dir)
}

fn write_if_missing(path: &Path, contents: &str) -> Result<()> {
    match std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(contents.as_bytes())?;
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// `aoe mcp stdio --session <id>`, which relays to the daemon's owner socket as that session.
/// The socket needs no bearer token, so nothing baked into a long-lived worker goes stale when
/// the token rotates.
pub fn mcp_server(session_id: &str) -> Option<McpServer> {
    let exe = std::env::current_exe().ok()?;
    Some(McpServer::Stdio(
        McpServerStdio::new(MCP_SERVER_NAME, exe).args(vec![
            "mcp".into(),
            "stdio".into(),
            "--session".into(),
            session_id.into(),
        ]),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn provision_keeps_user_edits() {
        let _home = crate::session::test_support::isolate_app_dir();
        let dir = provision().unwrap();
        assert_eq!(dir, coordinator_dir().unwrap());
        let instructions = dir.join("CLAUDE.md");
        std::fs::write(&instructions, "mine").unwrap();
        provision().unwrap();
        assert_eq!(std::fs::read_to_string(&instructions).unwrap(), "mine");
        let settings = std::fs::read_to_string(dir.join(".claude/settings.json")).unwrap();
        assert!(settings.contains("mcp__aoe"));
        assert!(dir.join("WORKFLOW.md").is_file());
    }

    #[test]
    #[serial]
    fn watchlist_round_trips_and_tolerates_a_missing_file() {
        let _home = crate::session::test_support::isolate_app_dir();
        assert_eq!(watchlist(), Watchlist::default());
        update_watchlist(|list| {
            list.sessions.insert("a".into());
        })
        .unwrap();
        let list = watchlist();
        assert!(list.watches("a") && !list.watches("b"));
        update_watchlist(|list| list.all = true).unwrap();
        assert!(watchlist().watches("b"));
    }

    #[test]
    #[serial]
    fn deploys_add_once_per_url_toggle_and_remove() {
        let _home = crate::session::test_support::isolate_app_dir();
        let new = |url: &str| NewDeploy {
            url: url.into(),
            title: Some("  ".into()),
            session_id: None,
            ticket: None,
            note: Some("first".into()),
        };
        let first = add_deploy(new(" https://x/pull/1 ")).unwrap();
        assert_eq!(first.url, "https://x/pull/1");
        assert_eq!(first.title, None, "blank fields are dropped");
        let linked = add_deploy(NewDeploy {
            ticket: Some("PI-1921".into()),
            note: None,
            ..new("https://x/pull/1")
        })
        .unwrap();
        assert_eq!(
            linked.id, first.id,
            "a known URL is updated, not added again"
        );
        assert_eq!(linked.ticket.as_deref(), Some("PI-1921"));
        assert_eq!(
            linked.note.as_deref(),
            Some("first"),
            "omitted fields are kept"
        );
        assert!(add_deploy(new(" ")).is_err());
        add_deploy(new("https://x/pull/2")).unwrap();
        assert_eq!(deploys().len(), 2);

        let deployed = set_deployed(&first.id, true).unwrap().unwrap();
        let at = deployed.deployed_at.unwrap();
        let again = set_deployed(&first.id, true).unwrap().unwrap();
        assert_eq!(
            again.deployed_at,
            Some(at),
            "re-marking keeps the first time"
        );
        assert_eq!(
            set_deployed(&first.id, false).unwrap().unwrap().deployed_at,
            None
        );
        assert_eq!(set_deployed("nope", true).unwrap(), None);

        assert!(remove_deploy(&first.id).unwrap());
        assert!(!remove_deploy(&first.id).unwrap());
        assert_eq!(deploys().len(), 1);
    }
}
