use serial_test::parallel;
use std::process::Command;
use std::time::Duration;

use crate::harness::{require_tmux, TuiTestHarness};

/// Read sessions.json from the harness's isolated home.
fn read_sessions_json(h: &TuiTestHarness) -> serde_json::Value {
    let sessions_path =
        crate::harness::app_dir_in(h.home_path()).join("profiles/default/sessions.json");
    let content = std::fs::read_to_string(&sessions_path)
        .unwrap_or_else(|e| panic!("failed to read {}: {}", sessions_path.display(), e));
    serde_json::from_str(&content).expect("invalid sessions JSON")
}

/// Best-effort cleanup so failures don't leak into the next `#[parallel]` test.
fn kill_tmux(sock: &std::path::Path, name: &str) {
    let _ = Command::new("tmux")
        .arg("-S")
        .arg(sock)
        .args(["kill-session", "-t", name])
        .output();
}

fn mark_tool_owned(h: &TuiTestHarness, name: &str, session_id: &str, tool_name: &str) {
    let target = format!("={name}:");
    let owner = serde_json::to_string(&(session_id, tool_name)).unwrap();
    let mark = h
        .tmux()
        .args([
            "set-option",
            "-t",
            &target,
            "@aoe_tool_owner",
            &owner,
            ";",
            "set-option",
            "-t",
            &target,
            "@aoe_kind",
            "tool",
        ])
        .output()
        .expect("mark archive tool fixture ownership");
    assert!(
        mark.status.success(),
        "mark archive tool fixture failed: {}",
        String::from_utf8_lossy(&mark.stderr)
    );
}

