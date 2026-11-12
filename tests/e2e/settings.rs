//! E2E coverage for the Settings TUI.

use serial_test::parallel;

use crate::harness::{app_dir_in, TuiTestHarness};

/// `aoe settings explain` answers from `config.toml`, so it has to see the
/// migrated file. Asked first on a pre-v009 install it used to read the
/// pre-migration one and report the schema default for a value the user did
/// set, with the legacy key listed as unrecognized.
#[test]
#[parallel]
fn settings_explain_runs_pending_migrations_before_reporting_a_value() {
    use std::fs;

    let h = TuiTestHarness::new("settings_explain_migrates");
    let app = app_dir_in(h.home_path());
    fs::write(
        app.join("config.toml"),
        "[updates]\ncheck_enabled = false\n",
    )
    .unwrap();
    fs::write(app.join(".schema_version"), "8").unwrap();

    let output = h.run_cli(&["settings", "explain", "updates.update_check_mode"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "exit={:?}\n{stdout}\n{stderr}",
        output.status
    );
    assert!(
        stdout.contains("updates.update_check_mode = \"off\""),
        "expected the persisted value v009 migrates `check_enabled = false` into, got:\n{stdout}"
    );
    assert!(
        stdout.contains("source: user value"),
        "the value must be attributed to config.toml, not to the schema, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("source: schema default"),
        "the schema default is what this command used to answer with:\n{stdout}"
    );
}

/// The other direction: a value a pending migration *corrects*. v026 repoints
/// the seeded `acp.default_agent = "aoe-agent"` at the current default, so
/// asked first on a pre-v026 install `explain` names an agent this build
/// cannot start.
#[test]
#[parallel]
fn settings_explain_does_not_report_a_value_a_pending_migration_would_replace() {
    use std::fs;

    let h = TuiTestHarness::new("settings_explain_v026");
    let app = app_dir_in(h.home_path());
    fs::write(
        app.join("config.toml"),
        "[acp]\ndefault_agent = \"aoe-agent\"\n",
    )
    .unwrap();
    fs::write(app.join(".schema_version"), "25").unwrap();

    let stdout = h.run_cli_ok(&["settings", "explain", "acp.default_agent"]);

    assert!(
        !stdout.contains("\"aoe-agent\""),
        "v026 repoints that agent at every startup; the first command after an \
         upgrade must not report the value it is about to replace:\n{stdout}"
    );
    assert!(
        stdout.contains("acp.default_agent = \"claude-code\""),
        "expected the value the current schema resolves to, got:\n{stdout}"
    );
}
