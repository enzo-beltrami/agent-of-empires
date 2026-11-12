//! Acp daemon client.
//!
//! HTTP + WebSocket client for talking to an `aoe serve` daemon, used by the
//! CLI verbs (`aoe acp history`, `status`, `prompt`, `approve`, `tail`, ...).
//!
//! Clients share endpoint discovery, bounded HTTP decoding and authenticated
//! WebSocket transport. An explicit `AOE_DAEMON_URL` takes precedence; otherwise
//! discovery selects the live local daemon through its owner-verified Unix socket.
//! [`daemon_manager::locate_daemon`] never spawns. [`daemon_manager::require_daemon`]
//! may autostart a loopback daemon for CLI commands.

pub mod daemon_manager;
pub mod discovery;
pub mod http;
pub(crate) mod passphrase_session;
pub mod ws;

pub use daemon_manager::{ensure_daemon, locate_daemon, require_daemon, ManagerError};
pub use discovery::{discover, DiscoveryError};
pub use http::{HttpClient, HttpError, PluginCommandView, REPLAY_PAGE_SIZE};
pub use ws::{connect as ws_connect, connect_with as ws_connect_with, WsHandle, WsMessage};
