use std::pin::Pin;
use std::sync::Arc;

use russh::client;

use crate::known_hosts::{KnownHost, KnownHosts, LookupResult};

// ---------------------------------------------------------------------------
// Host-key verification (TOFU)
// ---------------------------------------------------------------------------

/// Information about a server's presented host key, passed to the UI so the
/// user can decide whether to trust an unknown host.
#[derive(Debug, Clone)]
pub struct HostKeyInfo {
    /// Remote hostname / IP as supplied by the caller.
    pub host: String,
    /// SSH port.
    pub port: u16,
    /// Key algorithm name, e.g. `ssh-ed25519`.
    pub algo: String,
    /// SHA-256 base64 (nopad) fingerprint of the key.
    pub fingerprint: String,
}

/// A boxed future returned by [`HostKeyVerifier`].
pub type HostKeyVerifyFuture = Pin<Box<dyn std::future::Future<Output = bool> + Send>>;

/// Callback used to ask the UI whether to trust an unknown host key.
///
/// The future resolves to `true` when the user accepts the key (the caller
/// will then persist it to `known_hosts` and continue the connection), or
/// `false` when the user declines (the connection is aborted).
pub type HostKeyVerifier = Arc<dyn Fn(HostKeyInfo) -> HostKeyVerifyFuture + Send + Sync>;

// ---------------------------------------------------------------------------
// SSH client handler
// ---------------------------------------------------------------------------

pub(crate) struct SshHandler {
    /// Connection target — used for `known_hosts` lookup / persistence.
    pub(crate) host: String,
    pub(crate) port: u16,
    /// Persistent TOFU store. Opened lazily on the connecting task so a
    /// missing store never blocks a connection attempt — the worst case
    /// is that every connect prompts the user.
    pub(crate) known_hosts: Option<KnownHosts>,
    /// UI prompt callback for unknown hosts. `None` means "auto-reject"
    /// (no way to confirm), which is safer than auto-accept.
    pub(crate) verifier: Option<HostKeyVerifier>,
}

#[async_trait::async_trait]
impl client::Handler for SshHandler {
    type Error = anyhow::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::key::PublicKey,
    ) -> Result<bool, Self::Error> {
        let algo = server_public_key.name().to_string();
        let fingerprint = server_public_key.fingerprint();

        // 1. Consult known_hosts (TOFU).
        if let Some(store) = &self.known_hosts {
            match store.lookup(&self.host, self.port, &algo, &fingerprint) {
                Ok(LookupResult::Matched) => {
                    #[cfg(debug_assertions)]
                    tracing::debug!(
                        "SSH: known_hosts match for {}:{} ({} {})",
                        self.host,
                        self.port,
                        algo,
                        fingerprint
                    );
                    return Ok(true);
                }
                Ok(LookupResult::Mismatched {
                    expected_algo,
                    expected_fingerprint,
                }) => {
                    tracing::error!(
                        "SSH: host key mismatch for {}:{} — expected {} {}, got {} {}",
                        self.host,
                        self.port,
                        expected_algo,
                        expected_fingerprint,
                        algo,
                        fingerprint
                    );
                    // Mismatch is a hard failure — do not prompt.
                    return Ok(false);
                }
                Ok(LookupResult::NotFound) => {
                    // Fall through to user prompt below.
                }
                Err(e) => {
                    tracing::warn!(
                        "SSH: known_hosts lookup failed for {}:{} ({e}); falling back to prompt",
                        self.host,
                        self.port
                    );
                }
            }
        }

        // 2. Unknown host — ask the user via the UI verifier.
        let Some(verifier) = self.verifier.clone() else {
            tracing::error!(
                "SSH: unknown host {}:{} and no verifier wired — refusing to connect",
                self.host,
                self.port
            );
            return Ok(false);
        };

        let info = HostKeyInfo {
            host: self.host.clone(),
            port: self.port,
            algo: algo.clone(),
            fingerprint: fingerprint.clone(),
        };
        let accepted = verifier(info).await;

        if accepted {
            // Persist the new entry so future connections don't prompt again.
            if let Some(store) = &self.known_hosts {
                let entry = KnownHost {
                    host: self.host.clone(),
                    port: self.port,
                    algo: algo.clone(),
                    fingerprint: fingerprint.clone(),
                };
                if let Err(e) = store.add(&entry) {
                    tracing::warn!(
                        "SSH: failed to persist known_hosts entry for {}:{} ({e})",
                        self.host,
                        self.port
                    );
                }
            }
            Ok(true)
        } else {
            Ok(false)
        }
    }
}
