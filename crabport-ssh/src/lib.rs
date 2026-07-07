//! SSH backend that implements [`CrabPortTerminal`] using `russh`.
//!
//! Spawns an async task that connects to the remote host, opens a PTY session,
//! and bridges data between the terminal parser and the SSH channel.

mod handler;
mod keys;
mod monitor;
mod terminal;
mod transfer;

pub mod backend;
pub mod known_hosts;
pub mod session;

mod crabport_tunnel;
mod owned_session;

pub use backend::{SshBackend, TOKIO};
pub use crabport_tunnel::{CrabPortTunnel, TunnelManager};
pub use handler::{HostKeyInfo, HostKeyVerifier, HostKeyVerifyFuture, SshHandler};
pub use owned_session::OwnedSession;

// Re-export the transport-agnostic tunnel types from `crabport-tunnel` so
// callers can reach them via `crabport_ssh::` without depending on
// `crabport-tunnel` directly. These used to live in this crate's
// `reverse_registry` module + `crabport_tunnel` module but were hoisted up
// to keep `crabport-tunnel` free of russh bindings.
pub use ::crabport_tunnel::{
    LocalTarget, ReverseForwardRegistry, TunnelEndpoint, TunnelId, TunnelInfo, TunnelKind,
    TunnelStatus,
};
