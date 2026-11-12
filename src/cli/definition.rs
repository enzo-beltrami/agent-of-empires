//! CLI argument definitions for documentation generation

use clap::{Parser, Subcommand};
use clap_complete::Shell;

use super::acp::AcpCommands;
use super::add::AddArgs;
use super::cityhall::CityHallCommands;
use super::extract_session_id::ExtractSessionIdArgs;
use super::group::GroupCommands;
use super::hooks::HooksCommands;
use super::init::InitArgs;
use super::killall::KillallArgs;
use super::list::ListArgs;
use super::log_level::LogLevelArgs;
use super::logs::LogsArgs;
use super::mcp::McpCommands;
use super::plugin::PluginCommands;
use super::profile::ProfileCommands;
use super::project::ProjectCommands;
use super::ps::PsArgs;
use super::remove::RemoveArgs;
use super::sandbox::SandboxCommands;
use super::send::SendArgs;
use super::serve::ServeArgs;
use super::session::SessionCommands;
use super::settings::SettingsCommands;
use super::skill::SkillCommands;
use super::sounds::SoundsCommands;
use super::status::StatusArgs;
use super::theme::ThemeCommands;
use super::tmux::TmuxCommands;
use super::uninstall::UninstallArgs;
use super::url::UrlArgs;
use super::worktree::WorktreeCommands;

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(name = "aoe")]
#[command(about = "Terminal session manager for AI coding agents")]
#[command(version = VERSION)]
#[command(
    long_about = "Agent of Empires (aoe) is a terminal session manager that uses tmux to help \
    you manage and monitor AI coding agents like Claude Code and OpenCode.\n\n\
    Run `aoe serve` to start the daemon and web dashboard."
)]
pub struct Cli {
    /// Profile to use (separate workspace with its own sessions). Commands that
    /// consume or create profile state require an existing profile: an unknown
    /// name is refused, not created (make one with `aoe profile create`).
    /// Profile-independent commands such as `list --all` and `serve --stop`
    /// ignore it. `AGENT_OF_EMPIRES_PROFILE` names one too, and an explicit
    /// `-p` wins over it.
    // No env attribute: explicit and environment profiles are tracked separately.
    #[arg(short = 'p', long, global = true)]
    pub profile: Option<String>,

    /// Select a daemon endpoint for read-only CLI commands.
    ///
    /// Explicit endpoints and `AOE_DAEMON_URL` never fall back to local storage
    /// or start a local daemon. Authenticate with `AOE_DAEMON_TOKEN` or
    /// `AOE_DAEMON_PASSPHRASE`. An unauthenticated daemon needs neither.
    /// See the HTTP API reference for transport and authentication rules.
    // Keep the explicit flag separate from the optional environment endpoint.
    #[arg(long, global = true)]
    pub daemon_url: Option<String>,

