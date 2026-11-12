//! Structured sessions against a live daemon and the Node fake ACP agent
//! (`web/tests/helpers/fakeAcpAgent.mjs`), whose `permission_request` entries
//! gate the turn until the client decides.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serial_test::parallel;

use crate::harness::{
    app_dir_in, pick_free_port, require_node, require_tmux, wait_for_port, wait_until,
    TuiTestHarness,
};

/// Start the fake agent with `script`, a daemon, and a structured session whose
/// worker has accepted `prompt`.
fn live_session(name: &str, script: &str, prompt: &str) -> (TuiTestHarness, String) {
    let h = TuiTestHarness::new_acp(name, script);
    let (_, session_id) = h.start_structured_session(name);
    h.prompt_until_accepted(&session_id, prompt, Duration::from_secs(30));
    (h, session_id)
}

/// Daemon session-scoped `acp.protocol` tracing is teed into the per-session
/// worker log alongside the runner marker (#1864).
#[test]
#[parallel]
fn daemon_breadcrumbs_reach_per_session_log() {
    require_tmux!();
    require_node!();
    let script = r#"{ "turns": [ { "updates": [], "stopReason": "end_turn" } ] }"#;
    let (h, session_id) = live_session("acp_session_log_tee", script, "hello");

    let body = wait_until(Duration::from_secs(10), Duration::from_millis(250), || {
        let out = h.run_cli(&["acp", "logs", "--session", &session_id]);
        let body = String::from_utf8_lossy(&out.stdout).to_string();
        if body.contains("initializing ACP agent") {
            Ok(body)
        } else {
            Err(format!("`aoe acp logs` lacks the handshake:\n{body}"))
        }
    });
    assert!(body.contains("runner.startup"), "runner marker:\n{body}");
    assert!(body.contains("acp.protocol"), "daemon target:\n{body}");
}

/// The daemon fails the first fresh-spawn handshake while the runner stays
/// registered; the reconciler must readopt it so prompts are accepted (#1890).
#[test]
#[parallel]
fn acp_recovers_orphaned_runner_after_failed_first_handshake() {
    require_tmux!();
    require_node!();
    let mut h = TuiTestHarness::new_acp(
        "acp_orphan_recovery",
        r#"{ "turns": [ { "updates": [], "stopReason": "end_turn" } ] }"#,
    );
    h.set_env("AOE_ACP_TEST_FAIL_FIRST_HANDSHAKES", "1");
    let (_, session_id) = h.start_structured_session("orphan-recovery");

    // 404 or 503 `worker_not_ready` are the expected transients; anything
    // else fails fast rather than timing out.
    wait_until(Duration::from_secs(45), Duration::from_millis(250), || {
        let out = h.run_cli(&["acp", "prompt", &session_id, "please proceed"]);
        if out.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("not found on the daemon") || stderr.contains("worker_not_ready"),
            "acp prompt failed with an unexpected error before recovery.\nstderr: {stderr}"
        );
        Err(format!("worker never recovered.\nstderr: {stderr}"))
    });
}

fn env_value(capture: &str, key: &str) -> Option<String> {
    capture
        .lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .map(str::to_owned)
}

/// Wait for the adapter invocation that started with `key=expected`; other
/// shim invocations write their own captures and are skipped. Outlasts the
/// harness's 60s runner socket timeout.
fn wait_for_capture(dir: &Path, key: &str, expected: &str) -> String {
    wait_until(Duration::from_secs(75), Duration::from_millis(100), || {
        let seen = captures(dir);
        seen.iter()
            .find(|c| env_value(c, key).as_deref() == Some(expected))
            .cloned()
            .ok_or_else(|| {
                let observed: Vec<_> = seen.iter().map(|c| env_value(c, key)).collect();
                format!("no adapter started with {key}={expected}; observed {observed:?}")
            })
    })
}

fn captures(dir: &Path) -> Vec<String> {
    capture_files(dir)
        .into_iter()
        .map(|(_, contents)| contents)
        .collect()
}

fn capture_files(dir: &Path) -> Vec<(PathBuf, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            std::fs::read_to_string(&path)
                .ok()
                .map(|contents| (path, contents))
        })
        .collect()
}

