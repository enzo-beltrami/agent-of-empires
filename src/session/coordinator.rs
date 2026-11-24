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

/// Claude Code permissions: every `aoe` tool, and edits to its own instructions and workflow, run
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
    #[serde(default)]
    pub tested: bool,
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
    pub tested: Option<bool>,
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
            if let Some(tested) = new.tested {
                existing.tested = tested;
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
            tested: new.tested.unwrap_or(false),
            added_at: chrono::Utc::now(),
            deployed_at: None,
        };
        list.deploys.push(deploy.clone());
        Ok(deploy)
    })
    .map(|(_, deploy)| deploy)
}

/// Flags to change on a deploy; `None` keeps the current value.
#[derive(Debug, Default, Clone, Copy, Deserialize)]
pub struct DeployChange {
    /// False moves it back to pending.
    pub deployed: Option<bool>,
    pub tested: Option<bool>,
}

/// Applies `change` to a deploy; `None` when there is no such id.
pub fn update_deploy(id: &str, change: DeployChange) -> Result<Option<Deploy>> {
    update(DEPLOYS, |list: &mut DeployList| {
        Ok(list.deploys.iter_mut().find(|d| d.id == id).map(|d| {
            if let Some(deployed) = change.deployed {
                d.deployed_at = deployed.then(|| d.deployed_at.unwrap_or_else(chrono::Utc::now));
            }
            if let Some(tested) = change.tested {
                d.tested = tested;
            }
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

/// The user's notes, shared with the coordinator, which keeps what it is asked to remember here.
pub(crate) const NOTES: &str = "notes.json";

/// One note in the coordinator's Notes tab.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Note {
    pub id: String,
    /// Markdown.
    pub text: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

impl Note {
    pub(crate) fn new(text: String, at: chrono::DateTime<chrono::Utc>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().simple().to_string()[..12].to_string(),
            text,
            created_at: at,
            updated_at: at,
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct NoteList {
    #[serde(default)]
    pub(crate) notes: Vec<Note>,
}

/// The notes, oldest first.
pub fn notes() -> Vec<Note> {
    load::<NoteList>(NOTES).notes
}

fn note_text(text: &str) -> Result<String> {
    let text = text.trim();
    if text.is_empty() {
        anyhow::bail!("The note is empty");
    }
    Ok(text.to_string())
}

pub fn add_note(text: &str) -> Result<Note> {
    let note = Note::new(note_text(text)?, chrono::Utc::now());
    update(NOTES, |list: &mut NoteList| {
        list.notes.push(note.clone());
        Ok(())
    })?;
    Ok(note)
}

/// Replaces a note's text; `None` when there is no such id.
pub fn update_note(id: &str, text: &str) -> Result<Option<Note>> {
    let text = note_text(text)?;
    update(NOTES, |list: &mut NoteList| {
        Ok(list.notes.iter_mut().find(|n| n.id == id).map(|n| {
            if n.text != text {
                n.text = text;
                n.updated_at = chrono::Utc::now();
            }
            n.clone()
        }))
    })
    .map(|(_, note)| note)
}

/// Removes a note; false when there is no such id.
pub fn remove_note(id: &str) -> Result<bool> {
    update(NOTES, |list: &mut NoteList| {
        let before = list.notes.len();
        list.notes.retain(|n| n.id != id);
        Ok(list.notes.len() != before)
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
    let mut value: T = load(name);
    let result = change(&mut value)?;
    write_atomic(&dir, name, &serde_json::to_vec_pretty(&value)?)?;
    Ok((value, result))
}

fn write_atomic(dir: &Path, name: &str, bytes: &[u8]) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let tmp = dir.join(format!("{name}.tmp"));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(tmp, dir.join(name))?;
    Ok(())
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
    fn notes_add_edit_and_remove_individually() {
        let _home = crate::session::test_support::isolate_app_dir();
        assert!(notes().is_empty());
        let first = add_note("  ship PI-1921 Friday \n").unwrap();
        assert_eq!(first.text, "ship PI-1921 Friday");
        let second = add_note("ask about tests").unwrap();
        assert!(add_note("   ").is_err());
        assert_eq!(notes(), [first.clone(), second.clone()]);

        let edited = update_note(&first.id, "ship PI-1921 Monday")
            .unwrap()
            .unwrap();
        assert_eq!(edited.text, "ship PI-1921 Monday");
        assert!(edited.updated_at >= first.updated_at);
        assert_eq!(edited.created_at, first.created_at);
        assert!(update_note(&first.id, " ").is_err());
        assert_eq!(update_note("nope", "x").unwrap(), None);

        assert!(remove_note(&second.id).unwrap());
        assert!(!remove_note(&second.id).unwrap());
        assert_eq!(notes(), [edited]);
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
            tested: None,
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

        let set = |deployed, tested| {
            update_deploy(&first.id, DeployChange { deployed, tested })
                .unwrap()
                .unwrap()
        };
        let deployed = set(Some(true), None);
        let at = deployed.deployed_at.unwrap();
        assert!(!deployed.tested, "new deploys are untested");
        let again = set(Some(true), Some(true));
        assert_eq!(
            again.deployed_at,
            Some(at),
            "re-marking keeps the first time"
        );
        assert!(again.tested);
        let pending = set(Some(false), None);
        assert_eq!(pending.deployed_at, None);
        assert!(pending.tested, "an omitted flag is kept");
        let untested = add_deploy(NewDeploy {
            tested: Some(false),
            ..new("https://x/pull/1")
        })
        .unwrap();
        assert!(!untested.tested, "re-adding can set the flag");
        assert_eq!(
            update_deploy("nope", DeployChange::default()).unwrap(),
            None
        );

        assert!(remove_deploy(&first.id).unwrap());
        assert!(!remove_deploy(&first.id).unwrap());
        assert_eq!(deploys().len(), 1);
    }
}
