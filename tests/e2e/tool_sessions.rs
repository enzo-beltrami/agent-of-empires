//! E2E coverage for the [tools.*] feature: picker dialog, command-palette
//! integration, and the full attach + cleanup roundtrip against a real agent
//! session.

use serial_test::parallel;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use crate::harness::{app_dir_in, require_tmux, TuiTestHarness};

/// Append a `[tools.*]` block to the harness's pre-seeded config.toml.
fn append_tools_config(h: &TuiTestHarness, body: &str) {
    let path = app_dir_in(h.home_path()).join("config.toml");
    let existing = fs::read_to_string(&path).expect("read pre-seeded config.toml");
    fs::write(&path, format!("{existing}\n{body}\n")).expect("write tools config");
}

fn shell_quote_path(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

fn wait_for_file_contents(path: &Path, timeout: Duration) -> String {
    let start = Instant::now();
    loop {
        if let Ok(contents) = fs::read_to_string(path) {
            if !contents.trim().is_empty() {
                return contents;
            }
        }
        if start.elapsed() >= timeout {
            panic!("timed out waiting for {}", path.display());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Wait until the daemon has published `generation` with no live lifecycle
/// reservation for `id`.
///
/// This test process writes `sessions.json` directly, so the row only reaches
/// the daemon's canonical in-memory state through its debounced disk watch.
/// A mutation issued before that convergence is rejected as superseded, so
/// callers that write the file out of process must wait for it first.
async fn wait_for_lifecycle_published(
    http: &reqwest::Client,
    id: &str,
    generation: u64,
    timeout: Duration,
) {
    let start = Instant::now();
    loop {
        let snapshot: agent_of_empires::daemon::RuntimeSnapshot = http
            .get("http://localhost/api/runtime/snapshot")
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json()
            .await
            .unwrap();
        let published = snapshot
            .contents
            .sessions
            .iter()
            .find(|value| value.id == id)
            .expect("session missing from the published snapshot");
        if published.lifecycle_generation == generation && published.lifecycle_reservation.is_none()
        {
            return;
        }
        assert!(
            start.elapsed() < timeout,
            "generation {generation} without a reservation was not published"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
}

#[cfg(unix)]
#[tokio::test]
#[parallel]
async fn native_auxiliary_ensure_uses_fresh_context_and_refuses_purge() {
    use agent_of_empires::{
        daemon::DaemonClient,
        session::{Instance, LifecycleOperation, Status},
    };
    require_tmux!();
    let mut h = TuiTestHarness::new_in_tmp("native_tool_ensure");
    h.set_env("AGENT_OF_EMPIRES_PROFILE", "default");
    h.stop_daemon_on_drop();
    let app = app_dir_in(h.home_path());
    let first_output = h.home_path().join("first-tool-cwd");
    let second_output = h.home_path().join("second-tool-cwd");
    let command = |output: &Path| format!("pwd > {}; exec sleep 120", shell_quote_path(output));
    let first_command = serde_json::to_string(&command(&first_output)).unwrap();
    append_tools_config(&h, &format!("[tools.\"..\"]\ncommand = {first_command}"));
    let mut row = Instance::new("native tool", h.project_path().to_str().unwrap());
    row.status = Status::Stopped;
    row.terminal_info = Some(agent_of_empires::session::TerminalInfo { created: true });
    let rows_path = app.join("profiles/default/sessions.json");
    let session_id = row.id.clone();
    let load_committed = || {
        let rows: Vec<Instance> = serde_json::from_slice(&fs::read(&rows_path).unwrap()).unwrap();
        rows.into_iter()
            .find(|value| value.id == session_id)
            .unwrap()
    };
    let persist = |row: &Instance| {
        let mut pending = tempfile::NamedTempFile::new_in(rows_path.parent().unwrap()).unwrap();
        std::io::Write::write_all(&mut pending, &serde_json::to_vec(&[row]).unwrap()).unwrap();
        pending.persist(&rows_path).unwrap();
    };
    persist(&row);
    let started = h.run_cli(&["serve", "--core-only", "--daemon"]);
    assert!(
        started.status.success(),
        "{}",
        String::from_utf8_lossy(&started.stderr)
    );
    let sdk = DaemonClient::new_unix(app.join("daemon/api.sock")).unwrap();
    let epoch = sdk.runtime_info().await.unwrap().epoch;
    let http = reqwest::Client::builder()
        .unix_socket(app.join("daemon/api.sock"))
        .no_proxy()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap();
    let inspect_pane = |name: &str| {
        let output = Command::new("tmux")
            .arg("-S")
            .arg(h.home_path().join("tmux.sock"))
            .args([
                "display-message",
                "-p",
                "-t",
                &format!("={name}:^.0"),
                "#{pane_pid}\t#{pane_width}\t#{pane_height}\t#{pane_current_path}",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let host_url = format!("http://localhost/api/sessions/{}/terminal", row.id);
    let ensure_host = || {
        http.post(&host_url)
            .header(agent_of_empires::daemon::RUNTIME_EPOCH_HEADER, &epoch)
            .json(&serde_json::json!({"size": {"cols": 71, "rows": 29}}))
            .send()
    };
    let host = ensure_host().await.unwrap();
    assert_eq!(
        host.status(),
        reqwest::StatusCode::CREATED,
        "a persisted cache flag is not a live terminal"
    );
    assert!(host
        .headers()
        .contains_key(agent_of_empires::daemon::RUNTIME_REVISION_HEADER));
    let host: serde_json::Value = host.json().await.unwrap();
    let host_name = host["tmux_session"].as_str().unwrap();
    let original_host = inspect_pane(host_name);
    assert_eq!(original_host.split('\t').nth(1), Some("71"));
    assert_eq!(original_host.split('\t').nth(2), Some("29"));
    // Inject only the stale cache flag, never roll back an authoritative generation.
    row = load_committed();
    row.terminal_info = None;
    persist(&row);
    let ensured_host = sdk
        .ensure_terminal(
            &row.id,
            0,
            &agent_of_empires::daemon::StartSessionBody::default(),
            &epoch,
        )
        .await
        .unwrap();
    assert!(matches!(
        ensured_host.outcome.status,
        agent_of_empires::daemon::TerminalTargetStatus::Exists
    ));
    assert_eq!(ensured_host.outcome.tmux_session, host_name);
    assert_eq!(inspect_pane(host_name), original_host);
    let durable: Vec<Instance> = serde_json::from_slice(&fs::read(&rows_path).unwrap()).unwrap();
    assert!(
        durable[0].has_terminal(),
        "terminal creation was not committed"
    );
    row = durable
        .into_iter()
        .find(|value| value.id == session_id)
        .unwrap();
    let url = format!("http://localhost/api/sessions/{}/tools/ensure", row.id);
    let ensure = || {
        http.post(&url)
            .header(agent_of_empires::daemon::RUNTIME_EPOCH_HEADER, &epoch)
            .json(&serde_json::json!({"tool_name": "..", "size": {"cols": 91, "rows": 27}}))
            .send()
    };
    let injected = http
        .post(&url)
        .header(agent_of_empires::daemon::RUNTIME_EPOCH_HEADER, &epoch)
        .json(&serde_json::json!({"tool_name": "..", "command": "exit 0", "cwd": "/"}))
        .send()
        .await
        .unwrap();
    assert_eq!(injected.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);
    let response = ensure().await.unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    assert_eq!(
        response.headers()[agent_of_empires::daemon::RUNTIME_EPOCH_HEADER],
        epoch
    );
    assert!(response
        .headers()
        .contains_key(agent_of_empires::daemon::RUNTIME_REVISION_HEADER));
    let target: serde_json::Value = response.json().await.unwrap();
    let name = target["tmux_session"].as_str().unwrap();
    let pane = || inspect_pane(name);
    assert_eq!(
        wait_for_file_contents(&first_output, Duration::from_secs(5)).trim(),
        row.project_path
    );
    let original_pane = pane();
    assert_eq!(
        original_pane.split('\t').nth(1),
        Some("91"),
        "target {name}: {original_pane:?}"
    );
    assert_eq!(original_pane.split('\t').nth(2), Some("27"));
    row = load_committed();
    let generation = row
        .try_acquire_lifecycle_reservation(
            LifecycleOperation::Purge,
            Instance::LIFECYCLE_RESERVATION_TTL,
            chrono::Utc::now(),
        )
        .unwrap();
    persist(&row);
    assert_eq!(
        ensure().await.unwrap().status(),
        reqwest::StatusCode::CONFLICT
    );
    assert_eq!(
        ensure_host().await.unwrap().status(),
        reqwest::StatusCode::CONFLICT
    );
    let refused_stop = http
        .post(format!(
            "http://localhost/api/sessions/{}/auxiliary/stop",
            row.id
        ))
        .header(agent_of_empires::daemon::RUNTIME_EPOCH_HEADER, &epoch)
        .json(&serde_json::json!({"kind": "host", "index": 0}))
        .send()
        .await
        .unwrap();
    assert_eq!(refused_stop.status(), reqwest::StatusCode::CONFLICT);
    let refused_pair = http
        .delete(format!(
            "http://localhost/api/sessions/{}/terminal?index=1",
            row.id
        ))
        .header(agent_of_empires::daemon::RUNTIME_EPOCH_HEADER, &epoch)
        .send()
        .await
        .unwrap();
    assert_eq!(refused_pair.status(), reqwest::StatusCode::CONFLICT);
    let stream = tokio::net::UnixStream::connect(app.join("daemon/api.sock"))
        .await
        .unwrap();
    let websocket = tokio_tungstenite::client_async(
        format!("ws://localhost/sessions/{}/terminal/live-ws", row.id),
        stream,
    )
    .await;
    let Err(tokio_tungstenite::tungstenite::Error::Http(response)) = websocket else {
        panic!("a reserved purge admitted a host-terminal WebSocket");
    };
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(inspect_pane(host_name), original_host);
    assert_eq!(pane(), original_pane);
    row = load_committed();
    row.release_lifecycle_reservation_if_owned(LifecycleOperation::Purge, generation);
    persist(&row);
    wait_for_lifecycle_published(&http, &row.id, generation, Duration::from_secs(10)).await;
    let config_path = app.join("config.toml");
    let config = fs::read_to_string(&config_path).unwrap();
    fs::write(
        &config_path,
        config.replace(
            &first_command,
            &serde_json::to_string(&command(&second_output)).unwrap(),
        ),
    )
    .unwrap();
    let ensured = sdk
        .ensure_tool(
            &row.id,
            &agent_of_empires::daemon::EnsureToolBody {
                tool_name: "..".into(),
                size: None,
                adoption: None,
            },
            &epoch,
        )
        .await
        .unwrap();
    assert_eq!(ensured.outcome.tmux_session, name);
    assert_eq!(pane(), original_pane);
    assert!(
        !second_output.exists(),
        "ensure restarted a live tool after configuration changed"
    );
    let renamed = agent_of_empires::tmux::ToolSession::generate_name(&row.id, "Renamed", "..");
    let rename = |from: &str, to: &str| {
        let output = Command::new("tmux")
            .arg("-S")
            .arg(h.home_path().join("tmux.sock"))
            .args(["rename-session", "-t", &format!("={from}:"), to])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    rename(name, &renamed);
    let renamed_receipt = sdk
        .ensure_tool(
            &row.id,
            &agent_of_empires::daemon::EnsureToolBody {
                tool_name: "..".into(),
                size: None,
                adoption: None,
            },
            &epoch,
        )
        .await
        .unwrap();
    assert_eq!(renamed_receipt.outcome.tmux_session, renamed);
    let snapshot: agent_of_empires::daemon::RuntimeSnapshot = http
        .get("http://localhost/api/runtime/snapshot")
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(snapshot.cursor.epoch, epoch);
    assert!(snapshot.cursor.revision >= renamed_receipt.cursor.revision);
    let published = snapshot
        .contents
        .sessions
        .iter()
        .find(|item| item.id == row.id)
        .unwrap();
    for (target, expected_name) in [
        (
            agent_of_empires::session::AuxiliaryTarget::Host { index: 0 },
            host_name,
        ),
        (
            agent_of_empires::session::AuxiliaryTarget::Tool {
                tool_name: "..".into(),
            },
            renamed.as_str(),
        ),
    ] {
        let observed = published
            .auxiliary
            .iter()
            .find(|item| item.target == target)
            .unwrap();
        assert_eq!(
            observed.pane.state,
            agent_of_empires::session::PanePresence::Alive
        );
        assert_eq!(observed.pane.tmux_session.as_deref(), Some(expected_name));
    }
    rename(&renamed, name);
    assert_eq!(pane(), original_pane);
    let killed = Command::new("tmux")
        .arg("-S")
        .arg(h.home_path().join("tmux.sock"))
        .args(["kill-session", "-t", &format!("={name}")])
        .output()
        .unwrap();
    assert!(killed.status.success());
    let fresh = h.project_path().join("fresh-tool-cwd");
    fs::create_dir(&fresh).unwrap();
    row = load_committed();
    row.project_path = fresh.to_string_lossy().into_owned();
    persist(&row);
    let recreated = sdk
        .ensure_tool(
            &row.id,
            &agent_of_empires::daemon::EnsureToolBody {
                tool_name: "..".into(),
                size: None,
                adoption: None,
            },
            &epoch,
        )
        .await
        .unwrap();
    assert_eq!(recreated.outcome.tmux_session, name);
    assert_eq!(
        wait_for_file_contents(&second_output, Duration::from_secs(5)).trim(),
        row.project_path
    );
    assert_eq!(
        pane().trim().rsplit('\t').next(),
        Some(row.project_path.as_str())
    );
    let sent = Command::new("tmux")
        .arg("-S")
        .arg(h.home_path().join("tmux.sock"))
        .args([
            "send-keys",
            "-t",
            &format!("={host_name}:^.0"),
            "exit",
            "Enter",
        ])
        .output()
        .unwrap();
    assert!(sent.status.success());
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    loop {
        let dead = Command::new("tmux")
            .arg("-S")
            .arg(h.home_path().join("tmux.sock"))
            .args([
                "display-message",
                "-p",
                "-t",
                &format!("={host_name}:^.0"),
                "#{pane_dead}",
            ])
            .output()
            .unwrap();
        assert!(dead.status.success());
        if dead.stdout == b"1\n" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "host shell did not exit"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    row = load_committed();
    row.terminal_info = None;
    persist(&row);
    let snapshot = || async {
        http.get("http://localhost/api/runtime/snapshot")
            .send()
            .await
            .unwrap()
            .error_for_status()
            .unwrap()
            .json::<agent_of_empires::daemon::RuntimeSnapshot>()
            .await
            .unwrap()
    };
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while snapshot()
        .await
        .contents
        .sessions
        .iter()
        .find(|value| value.id == row.id)
        .unwrap()
        .has_terminal
    {
        assert!(
            std::time::Instant::now() < deadline,
            "cleared terminal flag was not reflected"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let stream = tokio::net::UnixStream::connect(app.join("daemon/api.sock"))
        .await
        .unwrap();
    let (mut websocket, response) = tokio_tungstenite::client_async(
        format!("ws://localhost/sessions/{}/terminal/live-ws", row.id),
        stream,
    )
    .await
    .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::SWITCHING_PROTOCOLS);
    assert!(
        snapshot()
            .await
            .contents
            .sessions
            .iter()
            .find(|value| value.id == row.id)
            .unwrap()
            .has_terminal,
        "WebSocket recovery granted access before publishing the terminal commit"
    );
    let revived_host = inspect_pane(host_name);
    assert_ne!(
        revived_host.split('\t').next(),
        original_host.split('\t').next()
    );
    assert_eq!(
        revived_host.trim().rsplit('\t').next(),
        Some(row.project_path.as_str())
    );
    websocket.close(None).await.unwrap();
    let mut stored: Vec<Instance> = serde_json::from_slice(&fs::read(&rows_path).unwrap()).unwrap();
    let container_row = &mut stored[0];
    container_row.sandbox_info = Some(agent_of_empires::session::SandboxInfo {
        enabled: true,
        container_id: None,
        image: "unused-existing-pane".into(),
        container_name: agent_of_empires::containers::DockerContainer::generate_name(
            &container_row.id,
        ),
        extra_env: None,
        custom_instruction: None,
        provider: None,
        container_workdir: None,
        before_start_env: Vec::new(),
        before_start_hook_env: Vec::new(),
    });
    persist(container_row);
    let container_name = container_row
        .container_terminal_tmux_session_indexed(0)
        .unwrap()
        .name()
        .to_owned();
    // Existing-pane admission must not require a container backend.
    let created = Command::new("tmux")
        .arg("-S")
        .arg(h.home_path().join("tmux.sock"))
        .args(["new-session", "-d", "-s", &container_name, "exec sleep 120"])
        .output()
        .unwrap();
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    let original_container = inspect_pane(&container_name);
    let target = sdk
        .ensure_container_terminal(
            &container_row.id,
            0,
            &agent_of_empires::daemon::StartSessionBody::default(),
            &epoch,
        )
        .await
        .unwrap();
    assert_eq!(target.outcome.tmux_session, container_name);
    assert!(matches!(
        target.outcome.status,
        agent_of_empires::daemon::TerminalTargetStatus::Exists
    ));
    let generation = container_row
        .try_acquire_lifecycle_reservation(
            LifecycleOperation::Purge,
            Instance::LIFECYCLE_RESERVATION_TTL,
            chrono::Utc::now(),
        )
        .unwrap();
    persist(container_row);
    let refused = http
        .post(format!(
            "http://localhost/api/sessions/{}/container-terminal",
            container_row.id
        ))
        .header(agent_of_empires::daemon::RUNTIME_EPOCH_HEADER, &epoch)
        .json(&agent_of_empires::daemon::StartSessionBody::default())
        .send()
        .await
        .unwrap();
    assert_eq!(refused.status(), reqwest::StatusCode::CONFLICT);
    let stream = tokio::net::UnixStream::connect(app.join("daemon/api.sock"))
        .await
        .unwrap();
    let websocket = tokio_tungstenite::client_async(
        format!(
            "ws://localhost/sessions/{}/container-terminal/live-ws",
            container_row.id
        ),
        stream,
    )
    .await;
    let Err(tokio_tungstenite::tungstenite::Error::Http(response)) = websocket else {
        panic!("a reserved purge admitted a container-terminal WebSocket");
    };
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
    assert_eq!(inspect_pane(&container_name), original_container);
    container_row.release_lifecycle_reservation_if_owned(LifecycleOperation::Purge, generation);
    container_row.sandbox_info = None;
    persist(container_row);
}