fn asserted_claude_store_harness(name: &str) -> (TuiTestHarness, PathBuf, PathBuf, String) {
    let mut h = TuiTestHarness::new_in_tmp(name);
    h.stop_daemon_on_drop();
    let selected = h.home_path().join("selected-claude");
    let configured = h.home_path().join("configured-claude");
    let project = h.project_path().canonicalize().unwrap();
    let script = h.home_path().join("agent.json");
    std::fs::write(&script, r#"{"turns":[]}"#).unwrap();
    let capture_dir = h.home_path().join("adapter-env");
    h.install_acp_shim_capturing_env(&script, &capture_dir);

    let config_path = app_dir_in(h.home_path()).join("config.toml");
    let config = std::fs::read_to_string(&config_path).unwrap();
    std::fs::write(
        &config_path,
        format!(
            "environment = [\"CLAUDE_CONFIG_DIR={}\"]\n{config}",
            configured.display()
        ),
    )
    .unwrap();
    let add = h.run_cli(&[
        "add",
        project.to_str().unwrap(),
        "-t",
        "handoff",
        "-c",
        "claude",
    ]);
    assert!(
        add.status.success(),
        "{}",
        String::from_utf8_lossy(&add.stderr)
    );

    let sid = "11111111-1111-4111-8111-111111111111";
    let encoded: String = project
        .to_string_lossy()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let transcripts = selected.join("projects").join(encoded);
    std::fs::create_dir_all(&transcripts).unwrap();
    std::fs::write(transcripts.join(format!("{sid}.jsonl")), "{}\n").unwrap();
    let pin = h.run_cli(&[
        "session",
        "set-session-id",
        "handoff",
        sid,
        "--store",
        selected.to_str().unwrap(),
    ]);
    assert!(
        pin.status.success(),
        "{}",
        String::from_utf8_lossy(&pin.stderr)
    );
    // Stores are recorded canonically; macOS `/tmp` resolves to `/private/tmp`.
    let selected = selected.canonicalize().unwrap();

    let sessions_path = app_dir_in(h.home_path()).join("profiles/default/sessions.json");
    let rows: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&sessions_path).unwrap()).unwrap();
    let id = rows
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["title"] == "handoff")
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    (h, selected, capture_dir, id)
}

fn start_daemon(h: &TuiTestHarness) -> u16 {
    let port = pick_free_port();
    let start = h.run_cli(&[
        "serve",
        "--daemon",
        "--port",
        &port.to_string(),
        "--no-auth",
    ]);
    assert!(
        start.status.success(),
        "{}",
        String::from_utf8_lossy(&start.stderr)
    );
    assert!(wait_for_port(port, Duration::from_secs(10)));
    port
}

fn wait_for_session_loads(log: &Path, minimum: usize) -> usize {
    wait_until(Duration::from_secs(75), Duration::from_millis(50), || {
        let contents = std::fs::read_to_string(log).unwrap_or_default();
        let loads = contents
            .matches("handleRequest method=session/load")
            .count();
        (loads >= minimum)
            .then_some(loads)
            .ok_or_else(|| format!("worker did not load the session: {contents}"))
    })
}

#[test]
#[parallel]
fn selected_claude_store_survives_stop_and_respawn() {
    require_tmux!();
    require_node!();
    let (h, selected, capture_dir, id) = asserted_claude_store_harness("handoff_respawn");
    let port = start_daemon(&h);
    let base = format!("http://127.0.0.1:{port}/api/sessions/{id}");
    let log = app_dir_in(h.home_path()).join("fake-acp.log");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let response = reqwest::Client::new()
            .post(format!("{base}/acp/enable"))
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "{}",
            response.text().await.unwrap()
        );
    });
    assert_eq!(wait_for_session_loads(&log, 1), 1);
    let original: agent_of_empires::process::worker_registry::WorkerRecord =
        serde_json::from_slice(
            &std::fs::read(
                app_dir_in(h.home_path())
                    .join("acp-workers")
                    .join(format!("{id}.json")),
            )
            .unwrap(),
        )
        .unwrap();
    let original_identity =
        agent_of_empires::acp::runner_lifecycle::RunnerIdentity::from_record(&original);
    assert!(original_identity.birth_fingerprint.is_some());
    let captures_before: Vec<_> = capture_files(&capture_dir)
        .into_iter()
        .map(|(path, _)| path.file_name().unwrap().to_owned())
        .collect();

    runtime.block_on(async {
        let client = reqwest::Client::new();
        let response = client.delete(format!("{base}/acp")).send().await.unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
        let stopped: agent_of_empires::session::Instance = serde_json::from_value(
            h.read_sessions()
                .as_array()
                .and_then(|rows| rows.iter().find(|row| row["id"] == id))
                .cloned()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(stopped.status, agent_of_empires::session::Status::Stopped);
        assert!(stopped.is_idle_dormant());
        use agent_of_empires::acp::runner_lifecycle::ProcessControl as _;
        assert!(
            !agent_of_empires::acp::runner_lifecycle::SystemProcessControl::default()
                .is_alive(original_identity)
        );
        let response = client
            .post(format!("{base}/acp/spawn"))
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "{}",
            response.text().await.unwrap()
        );
    });
    assert_eq!(wait_for_session_loads(&log, 2), 2);
    let respawned = capture_files(&capture_dir)
        .into_iter()
        .any(|(path, capture)| {
            !captures_before.contains(&path.file_name().unwrap().to_owned())
                && env_value(&capture, "CLAUDE_CONFIG_DIR").as_deref()
                    == Some(selected.to_str().unwrap())
        });
    assert!(
        respawned,
        "respawned adapter must use {}",
        selected.display()
    );
}

