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
    coordinator_dir()
        .and_then(|dir| Ok(std::fs::read(dir.join(WATCHLIST))?))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn update_watchlist(change: impl FnOnce(&mut Watchlist)) -> Result<Watchlist> {
    let dir = coordinator_dir()?;
    std::fs::create_dir_all(&dir)?;
    let mut list = watchlist();
    change(&mut list);
    let tmp = dir.join(format!("{WATCHLIST}.tmp"));
    std::fs::write(&tmp, serde_json::to_vec_pretty(&list)?)?;
    std::fs::rename(tmp, dir.join(WATCHLIST))?;
    Ok(list)
}

pub fn coordinator_dir() -> Result<PathBuf> {
    Ok(super::get_app_dir_path()?.join("coordinator"))
}

pub fn is_coordinator_dir(cwd: &Path) -> bool {
    coordinator_dir().is_ok_and(|dir| dir == cwd)
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

/// `aoe mcp stdio`, which relays to the daemon's owner socket. Using the socket instead of
/// the TCP listener means no bearer token is baked into a long-lived worker and outlives
/// a rotation.
pub fn mcp_server() -> Option<McpServer> {
    let exe = std::env::current_exe().ok()?;
    Some(McpServer::Stdio(
        McpServerStdio::new(MCP_SERVER_NAME, exe).args(vec!["mcp".into(), "stdio".into()]),
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
        assert!(is_coordinator_dir(&dir));
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
}
