//! Port-forwarding tunnel abstraction.
//!
//! Defines the protocol-agnostic data types shared between tunnel
//! implementations and their consumers (notably the SSH backend and the
//! UI). Anything that depends on a specific transport (russh handles,
//! tokio accept loops, SOCKS5 handshakes) lives in `crabport-ssh` — this
//! crate only holds the bits that are transport-independent.
//!
//! ## What lives here
//!
//! - [`TunnelEndpoint`] — a `host:port` pair for one end of a tunnel.
//! - [`ReverseForwardRegistry`] / [`LocalTarget`] — the shared map a remote
//!   (`-R`) tunnel's transport callback consults to dispatch inbound
//!   connections to the right local target. Transport-agnostic: it's just a
//!   `HashMap<(String, u32), LocalTarget>` behind an `Arc<Mutex>`.
//! - [`TunnelStatus`] / [`TunnelId`] / [`TunnelInfo`] — the user-facing
//!   snapshot types returned by a tunnel manager for UI rendering.
//! - Re-export of [`TunnelKind`] from `crabport-core`.
//!
//! ## What does NOT live here
//!
//! The `CrabPortTunnel` trait, `TunnelManager`, and `OwnedSession` stay in
//! `crabport-ssh` because they are bound to `russh::client::Handle` (the
//! trait's `handle()` method returns a russh handle, and the manager drives
//! russh channels). Moving them here would require either an associated
//! type on the trait (making the manager generic) or a dependency on russh
//! + on `crabport-ssh`'s connect/auth helpers (creating a cycle). Keeping
//! them in `crabport-ssh` avoids both.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;

use parking_lot::Mutex as PlMutex;

// Re-export `TunnelKind` so callers can reach it via `crabport_tunnel::`
// without depending on `crabport-core` directly.
pub use crabport_core::credential::TunnelKind;

// ---------------------------------------------------------------------------
// Tunnel endpoint
// ---------------------------------------------------------------------------

/// Describes one end of a tunnel — a listen address or a connect target.
#[derive(Debug, Clone)]
pub struct TunnelEndpoint {
    /// IP address or hostname.
    pub host: String,
    /// TCP port.
    pub port: u16,
}

impl TunnelEndpoint {
    pub fn new(host: impl Into<String>, port: u16) -> Self {
        Self {
            host: host.into(),
            port,
        }
    }
}

impl From<SocketAddr> for TunnelEndpoint {
    fn from(addr: SocketAddr) -> Self {
        Self {
            host: addr.ip().to_string(),
            port: addr.port(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tunnel status / id / info
// ---------------------------------------------------------------------------

/// Opaque identifier for a managed tunnel.
pub type TunnelId = u64;

/// Lifecycle status of a single tunnel.
///
/// This is the runtime-facing status (as opposed to the persistence-layer
/// `TunnelKind`, which is just Local/Remote/Dynamic). A tunnel moves
/// through `Starting` → `Active` → `Closed`, or lands in `Failed` if setup
/// errors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TunnelStatus {
    /// The tunnel is being set up (binding the listener / registering the
    /// forward).
    Starting,
    /// The tunnel is live and forwarding traffic.
    Active,
    /// Setup failed. Carries a short human-readable reason.
    Failed(String),
    /// The tunnel has been stopped (either explicitly or because the source
    /// session went away).
    Closed,
}

/// A user-facing snapshot of a tunnel's configuration + state. Returned by
/// a tunnel manager's `list` / `get` methods for UI rendering.
#[derive(Clone, Debug)]
pub struct TunnelInfo {
    pub id: TunnelId,
    pub kind: TunnelKind,
    pub name: String,
    pub status: TunnelStatus,
    /// Address the tunnel listens on. For Local/Dynamic this is the local
    /// bind address; for Remote it's the server-side bind address.
    pub bind_addr: String,
    /// Port the tunnel listens on. For Remote tunnels started with
    /// `bind_port = 0`, this is the server-chosen port returned by
    /// `tcpip_forward`.
    pub bind_port: u16,
    /// Target host for Local/Remote tunnels. Empty for Dynamic (SOCKS)
    /// tunnels, where the target is chosen per-connection by the SOCKS
    /// client.
    pub target_host: String,
    /// Target port for Local/Remote tunnels. `0` for Dynamic tunnels.
    pub target_port: u16,
    /// Total bytes forwarded through this tunnel (sum of both directions).
    /// Best-effort: updated by the accept loops as connections complete.
    pub bytes: u64,
}

// ---------------------------------------------------------------------------
// Reverse-forward registry
// ---------------------------------------------------------------------------

/// Local `host:port` that an inbound Remote-tunnel (`-R`) connection should
/// be bridged to.
#[derive(Clone, Debug)]
pub struct LocalTarget {
    pub host: String,
    pub port: u16,
}

/// Shared, thread-safe map of `(bind_addr, bind_port) -> LocalTarget`.
///
/// Used by remote (`-R`) tunnels: when the SSH server reports a new
/// connection on a remotely-forwarded port, it passes back the
/// `(connected_address, connected_port)` that the client registered via
/// `tcpip_forward`. The transport's `server_channel_open_forwarded_tcpip`
/// callback looks that key up here to find the local `host:port` the
/// inbound connection should be bridged to.
///
/// Cloning is cheap — it just clones the inner `Arc`. All clones share the
/// same underlying map. This type is transport-agnostic (no russh), so it
/// lives here rather than in `crabport-ssh`.
#[derive(Default, Clone)]
pub struct ReverseForwardRegistry {
    inner: Arc<PlMutex<HashMap<(String, u32), LocalTarget>>>,
}

impl ReverseForwardRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a remote-forward mapping.
    ///
    /// `bind_addr`/`bind_port` are the server-side listen address/port
    /// (matching what was passed to `tcpip_forward`), and `target` is the
    /// local destination that inbound connections should be bridged to.
    pub fn insert(&self, bind_addr: String, bind_port: u32, target: LocalTarget) {
        self.inner.lock().insert((bind_addr, bind_port), target);
    }

    /// Remove a remote-forward mapping (e.g. when the tunnel is stopped).
    pub fn remove(&self, bind_addr: &str, bind_port: u32) {
        self.inner
            .lock()
            .remove(&(bind_addr.to_string(), bind_port));
    }

    /// Look up the local target for an inbound connection reported by the
    /// server. `(connected_address, connected_port)` come straight from the
    /// transport's forwarded-tcpip callback.
    pub fn lookup(&self, connected_addr: &str, connected_port: u32) -> Option<LocalTarget> {
        self.inner
            .lock()
            .get(&(connected_addr.to_string(), connected_port))
            .cloned()
    }

    /// Number of registered forwards (mainly useful for diagnostics).
    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }
}
