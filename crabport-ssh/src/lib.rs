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

pub use backend::{SshBackend, TOKIO};
pub use handler::{HostKeyInfo, HostKeyVerifier, HostKeyVerifyFuture};
