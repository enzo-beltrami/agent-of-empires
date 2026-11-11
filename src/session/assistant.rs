//! The dashboard assistant: a structured session in a fixed app-dir directory whose worker
//! gets the daemon's own MCP server, so it can read and drive the other sessions.

use std::path::{Path, PathBuf};

use agent_client_protocol::v1::schema::{McpServer, McpServerStdio};
use anyhow::Result;

/// Name of the MCP server the assistant's worker sees; its tools are `mcp__aoe__*`.
pub const MCP_SERVER_NAME: &str = "aoe";

const INSTRUCTIONS: &str = include_str!("assistant/CLAUDE.md");

/// Claude Code permissions letting the assistant call every `aoe` tool without a prompt.
const SETTINGS: &str = r#"{
  "permissions": {
    "allow": ["mcp__aoe"]
  }
}
"#;

pub fn assistant_dir() -> Result<PathBuf> {
    Ok(super::get_app_dir_path()?.join("assistant"))
}

pub fn is_assistant_dir(cwd: &Path) -> bool {
    assistant_dir().is_ok_and(|dir| dir == cwd)
}

/// Create the directory with its default instructions and permissions. Existing files are
/// kept, so the user can edit them.
pub fn provision() -> Result<PathBuf> {
    let dir = assistant_dir()?;
    std::fs::create_dir_all(dir.join(".claude"))?;
    write_if_missing(&dir.join("CLAUDE.md"), INSTRUCTIONS)?;
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
        assert!(is_assistant_dir(&dir));
        let instructions = dir.join("CLAUDE.md");
        std::fs::write(&instructions, "mine").unwrap();
        provision().unwrap();
        assert_eq!(std::fs::read_to_string(&instructions).unwrap(), "mine");
        let settings = std::fs::read_to_string(dir.join(".claude/settings.json")).unwrap();
        assert!(settings.contains("mcp__aoe"));
    }
}