/// Configured `environment` entries and the desktop session layer reach a
/// structured adapter, read from the env the shim captured just before exec.
/// The daemon's ambient values are deliberately wrong so an allowlist
/// forward cannot pass, and `AOE_TOKEN` is always refused (#3262).
#[test]
#[parallel]
fn configured_host_environment_reaches_structured_worker() {
    require_tmux!();
    require_node!();
    let mut h = TuiTestHarness::new_in_tmp("acp_host_env");
    h.stop_daemon_on_drop();

    let home = h.home_path().to_path_buf();
    let expected_codex_home = home.join("configured-codex-home");
    let expected_git_config = home.join("configured-gitconfig");
    for (key, value) in [
        ("CODEX_HOME", home.join("wrong-ambient-codex-home")),
        ("GIT_CONFIG_GLOBAL", home.join("wrong-ambient-gitconfig")),
    ] {
        h.set_env(key, value.to_str().unwrap());
    }
    h.set_env("DISPLAY", ":42");
    h.set_env("XDG_RUNTIME_DIR", "/run/user/4242");
    // Stays out while `session.inherit_host_environment` is off.
    h.set_env("GOPATH", "/scratch/gopath");

    let config_path = app_dir_in(&home).join("config.toml");
    let seeded = std::fs::read_to_string(&config_path).expect("read seeded config");
    std::fs::write(
        &config_path,
        format!(
            "environment = [\n  \"CODEX_HOME={}\",\n  \"GIT_CONFIG_GLOBAL={}\",\n  \"AOE_TOKEN=must-not-reach-agent\",\n]\n\n{seeded}",
            expected_codex_home.display(),
            expected_git_config.display(),
        ),
    )
    .expect("write host environment config");

    let fake_script = home.join("host-env-script.json");
    std::fs::write(&fake_script, r#"{"turns":[]}"#).expect("write fake-acp script");
    let capture_dir = home.join("adapter-env");
    h.install_acp_shim_capturing_env(&fake_script, &capture_dir);
    // The reconciler spawns the adapter without a prompt.
    h.start_structured_session("HostEnv");

    let capture = wait_for_capture(
        &capture_dir,
        "CODEX_HOME",
        expected_codex_home.to_str().unwrap(),
    );

    for (key, expected) in [
        ("GIT_CONFIG_GLOBAL", expected_git_config.to_str().unwrap()),
        ("DISPLAY", ":42"),
        ("XDG_RUNTIME_DIR", "/run/user/4242"),
    ] {
        assert_eq!(env_value(&capture, key).as_deref(), Some(expected), "{key}");
    }
    assert_eq!(env_value(&capture, "GOPATH"), None);
    for capture in captures(&capture_dir) {
        assert_eq!(env_value(&capture, "AOE_TOKEN"), None);
    }
}

/// `host_hooks.before_session` output reaches a structured adapter and beats
/// both the daemon's ambient value and a same-keyed static `environment` entry,
/// the opposite precedence from `before_start`. `AOE_TOKEN` is still refused.
#[test]
#[parallel]
fn before_session_mints_environment_for_structured_worker() {
    require_tmux!();
    require_node!();
    let mut h = TuiTestHarness::new_in_tmp("before_session_env");
    h.stop_daemon_on_drop();

    let home = h.home_path().to_path_buf();
    let minted_codex_home = home.join("minted-codex-home");
    let minted_git_config = home.join("minted-gitconfig");
    let static_codex_home = home.join("static-codex-home");
    h.set_env(
        "CODEX_HOME",
        home.join("wrong-ambient-codex-home").to_str().unwrap(),
    );

    let config_path = app_dir_in(&home).join("config.toml");
    let seeded = std::fs::read_to_string(&config_path).expect("read seeded config");
    std::fs::write(
        &config_path,
        format!(
            "environment = [\n  \"CODEX_HOME={static_codex}\",\n]\n\n\
             {seeded}\n\n\
             [host_hooks]\n\
             before_session = [\n  \
               \"echo CODEX_HOME={minted_codex}\",\n  \
               \"echo GIT_CONFIG_GLOBAL={minted_git}\",\n  \
               \"echo AOE_TOKEN=must-not-reach-agent\",\n\
             ]\n",
            static_codex = static_codex_home.display(),
            minted_codex = minted_codex_home.display(),
            minted_git = minted_git_config.display(),
        ),
    )
    .expect("write before_session config");

    let fake_script = home.join("before-session-script.json");
    std::fs::write(&fake_script, r#"{"turns":[]}"#).expect("write fake-acp script");
    let capture_dir = home.join("adapter-env");
    h.install_acp_shim_capturing_env(&fake_script, &capture_dir);
    h.start_structured_session("BeforeSession");

    // Only a minting invocation carries GIT_CONFIG_GLOBAL, so waiting on it
    // keeps the CODEX_HOME comparison a real assertion.
    let capture = wait_for_capture(
        &capture_dir,
        "GIT_CONFIG_GLOBAL",
        minted_git_config.to_str().unwrap(),
    );
    assert_eq!(
        env_value(&capture, "CODEX_HOME").as_deref(),
        Some(minted_codex_home.to_str().unwrap()),
        "minted CODEX_HOME must win over static {} and ambient values",
        static_codex_home.display(),
    );
    for capture in captures(&capture_dir) {
        assert_eq!(env_value(&capture, "AOE_TOKEN"), None);
    }
}
