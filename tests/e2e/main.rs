//! End-to-end tests for Agent of Empires.
//!
//! These tests exercise the full `aoe` binary -- both TUI mode (via tmux) and
//! CLI subcommands (via subprocess). They catch startup failures, rendering
//! bugs, config resolution errors, and full-flow regressions that unit and
//! integration tests miss.
//!
//! # Running
//!
//! ```sh
//! cargo test --features e2e-tests --test e2e              # run all e2e tests
//! cargo test --features e2e-tests --test e2e -- --nocapture  # with screen dumps on failure
//! ```
//!
//! TUI tests require tmux and are skipped automatically if it is not installed.
//! Docker-dependent tests are `#[ignore]` and require a running Docker daemon.

mod harness;

mod acp_live_e2e;
mod add_project_cli;
mod archive_restore;
mod archive_structured;
mod autostart;
mod cli;
mod cli_session_id_capture;
mod daemon_config;
mod errors;
mod fake_agent;
mod fork_cli;
mod fork_structured_e2e;
mod kiro_launch;
mod logs;
mod mcp_config;
mod opencode_preassign_no_runtime_panic;
mod opencode_store_fork;
mod pi_fork_e2e;
mod pinned_session_id_e2e;
mod plugins;
mod profile_lazy_creation;
mod project_registry;
mod purge_restore_race;
#[cfg(target_os = "linux")]
mod remove_structured_runner;
mod restart_structured;
mod resume_fallback;
mod sandbox;
mod send_structured_e2e;
mod serve;
mod serve_lock;
mod session_wait;
mod settings;
#[cfg(target_os = "linux")]
mod systemd_scope;
mod tool_sessions;
mod update_command;
