//! xtask: development tasks for agent-of-empires.

use clap::{Args, CommandFactory, Parser, Subcommand};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "Development tasks for agent-of-empires")]
struct Xtask {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate CLI documentation from clap definitions
    GenDocs,
    /// Generate the web dashboard's setting-name types from the settings schema
    GenSettingsKeys,
    /// Generate the web dashboard's live-stream wire types from the Rust ones
    GenLiveWire,
    /// Run the web dashboard backend and Vite dev server together (Ctrl-C stops both)
    Dev(DevArgs),
}

#[derive(Args)]
struct DevArgs {
    /// Port for the `aoe serve` backend (matches the debug-build default)
    #[arg(long, default_value_t = 8081)]
    serve_port: u16,
    /// Port for the Vite dev server
    #[arg(long, default_value_t = 5173)]
    web_port: u16,
    /// Interface the Vite dev server binds to. Defaults to loopback; pass
    /// `0.0.0.0` to reach the dashboard from other devices on the network. The
    /// backend stays on 127.0.0.1 and is reached through Vite's proxy.
    #[arg(long, default_value = "127.0.0.1")]
    host: String,
    /// Watch `src/**`, `Cargo.toml`, and `Cargo.lock`; on change rebuild and
    /// restart `aoe serve` (Vite stays up). Unix-only, same as the base command.
    #[arg(long)]
    watch: bool,
}

fn main() {
    let args = Xtask::parse();
    match args.command {
        Commands::GenDocs => generate_cli_docs(),
        Commands::GenSettingsKeys => generate_settings_keys(),
        Commands::GenLiveWire => generate_live_wire(),
        Commands::Dev(dev) => run_dev(dev),
    }
}

#[cfg(not(unix))]
fn run_dev(_args: DevArgs) {
    eprintln!("`cargo xtask dev` is unix-only (it relies on POSIX process groups).");
    std::process::exit(1);
}

#[cfg(unix)]
fn build_web() -> bool {
    use std::process::Command;
    eprintln!("[xtask dev] building aoe --features web...");
    Command::new("cargo")
        .args(["build", "--features", "web"])
        .status()
        .map(|s| s.success())
        .unwrap_or_else(|e| {
            eprintln!("[xtask dev] failed to run cargo build: {e}");
            false
        })
}

#[cfg(unix)]
fn host_is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1") || host.starts_with("127.")
}

#[cfg(unix)]
fn child_exited(child: &mut std::process::Child) -> bool {
    matches!(child.try_wait(), Ok(Some(_)))
}