    /// Fail instead of starting a local daemon when none is running
    ///
    /// Also read from `AOE_NO_AUTOSTART=1`. Has no effect with `--daemon-url`
    /// or `AOE_DAEMON_URL`, which never start a daemon.
    #[arg(
        long,
        global = true,
        env = "AOE_NO_AUTOSTART",
        value_parser = clap::builder::BoolishValueParser::new()
    )]
    pub no_autostart: bool,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Add a new session
    Add(Box<AddArgs>),

    /// List supported agents and their install status
    Agents,

    /// Initialize .agent-of-empires/config.toml in a repository
    Init(InitArgs),

    /// List all sessions
    #[command(alias = "ls")]
    List(ListArgs),

    /// Show a substrate-agnostic runtime view of in-flight sessions
    /// (tmux agent panes and ACP structured-view workers), one row each.
    Ps(PsArgs),

    /// View the configured AoE log file with a pretty viewer
    Logs(LogsArgs),

    /// Get or set the running daemon's log filter at runtime.
    /// Pass a bare level (debug/info/...) for the safe expansion, or
    /// `--filter <expr>` for raw EnvFilter syntax. `--get` prints the
    /// current filter. Changes are ephemeral and lost on daemon restart.
    LogLevel(LogLevelArgs),

    /// Remove a session
    #[command(alias = "rm")]
    Remove(RemoveArgs),

    /// Send a message to a running agent session
    Send(SendArgs),

    /// Show session status summary
    ///
    /// JSON takes precedence over quiet, which takes precedence over verbose.
    Status(StatusArgs),

    /// Force-stop everything aoe is running: the serve daemon, all agent
    /// workers, and all aoe tmux sessions. Destructive and unprompted.
    Killall(KillallArgs),

    /// Internal: trap for `aoe stop`, which is not a command in aoe (stopping
    /// is always scoped to a noun). Redirects users to `session stop`,
    /// `acp stop`, `serve --stop`, or `killall`. Hidden from help.
    #[command(name = "stop", hide = true)]
    Stop {
        /// Swallow any args the user typed (e.g. a session id) so the trap
        /// fires instead of clap erroring on an unexpected positional.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },

    /// Manage session lifecycle (start, stop, attach, etc.)
    Session {
        #[command(subcommand)]
        command: SessionCommands,
    },

    /// Manage groups for organizing sessions
    Group {
        #[command(subcommand)]
        command: GroupCommands,
    },

    /// List, enable and disable the built-in plugins
    Plugin {
        #[command(subcommand)]
        command: PluginCommands,
    },

    /// Manage profiles (separate workspaces)
    Profile {
        #[command(subcommand)]
        command: Option<ProfileCommands>,
    },

    /// Manage the project registry used by multi-repo session pickers
    Project {
        #[command(subcommand)]
        command: ProjectCommands,
    },

    /// Inspect and reclaim per-session sandbox agent stores
    Sandbox {
        #[command(subcommand)]
        command: SandboxCommands,
    },

    /// Manage git worktrees for parallel development
    Worktree {
        #[command(subcommand)]
        command: WorktreeCommands,
    },

    /// tmux integration utilities
    Tmux {
        #[command(subcommand)]
        command: TmuxCommands,
    },

    /// Manage sound effects for agent state transitions
    Sounds {
        #[command(subcommand)]
        command: SoundsCommands,
    },

    /// Manage color themes (list, export, customize)
    Theme {
        #[command(subcommand)]
        command: ThemeCommands,
    },

    /// Inspect resolved settings and their provenance
    Settings {
        #[command(subcommand)]
        command: SettingsCommands,
    },

    /// Export and apply the CityHall config bundle (settings + projects)
    Cityhall {
        #[command(subcommand)]
        command: CityHallCommands,
    },

    /// Inspect the effective MCP server set (provenance, conflicts, drift)
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },

    /// Query and manage agent skills
    Skill {
        #[command(subcommand)]
        command: SkillCommands,
    },

    /// Let AoE write agent hooks into each agent's own config, for every
    /// agent and every profile
    Hooks {
        #[command(subcommand)]
        command: HooksCommands,
    },

    /// Start the aoe daemon: REST/WebSocket API, plus the web dashboard in
    /// builds that embed it
    Serve(ServeArgs),

    /// Control the background aoe daemon; each verb is the matching `aoe serve`
    /// flag (`--daemon`, `--stop`, `--status`, `--restart`)
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },

    /// Print the URL of a running `aoe serve` daemon
    Url(UrlArgs),

    /// Manage the ACP structured-view workers (doctor, ps, logs, prompt, approve, ...).
    Acp {
        #[command(subcommand)]
        command: AcpCommands,
    },

    /// Internal: per-acp-worker shim spawned by `aoe serve`. Owns the
    /// agent subprocess and outlives the daemon so workers survive
    /// `aoe serve --stop`. Hidden from help.
    #[command(name = "__acp-runner", hide = true)]
    AcpRunner(Box<crate::process::runner::AcpRunnerArgs>),

    /// Internal: own the native Prime daemon and its frontend.
    #[command(name = "__prime-agent", hide = true)]
    PrimeRunner(crate::process::prime::PrimeRunnerArgs),

    /// Internal: extract Claude's `session_id` from a hook stdin payload
    /// and write it to the sidecar file. Spawned by the host-side
    /// `SessionStart`/`UserPromptSubmit` hook. Hidden from help.
    #[command(name = "__extract-session-id", hide = true)]
    ExtractSessionId(ExtractSessionIdArgs),

    /// Uninstall Agent of Empires
    Uninstall(UninstallArgs),

    /// Run pending migrations or restore complete original lifecycle authority.
    /// Stop the daemon with `aoe serve --stop` first. By default, also move
    /// eligible sandbox stores and retry deferred ownership recovery.
    Migrate(crate::cli::migrate::MigrateArgs),

    /// Generate shell completions
    Completion {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Subcommand, Clone, Copy)]