fn tmux_has_session(sock: &std::path::Path, name: &str) -> bool {
    Command::new("tmux")
        .arg("-S")
        .arg(sock)
        .args(["has-session", "-t", name])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Locks #1868: archive kills all four tmux session kinds. Pre-creates real
/// sessions, runs the CLI, asserts each kind is gone.
#[test]
#[parallel]
fn test_cli_archive_kills_agent_and_terminal_tmux_sessions() {
    require_tmux!();

    let h = TuiTestHarness::new("cli_archive_full_teardown");
    let project = h.project_path();

    let add_output = h.run_cli(&["add", project.to_str().unwrap(), "-t", "ArchiveTeardown"]);
    assert!(
        add_output.status.success(),
        "aoe add failed: {}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    let sessions = read_sessions_json(&h);
    let session_id = sessions[0]["id"]
        .as_str()
        .expect("session should have id")
        .to_string();
    let truncated_id = session_id[..8.min(session_id.len())].to_string();

    let agent_tmux_name = format!(
        "{}ArchiveTeardown_{}",
        agent_of_empires::tmux::SESSION_PREFIX,
        truncated_id
    );
    let terminal_tmux_name =
        agent_of_empires::tmux::TerminalSession::generate_name(&session_id, "ArchiveTeardown");
    let cterm_tmux_name = agent_of_empires::tmux::ContainerTerminalSession::generate_name(
        &session_id,
        "ArchiveTeardown",
    );
    let tool_tmux_name =
        agent_of_empires::tmux::ToolSession::new(&session_id, "ArchiveTeardown", "lazygit")
            .session_name()
            .to_string();

    // Pre-create the four tmux kinds so archive can find and kill them. Created
    // on the harness's tmux socket (`AOE_TMUX_SOCKET`), the same one the CLI
    // resolves (#2608), so archive's teardown sweeps them.
    let sock = h.home_path().join("tmux.sock");
    let names = [
        &agent_tmux_name,
        &terminal_tmux_name,
        &cterm_tmux_name,
        &tool_tmux_name,
    ];
    for name in names {
        let target = format!("={name}:");
        let create = Command::new("tmux")
            .arg("-S")
            .arg(&sock)
            .args([
                "new-session",
                "-d",
                "-s",
                name,
                "-x",
                "80",
                "-y",
                "24",
                "sleep",
                "600",
                ";",
                "set-option",
                "-t",
                &target,
                "pane-base-index",
                "0",
            ])
            .output()
            .expect("tmux new-session");
        assert!(
            create.status.success(),
            "failed to create tmux session {}: {}",
            name,
            String::from_utf8_lossy(&create.stderr)
        );
    }

    mark_tool_owned(&h, &tool_tmux_name, &session_id, "lazygit");

    let archive_output = h.run_cli(&["session", "archive", &session_id]);
    assert!(
        archive_output.status.success(),
        "aoe session archive failed: {}",
        String::from_utf8_lossy(&archive_output.stderr)
    );

    let alive: Vec<(&&String, bool)> = names
        .iter()
        .map(|n| (n, tmux_has_session(&sock, n)))
        .collect();

    // Cleanup BEFORE asserting so a single failure cannot leak survivors
    // into the next serial test.
    for (name, is_alive) in &alive {
        if *is_alive {
            kill_tmux(&sock, name);
        }
    }

    for (name, is_alive) in &alive {
        assert!(
            !is_alive,
            "tmux session '{}' must be killed by archive (#1868)",
            name
        );
    }
}

/// Locks the widened `--no-kill` semantic from #1868: skip ALL tmux
/// teardown. The agent assertion is the pre/post differentiator.
#[test]
#[parallel]
fn test_cli_archive_no_kill_preserves_all_tmux_sessions() {
    require_tmux!();

    let h = TuiTestHarness::new("cli_archive_no_kill");
    let project = h.project_path();

    let add_output = h.run_cli(&["add", project.to_str().unwrap(), "-t", "ArchiveNoKill"]);
    assert!(
        add_output.status.success(),
        "aoe add failed: {}",
        String::from_utf8_lossy(&add_output.stderr)
    );

    let sessions = read_sessions_json(&h);
    let session_id = sessions[0]["id"]
        .as_str()
        .expect("session should have id")
        .to_string();
    let truncated_id = session_id[..8.min(session_id.len())].to_string();

    let agent_tmux_name = format!(
        "{}ArchiveNoKill_{}",
        agent_of_empires::tmux::SESSION_PREFIX,
        truncated_id
    );
    let terminal_tmux_name =
        agent_of_empires::tmux::TerminalSession::generate_name(&session_id, "ArchiveNoKill");
    let cterm_tmux_name = agent_of_empires::tmux::ContainerTerminalSession::generate_name(
        &session_id,
        "ArchiveNoKill",
    );
    let tool_tmux_name =
        agent_of_empires::tmux::ToolSession::new(&session_id, "ArchiveNoKill", "lazygit")
            .session_name()
            .to_string();

    let sock = h.home_path().join("tmux.sock");
    let names = [
        &agent_tmux_name,
        &terminal_tmux_name,
        &cterm_tmux_name,
        &tool_tmux_name,
    ];
    for name in names {
        let target = format!("={name}:");
        let create = Command::new("tmux")
            .arg("-S")
            .arg(&sock)
            .args([
                "new-session",
                "-d",
                "-s",
                name,
                "-x",
                "80",
                "-y",
                "24",
                "sleep",
                "600",
                ";",
                "set-option",
                "-t",
                &target,
                "pane-base-index",
                "0",
            ])
            .output()
            .expect("tmux new-session");
        assert!(
            create.status.success(),
            "failed to create tmux session {}: {}",
            name,
            String::from_utf8_lossy(&create.stderr)
        );
    }

    mark_tool_owned(&h, &tool_tmux_name, &session_id, "lazygit");

    let archive_output = h.run_cli(&["session", "archive", &session_id, "--no-kill"]);
    assert!(
        archive_output.status.success(),
        "aoe session archive --no-kill failed: {}",
        String::from_utf8_lossy(&archive_output.stderr)
    );

    let alive: Vec<(&&String, bool)> = names
        .iter()
        .map(|n| (n, tmux_has_session(&sock, n)))
        .collect();

    // Cleanup explicitly: --no-kill leaves the survivors so they would
    // pollute the next serial test.
    for name in &names {
        kill_tmux(&sock, name);
    }

    for (name, is_alive) in &alive {
        assert!(
            *is_alive,
            "tmux session '{}' must survive --no-kill archive (#1868)",
            name
        );
    }

    let post_archive = read_sessions_json(&h);
    let archived_at = post_archive[0]["archived_at"].as_str();
    assert!(
        archived_at.is_some() && !archived_at.unwrap().is_empty(),
        "session must still be archived on disk even with --no-kill: archived_at = {:?}",
        archived_at
    );
}

/// A row archived while `Waiting` has no pane behind it (archive tears the
/// tmux down, #1868) and the poller never revisits archived rows (#2206), so
/// the persisted status used to stand forever: `aoe ps` kept listing a
/// pending-permission row that nothing could clear. Seeds that persisted
/// shape straight onto disk and checks both layers on the real binary: the
/// archived poll guard settles the row on read, and migration v028 settles
/// the stored row once for installs that predate the fix. A resting status
/// (`error`) on an archived row is the control: neither layer touches it.
#[test]
#[parallel]
fn test_archived_waiting_row_reads_idle_and_migrates_once() {
    require_tmux!();

    let h = TuiTestHarness::new("archive_waiting_zombie");
    let app_dir = crate::harness::app_dir_in(h.home_path());
    let profile_dir = app_dir.join("profiles").join("default");
    std::fs::create_dir_all(&profile_dir).expect("create profile dir");
    let sessions_path = profile_dir.join("sessions.json");
    let version_path = app_dir.join(".schema_version");

    // First boot stamps the build's schema version, so the read below
    // exercises the in-process guard alone rather than the migration.
    let boot = h.run_cli(&["ps", "--json"]);
    assert!(
        boot.status.success(),
        "aoe ps failed: {}",
        String::from_utf8_lossy(&boot.stderr)
    );
    let stamped: u32 = std::fs::read_to_string(&version_path)
        .expect("first boot stamps .schema_version")
        .trim()
        .parse()
        .expect("schema version is a number");
    assert!(stamped >= 28, "build must carry v028, stamped {stamped}");

    // `aoe ps` joins rows to instances by the 8-char id suffix of the tmux
    // name, so the ids must be underscore-free like real session ids.
    let project = h.project_path();
    let project = project.to_str().unwrap();
    let row = |id: &str, title: &str, status: &str| {
        format!(
            r#"{{"id":"{id}","title":"{title}","project_path":"{project}","group_path":"","command":"","tool":"claude","yolo_mode":false,"status":"{status}","archived_at":"2026-07-13T22:17:21Z","created_at":"2026-01-01T00:00:00Z"}}"#
        )
    };
    let frozen = format!(
        "[{},{}]",
        row("frozen0waiting01", "Frozen", "waiting"),
        row("resting0error001", "Resting", "error")
    );
    std::fs::write(&sessions_path, &frozen).expect("write sessions.json");

    // No tmux pane exists for either row. The archived guard must settle the
    // frozen Waiting to idle on read and leave the resting Error alone.
    let ps = h.run_cli(&["ps", "--json", "--dead"]);
    assert!(
        ps.status.success(),
        "aoe ps failed: {}",
        String::from_utf8_lossy(&ps.stderr)
    );
    let rows: serde_json::Value =
        serde_json::from_slice(&ps.stdout).expect("aoe ps --json emits JSON");
    let state_of = |id: &str| {
        rows.as_array()
            .unwrap()
            .iter()
            .find(|r| r["session"] == id)
            .unwrap_or_else(|| panic!("{id} missing from aoe ps --json: {rows}"))["state"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(
        state_of("frozen0waiting01"),
        "idle",
        "an archived row must never read as a pending-permission row"
    );
    assert_eq!(state_of("resting0error001"), "dead");

    // An install that predates v028 boots on the same stored rows: the
    // migration settles the frozen row once and leaves the control alone.
    std::fs::write(&sessions_path, &frozen).expect("rewrite sessions.json");
    std::fs::write(&version_path, "27").expect("rewind .schema_version");
    let migrated = h.run_cli(&["ps", "--json"]);
    assert!(
        migrated.status.success(),
        "aoe ps failed: {}",
        String::from_utf8_lossy(&migrated.stderr)
    );
    let stored = read_sessions_json(&h);
    let stored_row = |id: &str| {
        stored
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["id"] == id)
            .unwrap_or_else(|| panic!("{id} missing from sessions.json: {stored}"))
            .clone()
    };
    let healed = stored_row("frozen0waiting01");
    assert_eq!(healed["status"], "idle", "v028 settles the stored row");
    assert_eq!(
        healed["archived_at"], "2026-07-13T22:17:21Z",
        "the archive itself survives the settle"
    );
    assert_eq!(stored_row("resting0error001")["status"], "error");
    let after: u32 = std::fs::read_to_string(&version_path)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(
        after, stamped,
        "the rewound install converges on the build's version"
    );
}

#[test]
#[parallel]
fn trash_retry_ignores_permanent_delete_policy() {
    use agent_of_empires::session::{Instance, LifecycleOperation, Status};
    require_tmux!();
    let mut h = TuiTestHarness::new("trash_retry_policy");
    h.stop_daemon_on_drop();
    h.append_config("[session]\ndelete_to_trash = false");
    let mut row = Instance::new("PendingTrashPolicy", h.project_path().to_str().unwrap());
    row.status = Status::Stopped;
    let generation = row
        .try_acquire_lifecycle_reservation(
            LifecycleOperation::Trash,
            Instance::LIFECYCLE_RESERVATION_TTL,
            chrono::Utc::now() - Instance::LIFECYCLE_RESERVATION_TTL - chrono::Duration::seconds(1),
        )
        .unwrap();
    row.trash();
    let id = row.id.clone();
    let identity = row.trash_lifecycle_id.clone().unwrap();
    std::fs::write(h.sessions_path(), serde_json::to_vec(&[row]).unwrap()).unwrap();
    h.start_daemon();
    let refused = h.run_cli(&["rm", "--purge", &id]);
    assert!(!refused.status.success());
    let message = String::from_utf8_lossy(&refused.stderr);
    assert!(message.contains("aoe rm"), "{message}");
    h.run_cli_ok(&["rm", &id]);
    let rows = h.read_sessions();
    let row = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap();
    assert!(row["trashed_at"].is_string());
    assert_eq!(row["trash_lifecycle_id"], identity);
    assert_eq!(row["lifecycle_generation"], generation + 1);
    assert!(row["lifecycle_reservation"].is_null());
    h.run_cli_ok(&["session", "restore", &id]);
    // Restore writes locally; wait for the daemon's selection snapshot.
    crate::harness::wait_until(Duration::from_secs(10), Duration::from_millis(20), || {
        let rows: serde_json::Value =
            serde_json::from_str(&h.run_cli_ok(&["list", "--json"])).unwrap();
        if rows
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["id"] == id && row["trashed_at"].is_null())
        {
            Ok(())
        } else {
            Err("daemon has not adopted the restored row".into())
        }
    });
    h.run_cli_ok(&["rm", &id]);
    assert!(!h
        .read_sessions()
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == id));
}
