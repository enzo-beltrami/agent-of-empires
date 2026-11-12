//! E2E coverage for the built-in plugin toggle: `plugin list` shows the
//! built-in plugins with version and state, enable/disable round-trips through
//! config, and `aoe serve` refuses to start while `aoe.web` is disabled.
//!
//! Compiled only under `web`: the sole bundled plugin is the `web`-gated
//! `aoe.web`, so without that feature the builtin set is empty and there is
//! nothing for these management-surface tests to exercise. The bare-core
//! `--no-default-features` e2e leg skips this module by design.
#![cfg(feature = "web")]

use serial_test::parallel;

use crate::harness::TuiTestHarness;

fn web_row(h: &TuiTestHarness) -> String {
    let stdout = h.run_cli_ok(&["plugin", "list"]);
    stdout
        .lines()
        .find(|l| l.contains("aoe.web"))
        .unwrap_or_else(|| panic!("missing aoe.web row:\n{stdout}"))
        .to_string()
}

/// `plugin list` shows the builtin `aoe.web` with version and state, and
/// disable/enable flip the listed state. Unknown ids error and name the fix.
#[test]
#[parallel]
fn test_plugin_cli_lists_describes_and_toggles_builtins() {
    let mut h = TuiTestHarness::new_in_tmp("plugin_cli");
    h.stop_daemon_on_drop();
    let list = h.run_cli_ok(&["plugin", "list"]);
    assert!(
        list.contains("ID") && list.contains("VERSION") && list.contains("STATE"),
        "missing header line:\n{list}"
    );
    let row = web_row(&h);
    assert!(row.contains("1.0.0") && row.contains("enabled"), "{row}");

    h.run_cli_ok(&["plugin", "disable", "aoe.web"]);
    assert!(web_row(&h).contains("disabled"));
    h.run_cli_ok(&["plugin", "enable", "aoe.web"]);
    let row = web_row(&h);
    assert!(
        row.contains("enabled") && !row.contains("disabled"),
        "{row}"
    );

    let stderr = h.run_cli_err(&["plugin", "enable", "acme.nope"]);
    assert!(stderr.contains("unknown plugin"), "{stderr}");
}

/// `aoe.web` is a default plugin; disabling it must turn off the serve surface
/// at runtime. A fresh `aoe serve` start is then rejected as an unrecognized
/// subcommand before any daemon spawn, and re-enabling restores it.
#[test]
#[parallel]
fn test_serve_refuses_when_web_plugin_disabled() {
    let mut h = TuiTestHarness::new_in_tmp("plugin_serve_gate");
    h.stop_daemon_on_drop();
    let free_port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
        .to_string();

    let disable = h.run_cli(&["plugin", "disable", "aoe.web"]);
    assert!(disable.status.success(), "disable aoe.web failed");

    let refused = h.run_cli(&["serve", "--daemon", "--port", &free_port, "--no-auth"]);
    assert!(
        !refused.status.success(),
        "serve must refuse while aoe.web is disabled"
    );
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains("unrecognized subcommand 'serve'"),
        "refusal must read as an unrecognized subcommand:\n{}",
        String::from_utf8_lossy(&refused.stderr)
    );

    let enable = h.run_cli(&["plugin", "enable", "aoe.web"]);
    assert!(enable.status.success(), "enable aoe.web failed");
    // The disable auto-started a Daemon; stop it so the explicit start below binds its own port.
    assert!(h.run_cli(&["serve", "--stop"]).status.success());

    let started = h.run_cli(&["serve", "--daemon", "--port", &free_port, "--no-auth"]);
    assert!(
        started.status.success(),
        "serve must start once aoe.web is enabled:\n{}",
        String::from_utf8_lossy(&started.stderr)
    );
    // `serve --daemon` returns once the child is spawned, before it has finished
    // binding the port and writing serve.pid. Stopping in that window races the
    // startup, so wait for the port to accept a connection first (mirrors the
    // serve.rs lifecycle test).
    let port: u16 = free_port.parse().unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while std::net::TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            std::time::Instant::now() < deadline,
            "daemon never bound port {port} after enabling aoe.web"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let stopped = h.run_cli(&["serve", "--stop"]);
    assert!(stopped.status.success(), "serve --stop must succeed");
}