pub enum DaemonCommands {
    /// Start the daemon in the background (`aoe serve --daemon`)
    Start,
    /// Stop the running daemon (`aoe serve --stop`)
    Stop,
    /// Print the daemon's PID, mode, URLs and log path, or remote reachability with `AOE_DAEMON_URL` (`aoe serve --status`)
    Status,
    /// Restart the daemon with its persisted launch config (`aoe serve --restart`)
    Restart,
}

impl Commands {
    /// Rewrites `aoe daemon <verb>` into the equivalent `aoe serve` command so
    /// both spellings share every startup and gating path.
    pub fn resolve_daemon_alias(self) -> Self {
        let Commands::Daemon { command } = self else {
            return self;
        };
        let flag = match command {
            DaemonCommands::Start => "--daemon",
            DaemonCommands::Stop => "--stop",
            DaemonCommands::Status => "--status",
            DaemonCommands::Restart => "--restart",
        };
        let args = ServeCli::parse_from(["serve", flag]).args;
        Commands::Serve(args)
    }
}

#[derive(Parser)]
struct ServeCli {
    #[command(flatten)]
    args: ServeArgs,
}

/// False for hidden machine-spawned subcommands, which must never print
/// migration progress or config warnings into a worker's redirected stderr.
pub fn is_user_command(command: &Commands) -> bool {
    !matches!(
        command,
        Commands::Stop { .. }
            | Commands::AcpRunner(_)
            | Commands::PrimeRunner(_)
            | Commands::ExtractSessionId(_)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[test]
    fn daemon_verbs_resolve_to_the_serve_lifecycle_flags() {
        for (verb, expect) in [
            ("start", "--daemon"),
            ("stop", "--stop"),
            ("status", "--status"),
            ("restart", "--restart"),
        ] {
            let alias = Cli::try_parse_from(["aoe", "daemon", verb]).expect("parse");
            let Some(Commands::Serve(got)) = alias.command.map(Commands::resolve_daemon_alias)
            else {
                panic!("daemon {verb} did not resolve to serve");
            };
            let Some(Commands::Serve(want)) = Cli::try_parse_from(["aoe", "serve", expect])
                .expect("parse")
                .command
            else {
                unreachable!()
            };
            assert_eq!(
                (got.daemon, got.stop, got.status, got.restart),
                (want.daemon, want.stop, want.status, want.restart),
                "daemon {verb}"
            );
            assert_eq!(got.resolved_port(), want.resolved_port());
            assert_eq!(got.host, want.host);
        }
    }

    #[test]
    fn machine_spawned_subcommands_are_not_user_commands() {
        for (argv, expected) in [
            (&["aoe", "add", "demo"][..], true),
            (&["aoe", "session", "current"][..], true),
            (&["aoe", "__extract-session-id"][..], false),
        ] {
            let cli = Cli::try_parse_from(argv).expect("parse");
            assert_eq!(
                is_user_command(cli.command.as_ref().expect("command")),
                expected,
                "argv {argv:?}"
            );
        }
    }

    #[test]
    fn restart_tool_and_switch_account_parse() {
        let Commands::Session {
            command: crate::cli::session::SessionCommands::Restart(args),
        } = Cli::try_parse_from([
            "aoe",
            "session",
            "restart",
            "demo",
            "--tool",
            "claude-b",
            "--to-profile",
            "work",
        ])
        .expect("parse")
        .command
        .expect("command")
        else {
            panic!("not a session restart");
        };
        assert_eq!(args.identifier.as_deref(), Some("demo"));
        assert_eq!(args.tool.as_deref(), Some("claude-b"));
        assert_eq!(args.to_profile.as_deref(), Some("work"));

        for argv in [
            ["aoe", "session", "restart", "--all", "--tool", "claude-b"],
            ["aoe", "session", "restart", "--tool", "claude-b", "--all"],
        ] {
            assert!(Cli::try_parse_from(argv).is_err(), "{argv:?}");
        }

        let Commands::Acp {
            command: crate::cli::acp::AcpCommands::SwitchAccount { session, tool },
        } = Cli::try_parse_from(["aoe", "acp", "switch-account", "abc123", "claude-b"])
            .expect("parse")
            .command
            .expect("command")
        else {
            panic!("not a switch-account");
        };
        assert_eq!((session.as_str(), tool.as_str()), ("abc123", "claude-b"));
        assert!(Cli::try_parse_from(["aoe", "acp", "switch-account", "abc123"]).is_err());
    }
}