#[cfg(unix)]
fn terminate_group(child: &mut std::process::Child, grace: std::time::Duration) {
    use nix::sys::signal::{killpg, Signal};
    use nix::unistd::Pid;
    use std::time::{Duration, Instant};
    if child_exited(child) {
        let _ = child.wait();
        return;
    }
    let pid = Pid::from_raw(child.id() as i32);
    let _ = killpg(pid, Signal::SIGTERM);
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        if child_exited(child) {
            let _ = child.wait();
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let _ = killpg(pid, Signal::SIGKILL);
    let _ = child.wait();
}

#[cfg(unix)]
fn wait_for_port(port: u16, timeout: std::time::Duration) {
    use std::net::TcpListener;
    use std::time::{Duration, Instant};
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if TcpListener::bind(("127.0.0.1", port)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    eprintln!("[xtask dev] port {port} still busy after waiting; respawning anyway");
}

#[cfg(unix)]
fn is_watch_relevant(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) == Some("rs") {
        return true;
    }
    matches!(
        path.file_name().and_then(|n| n.to_str()),
        Some("Cargo.toml") | Some("Cargo.lock")
    )
}

/// Each child runs in its own process group so one Ctrl-C tears the whole tree down.
#[cfg(unix)]
fn run_dev(args: DevArgs) {
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    if !build_web() {
        std::process::exit(1);
    }

    let target_dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
    let bin = Path::new(&target_dir).join("debug").join("aoe");

    let shutdown = Arc::new(AtomicBool::new(false));
    {
        let shutdown = shutdown.clone();
        ctrlc::set_handler(move || shutdown.store(true, Ordering::SeqCst))
            .expect("failed to install Ctrl-C handler");
    }

    // Detach stdin: a background process group touching the TTY gets SIGTTOU.
    let serve_port = args.serve_port;
    // The backend stays on loopback; remote devices reach it through Vite's proxy.
    let host = args.host.clone();
    let spawn_serve = || -> Child {
        Command::new(&bin)
            .args(["serve", "--no-auth", "--port", &serve_port.to_string()])
            .stdin(Stdio::null())
            .process_group(0)
            .spawn()
            .expect("failed to spawn `aoe serve`")
    };

    // A stale dev serve PID file would make the fresh serve refuse to start.
    {
        let stopped = Command::new(&bin)
            .args(["serve", "--stop"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if stopped {
            eprintln!("[xtask dev] stopped a pre-existing dev `aoe serve` on :{serve_port}");
            wait_for_port(serve_port, Duration::from_secs(5));
        }
    }

    let mut serve: Option<Child> = Some(spawn_serve());

    let mut vite = match Command::new("npm")
        .args([
            "--prefix",
            "web",
            "run",
            "dev",
            "--",
            "--port",
            &args.web_port.to_string(),
            "--host",
            &host,
        ])
        .env(
            "VITE_PROXY",
            format!("http://127.0.0.1:{}", args.serve_port),
        )
        .stdin(Stdio::null())
        .process_group(0)
        .spawn()
    {
        Ok(child) => child,
        Err(e) => {
            eprintln!("[xtask dev] failed to spawn `npm run dev`: {e}");
            if let Some(mut serve) = serve.take() {
                terminate_group(&mut serve, Duration::from_secs(2));
            }
            std::process::exit(1);
        }
    };

    eprintln!(
        "[xtask dev] aoe serve on :{} | open http://localhost:{}{}",
        args.serve_port,
        args.web_port,
        if args.watch {
            " | watching src for changes"
        } else {
            ""
        }
    );
    if !host_is_loopback(&host) {
        eprintln!(
            "[xtask dev] WARNING: Vite is bound to {host}:{} and reachable from your \
             network. The proxied backend runs with --no-auth, so anyone who can reach \
             this port can control your agent sessions.",
            args.web_port
        );
    }

    let (watch_tx, watch_rx) = std::sync::mpsc::channel::<()>();
    let _watcher = if args.watch {
        use notify::{RecursiveMode, Watcher};
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                if event.paths.iter().any(|p| is_watch_relevant(p)) {
                    let _ = watch_tx.send(());
                }
            }
        })
        .expect("failed to create file watcher");
        // Watch the root non-recursively so an atomic-save rename of Cargo.toml is caught.
        watcher
            .watch(Path::new("src"), RecursiveMode::Recursive)
            .expect("failed to watch src/");
        watcher
            .watch(Path::new("."), RecursiveMode::NonRecursive)
            .expect("failed to watch project root");
        Some(watcher)
    } else {
        drop(watch_tx);
        None
    };

    let debounce = Duration::from_millis(300);
    let mut rebuild_at: Option<Instant> = None;

    loop {
        if shutdown.load(Ordering::SeqCst) {
            break;
        }
        if child_exited(&mut vite) {
            eprintln!("[xtask dev] vite exited; stopping `aoe serve`");
            break;
        }
        if let Some(child) = serve.as_mut() {
            if child_exited(child) {
                if args.watch {
                    eprintln!("[xtask dev] `aoe serve` exited; waiting for a change to rebuild");
                    let _ = child.wait();
                    serve = None;
                } else {
                    eprintln!("[xtask dev] `aoe serve` exited; stopping vite");
                    break;
                }
            }
        }

        if args.watch {
            let mut saw_change = false;
            while watch_rx.try_recv().is_ok() {
                saw_change = true;
            }
            if saw_change {
                rebuild_at = Some(Instant::now() + debounce);
            }
            if let Some(at) = rebuild_at {
                if Instant::now() >= at {
                    rebuild_at = None;
                    eprintln!("[xtask dev] change detected; rebuilding aoe...");
                    if build_web() {
                        if let Some(mut old) = serve.take() {
                            terminate_group(&mut old, Duration::from_secs(2));
                        }
                        wait_for_port(serve_port, Duration::from_secs(5));
                        serve = Some(spawn_serve());
                        eprintln!("[xtask dev] aoe serve restarted on :{serve_port}");
                    } else {
                        eprintln!("[xtask dev] build failed; keeping the running backend");
                    }
                }
            }
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    terminate_group(&mut vite, Duration::from_secs(2));
    if let Some(mut child) = serve.take() {
        terminate_group(&mut child, Duration::from_secs(2));
    }
}

fn generate_cli_docs() {
    let markdown = clap_markdown::help_markdown::<agent_of_empires::cli::Cli>();

    let docs_dir = Path::new("docs/cli");
    fs::create_dir_all(docs_dir).expect("Failed to create docs/cli directory");

    let output_path = docs_dir.join("reference.md");
    fs::write(&output_path, markdown).expect("Failed to write CLI reference");

    println!("Generated CLI documentation at {}", output_path.display());
}

const LIVE_WIRE_PATH: &str = "web/src/lib/generated/liveWire.ts";

fn generate_live_wire() {
    let path = Path::new(LIVE_WIRE_PATH);
    fs::create_dir_all(path.parent().unwrap()).expect("Failed to create generated directory");
    fs::write(
        path,
        agent_of_empires::daemon::live::typescript_declarations(),
    )
    .expect("Failed to write live wire types");
    println!("Generated live wire types at {}", path.display());
}

const SETTINGS_KEYS_PATH: &str = "web/src/lib/generated/settingsKeys.ts";

fn generate_settings_keys() {
    let schema = agent_of_empires::session::config::settings_schema::schema_ref();
    let ts = settings_keys_ts(
        schema
            .iter()
            .map(|d| (d.section.as_str(), d.field.as_str())),
    );
    let path = Path::new(SETTINGS_KEYS_PATH);
    fs::create_dir_all(path.parent().unwrap()).expect("Failed to create generated directory");
    fs::write(path, ts).expect("Failed to write settings keys");
    println!("Generated settings keys at {}", path.display());
}

/// TypeScript types naming every `(section, field)` pair, so the dashboard
/// cannot read a setting the server does not have.
fn settings_keys_ts<'a>(fields: impl Iterator<Item = (&'a str, &'a str)>) -> String {
    let mut sections: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (section, field) in fields {
        sections.entry(section).or_default().insert(field);
    }
    let mut out = String::from(
        "// Generated by `cargo xtask gen-settings-keys`. Do not edit.\n\nexport interface SettingsKeys {\n",
    );
    for (section, names) in &sections {
        let union: Vec<String> = names.iter().map(|n| format!("{n:?}")).collect();
        out.push_str(&format!("  {section}: {};\n", union.join(" | ")));
    }
    out.push_str(
        "}\n\nexport type SettingsSection = keyof SettingsKeys;\n\nexport type SettingsField<S extends SettingsSection> = SettingsKeys[S];\n\n/** `/api/settings` sections the schema describes, with every field optional. */\nexport type SchemaSettings = {\n  [S in SettingsSection]?: { [F in SettingsField<S>]?: unknown };\n};\n",
    );
    out
}

#[cfg(all(test, unix))]
mod tests {
    use super::is_watch_relevant;
    use std::path::Path;

    #[test]
    fn only_rust_sources_and_cargo_manifests_trigger_a_rebuild() {
        for relevant in [
            "src/main.rs",
            "src/server/mod.rs",
            "/abs/agent-of-empires/src/tui/app.rs",
            "Cargo.toml",
            "./Cargo.lock",
            "/abs/repo/Cargo.toml",
        ] {
            assert!(is_watch_relevant(Path::new(relevant)), "{relevant}");
        }
        for ignored in [
            "README.md",
            "target/debug/aoe",
            ".git/index",
            "Cargo.toml.swp",
            "web/src/App.tsx",
        ] {
            assert!(!is_watch_relevant(Path::new(ignored)), "{ignored}");
        }
    }
}

#[cfg(test)]
mod settings_keys_tests {
    use super::settings_keys_ts;

    #[test]
    fn groups_sorts_and_dedupes_fields_by_section() {
        let ts = settings_keys_ts([("b", "y"), ("a", "z"), ("a", "x"), ("a", "x")].into_iter());
        assert!(ts.contains("  a: \"x\" | \"z\";\n  b: \"y\";\n"), "{ts}");
    }
}
