use std::{
    io::Cursor,
    pin::Pin,
    sync::{Arc, LazyLock},
};

use async_broadcast::{
    InactiveReceiver, Receiver as BroadcastReceiver, Sender as BroadcastSender, broadcast,
};
use async_channel::{Sender as MpscSender, unbounded};
use crabport_sftp::CrabPortSftp;
use crabport_terminal::terminal::{
    BackendEvent, CrabPortMonitor, CrabPortTerminal, MemoryStats, NetworkStats, RemoteMetrics,
    RemoteStatus, SftpTransferBytes, SftpTransferKind, SftpTransferStage,
};
use parking_lot::RwLock;
use russh::{
    Channel, ChannelMsg,
    client::{self, Msg},
    keys::key::KeyPair,
};
use tokio::{runtime::Runtime, select, sync::Mutex as TokioMutex};

use crate::known_hosts::{KnownHost, KnownHosts, LookupResult};
use crate::session::SshConnectionInfo;

// ---------------------------------------------------------------------------
// Tokio runtime for russh (russh internally requires tokio)
// ---------------------------------------------------------------------------

static TOKIO: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("failed to create tokio runtime for SSH"));

// ---------------------------------------------------------------------------
// Internal command queue
// ---------------------------------------------------------------------------

enum Command {
    Write(Vec<u8>),
    Resize(u16, u16),
    Close,
}

// ---------------------------------------------------------------------------
// Shared monitor state
// ---------------------------------------------------------------------------

/// State shared between the SSH event loop and the monitor task.
struct MonitorState {
    status: RemoteStatus,
    metrics: RemoteMetrics,
}

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

struct SshHandler {
    /// Connection target — used for `known_hosts` lookup / persistence.
    host: String,
    port: u16,
    /// Persistent TOFU store. Opened lazily on the connecting task so a
    /// missing store never blocks a connection attempt — the worst case
    /// is that every connect prompts the user.
    known_hosts: Option<KnownHosts>,
    /// UI prompt callback for unknown hosts. `None` means "auto-reject"
    /// (no way to confirm), which is safer than auto-accept.
    verifier: Option<HostKeyVerifier>,
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

// ---------------------------------------------------------------------------
// SshBackend
// ---------------------------------------------------------------------------

/// SSH terminal backend.
///
/// Connects via TCP, authenticates, opens a PTY session, then enters a
/// single `tokio::select!` event loop that handles reads, writes, and
/// resizes — no mutex needed because only one task touches the channel.
pub struct SshBackend {
    command_tx: MpscSender<Command>,
    event_tx: BroadcastSender<BackendEvent>,
    _event_rx: InactiveReceiver<BackendEvent>,
    monitor: Arc<RwLock<MonitorState>>,
    _on_status: Arc<dyn Fn(String) + Send + Sync>,
    handle: Arc<TokioMutex<Option<Arc<TokioMutex<client::Handle<SshHandler>>>>>>,
    sftp_entries: Arc<RwLock<Option<Arc<Vec<(String, bool)>>>>>,
    sftp_cwd: Arc<RwLock<Option<Arc<String>>>>,
    /// Cached SFTP subsystem session. Reused across navigations so we don't
    /// pay the cost of opening a fresh SFTP channel (and leaking the old
    /// one) on every `sftp_navigate` call. Lazily (re)connected if `None`,
    /// e.g. after the server closes the channel or on first use.
    sftp_session: Arc<TokioMutex<Option<crabport_sftp::SftpBackend>>>,
}

impl SshBackend {
    pub fn new(
        info: SshConnectionInfo,
        cols: u16,
        rows: u16,
        on_status: Arc<dyn Fn(String) + Send + Sync>,
        verifier: Option<HostKeyVerifier>,
    ) -> Self {
        let (event_tx, event_rx) = broadcast(1024);
        let _event_rx = event_rx.deactivate();
        let (command_tx, command_rx) = unbounded::<Command>();

        let monitor = Arc::new(RwLock::new(MonitorState {
            status: RemoteStatus::Connecting,
            metrics: RemoteMetrics::default(),
        }));
        let handle: Arc<TokioMutex<Option<Arc<TokioMutex<client::Handle<SshHandler>>>>>> =
            Arc::new(TokioMutex::new(None));

        let sftp_entries: Arc<RwLock<Option<Arc<Vec<(String, bool)>>>>> =
            Arc::new(RwLock::new(None));
        let sftp_entries2 = sftp_entries.clone();
        let sftp_cwd: Arc<RwLock<Option<Arc<String>>>> = Arc::new(RwLock::new(None));
        let sftp_cwd2 = sftp_cwd.clone();
        let sftp_session: Arc<TokioMutex<Option<crabport_sftp::SftpBackend>>> =
            Arc::new(TokioMutex::new(None));
        let sftp_session2 = sftp_session.clone();

        let event_tx2 = event_tx.clone();
        let monitor2 = monitor.clone();
        let info_for_monitor = info.clone();
        let on_status2 = on_status.clone();
        let handle_for_spawn = handle.clone();
        let host_for_handler = info.host.clone();
        let port_for_handler = info.port;
        let verifier_for_handler = verifier.clone();

        TOKIO.spawn(async move {
            // ---- Connect ----
            let addr = format!("{}:{}", info.host, info.port);
            #[cfg(debug_assertions)]
            tracing::info!("SSH: connecting to {}", addr);
            on_status2(format!("Connecting to {}", addr));

            // Open the known_hosts store. Failure here is non-fatal — we
            // just fall back to prompting on every connect.
            let known_hosts = match KnownHosts::open() {
                Ok(s) => Some(s),
                Err(e) => {
                    tracing::warn!("SSH: could not open known_hosts store ({e})");
                    None
                }
            };

            let handler = SshHandler {
                host: host_for_handler,
                port: port_for_handler,
                known_hosts,
                verifier: verifier_for_handler,
            };

            let config = Arc::new(client::Config::default());
            let mut sh = match client::connect(config, &addr, handler).await {
                Ok(sh) => {
                    on_status2("TCP connection established".into());
                    sh
                }
                Err(e) => {
                    tracing::error!("SSH: connect failed: {e}");
                    {
                        let mut m = monitor2.write();
                        m.status = RemoteStatus::Disconnected;
                    }
                    let _ = event_tx2
                        .broadcast(BackendEvent::Error(e.to_string()))
                        .await;
                    return;
                }
            };

            #[cfg(debug_assertions)]
            tracing::info!(
                "SSH: auth decision — uses_key_auth={}, private_key={}, has_passphrase={}, username={}",
                info.uses_key_auth(),
                info.private_key.is_some(),
                info.passphrase.is_some(),
                info.username,
            );
            if info.uses_key_auth() {
                on_status2("Authenticating with public key...".into());

                let key_str = info.private_key.as_deref().unwrap_or("");
                #[cfg(debug_assertions)]
                tracing::info!("SSH: private key length={}, starts_with_BEGIN={}", key_str.len(), key_str.contains("BEGIN"));
                let key_pair = match decode_private_key(key_str, info.passphrase.as_deref()) {
                    Ok(kp) => kp,
                    Err(e) => {
                        tracing::error!("SSH: failed to decode private key: {e}");
                        {
                            let mut m = monitor2.write();
                            m.status = RemoteStatus::Disconnected;
                        }
                        let _ = event_tx2
                            .broadcast(BackendEvent::Error(format!(
                                "Public key decode failed: {e}"
                            )))
                            .await;
                        return;
                    }
                };

                let auth_result = sh
                    .authenticate_publickey(&info.username, Arc::new(key_pair))
                    .await;

                #[cfg(debug_assertions)]
                tracing::info!("SSH: publickey auth result = {:?}", auth_result);
                match auth_result {
                    Ok(true) => {
                        on_status2("Public key authentication succeeded".into());
                    }
                    Ok(false) => {
                        tracing::error!("SSH: key auth rejected");
                        {
                            let mut m = monitor2.write();
                            m.status = RemoteStatus::Disconnected;
                        }
                        let _ = event_tx2
                            .broadcast(BackendEvent::Error(
                                "Public key authentication failed".into(),
                            ))
                            .await;
                        return;
                    }
                    Err(e) => {
                        tracing::error!("SSH: key auth failed: {e}");
                        {
                            let mut m = monitor2.write();
                            m.status = RemoteStatus::Disconnected;
                        }
                        let _ = event_tx2
                            .broadcast(BackendEvent::Error(format!(
                                "Public key authentication failed: {e}"
                            )))
                            .await;
                        return;
                    }
                }
            } else {
                #[cfg(debug_assertions)]
                tracing::info!("SSH: using password auth (private_key is None)");
                on_status2("Authenticating with password...".into());
                match sh
                    .authenticate_password(&info.username, &info.password)
                    .await
                {
                    Ok(true) => {
                        #[cfg(debug_assertions)]
                        tracing::info!("SSH: password auth succeeded");
                        on_status2("Password authentication succeeded".into());
                    }
                    Ok(false) => {
                        tracing::error!("SSH: password auth rejected");
                        {
                            let mut m = monitor2.write();
                            m.status = RemoteStatus::Disconnected;
                        }
                        let _ = event_tx2
                            .broadcast(BackendEvent::Error("Password authentication failed".into()))
                            .await;
                        return;
                    }
                    Err(e) => {
                        tracing::error!("SSH: auth failed: {e}");
                        {
                            let mut m = monitor2.write();
                            m.status = RemoteStatus::Disconnected;
                        }
                        let _ = event_tx2
                            .broadcast(BackendEvent::Error(format!(
                                "Password authentication failed: {e}"
                            )))
                            .await;
                        return;
                    }
                }
            }

            // ---- Open session channel ----
            on_status2("Opening session channel...".into());
            let mut channel: Channel<Msg> = match sh.channel_open_session().await {
                Ok(ch) => {
                    on_status2("Session channel opened".into());
                    ch
                }
                Err(e) => {
                    tracing::error!("SSH: open session failed: {e}");
                    {
                        let mut m = monitor2.write();
                        m.status = RemoteStatus::Disconnected;
                    }
                    let _ = event_tx2
                        .broadcast(BackendEvent::Error(format!(
                            "Session channel open failed: {e}"
                        )))
                        .await;
                    return;
                }
            };

            // ---- Request PTY ----
            on_status2(format!("Requesting PTY ({}x{})...", cols, rows));
            let term = std::env::var("TERM").unwrap_or_else(|_| "xterm-256color".into());
            if let Err(e) = channel
                .request_pty(false, &term, cols as u32, rows as u32, 0, 0, &[])
                .await
            {
                tracing::error!("SSH: PTY request failed: {e}");
                {
                    let mut m = monitor2.write();
                    m.status = RemoteStatus::Disconnected;
                }
                let _ = event_tx2
                    .broadcast(BackendEvent::Error(format!("PTY request failed: {e}")))
                    .await;
                return;
            }
            on_status2("PTY allocated".into());

            // ---- Start shell ----
            on_status2("Starting shell...".into());
            if let Err(e) = channel.request_shell(true).await {
                tracing::error!("SSH: shell request failed: {e}");
                {
                    let mut m = monitor2.write();
                    m.status = RemoteStatus::Disconnected;
                }
                let _ = event_tx2
                    .broadcast(BackendEvent::Error(format!("Shell request failed: {e}")))
                    .await;
                return;
            }
            on_status2("Shell started".into());

            // Mark as connected
            {
                let mut m = monitor2.write();
                m.status = RemoteStatus::Connected;
            }

            // ---- Try SFTP ----
            // Open the SFTP subsystem once and cache it for the lifetime of
            // the connection. Subsequent `sftp_navigate` calls reuse this
            // session instead of opening (and leaking) a new channel each
            // time.
            match crabport_sftp::SftpBackend::connect(&sh).await {
                Ok(sftp) => {
                    tracing::info!("SSH: SFTP subsystem available");
                    // Resolve cwd
                    match sftp.canonicalize(".").await {
                        Ok(cwd) => {
                            *sftp_cwd2.write() = Some(Arc::new(cwd));
                        }
                        Err(e) => tracing::warn!("SSH: SFTP canonicalize failed ({e})"),
                    }
                    match sftp.read_dir(".").await {
                        Ok(entries) => {
                            *sftp_entries2.write() = Some(Arc::new(entries));
                        }
                        Err(e) => tracing::warn!("SSH: SFTP read_dir failed ({e})"),
                    }
                    *sftp_session2.lock().await = Some(sftp);
                }
                Err(e) => tracing::warn!("SSH: SFTP subsystem not available ({e})"),
            }

            // ---- Spawn monitor task ----
            // Wrap the Handle in Arc<TokioMutex> so the monitor task can use it.
            // Handle is not Clone, so we share it via Arc.
            let handle_for_monitor = Arc::new(TokioMutex::new(sh));

            // Share the same Arc with SshBackend for SFTP access.
            *handle_for_spawn.lock().await = Some(handle_for_monitor.clone());

            let monitor_for_task = monitor2.clone();
            let info_for_task = info_for_monitor;
            TOKIO.spawn(async move {
                monitor_loop(handle_for_monitor, info_for_task, monitor_for_task).await;
            });

            // ---- Event loop (read + cmd via tokio::select!) ----
            loop {
                select! {
                    msg = channel.wait() => {
                        match msg {
                            Some(ChannelMsg::Data { data }) => {
                                let _ = event_tx2
                                    .broadcast(BackendEvent::Data(data.to_vec()))
                                    .await;
                            }
                            Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => {
                                #[cfg(debug_assertions)]
                                tracing::info!("SSH: channel closed by remote");
                                {
                                    let mut m = monitor2.write();
                                    m.status = RemoteStatus::Disconnected;
                                }
                                let _ = event_tx2.broadcast(BackendEvent::Closed).await;
                                return;
                            }
                            _ => {}
                        }
                    }
                    cmd = command_rx.recv() => {
                        match cmd {
                            Ok(Command::Write(data)) => {
                                if let Err(_e) = channel.data(Cursor::new(data)).await {
                                    #[cfg(debug_assertions)]
                                    tracing::warn!("SSH: write error: {_e}");
                                }
                            }
                            Ok(Command::Resize(cols, rows)) => {
                                if let Err(_e) = channel
                                    .window_change(cols as u32, rows as u32, 0, 0)
                                    .await
                                {
                                    #[cfg(debug_assertions)]
                                    tracing::warn!("SSH: window change error: {_e}");
                                }
                            }
                            Ok(Command::Close) | Err(_) => {
                                let _ = channel.eof().await;
                                // Close the cached SFTP session (if any) so the
                                // server-side channel is torn down cleanly
                                // rather than lingering until the SSH
                                // connection itself drops.
                                if let Some(sftp) = sftp_session2.lock().await.take() {
                                    let _ = sftp.close().await;
                                }
                                {
                                    let mut m = monitor2.write();
                                    m.status = RemoteStatus::Disconnected;
                                }
                                let _ = event_tx2.broadcast(BackendEvent::Closed).await;
                                return;
                            }
                        }
                    }
                }
            }
        });

        Self {
            command_tx,
            event_tx,
            _event_rx,
            monitor,
            _on_status: on_status,
            handle,
            sftp_entries,
            sftp_cwd,
            sftp_session,
        }
    }

    // -----------------------------------------------------------------------
    // File transfer (implicit gzip + remote tmp staging)
    // -----------------------------------------------------------------------
    //
    // Both directions follow the same pattern to keep the wire format
    // compressed without changing what's stored at the final destinations:
    //
    //   upload   local_orig --gzip--> remote_tmp.gz --gunzip--> remote_orig
    //   download remote_orig --gzip--> remote_tmp.gz --gunzip--> local_orig
    //
    // The client-side gzip happens in-memory over the SFTP file stream (see
    // `SftpBackend::upload_file_gz` / `download_file_gz`), so no local tmp
    // file is needed. The remote side needs a tmp file because we drive the
    // (de)compression via `ssh exec`, which can't stream into a pre-opened
    // SFTP handle — it has to read from a path.
    //
    // Remote tmp paths use a per-call random token to avoid collisions
    // between concurrent transfers. They live under `/tmp` (overridable via
    // `$TMPDIR` on the remote, which `mktemp` honours).

    /// Download a remote file into `local_path`.
    ///
    /// See [`sftp_download_impl`] for the gzip/tmp staging flow. This is a
    /// thin wrapper that supplies the shared fields as a [`SftpTransferHandle`].
    pub async fn sftp_download(&self, remote_path: &str, local_path: &str) -> anyhow::Result<()> {
        let backend = SftpTransferHandle {
            handle: self.handle.clone(),
            sftp_session: self.sftp_session.clone(),
            event_tx: Some(self.event_tx.clone()),
        };
        sftp_download_impl(&backend, remote_path, local_path).await
    }

    /// Upload a local file into `remote_path`.
    ///
    /// See [`sftp_upload_impl`] for the gzip/tmp staging flow. This is a
    /// thin wrapper that supplies the shared fields as a [`SftpTransferHandle`].
    pub async fn sftp_upload(&self, local_path: &str, remote_path: &str) -> anyhow::Result<()> {
        let backend = SftpTransferHandle {
            handle: self.handle.clone(),
            sftp_session: self.sftp_session.clone(),
            event_tx: Some(self.event_tx.clone()),
        };
        sftp_upload_impl(&backend, local_path, remote_path).await
    }
}

// ---------------------------------------------------------------------------
// Monitor loop — periodically collects latency / memory / network via SSH exec
// ---------------------------------------------------------------------------

async fn monitor_loop(
    handle: Arc<TokioMutex<client::Handle<SshHandler>>>,
    _info: SshConnectionInfo,
    monitor: Arc<RwLock<MonitorState>>,
) {
    let mut prev_net_sent: u64 = 0;
    let mut prev_net_recv: u64 = 0;

    // ---- First collection immediately on connection ----
    {
        let h = handle.lock().await;
        let latency_ms = measure_latency(&h).await;
        let memory = collect_memory(&h).await;
        let network = collect_network(&h).await;

        if let Some(net) = network {
            prev_net_sent = net.bytes_sent;
            prev_net_recv = net.bytes_recv;
        }

        let mut m = monitor.write();
        m.metrics = RemoteMetrics {
            latency_ms,
            memory,
            network: None, // No rate on first tick
        };
    }

    loop {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;

        // Skip if disconnected
        {
            let m = monitor.read();
            if m.status == RemoteStatus::Disconnected {
                return;
            }
        }

        let h = handle.lock().await;

        // ---- Latency: measure RTT of a small exec command ----
        let latency_ms = measure_latency(&h).await;

        // ---- Memory: parse /proc/meminfo ----
        let memory = collect_memory(&h).await;

        // ---- Network: parse /proc/net/dev ----
        let raw_network = collect_network(&h).await;
        let network = raw_network.map(|net| {
            let rate_sent = net.bytes_sent.saturating_sub(prev_net_sent);
            let rate_recv = net.bytes_recv.saturating_sub(prev_net_recv);
            prev_net_sent = net.bytes_sent;
            prev_net_recv = net.bytes_recv;
            NetworkStats {
                bytes_sent: rate_sent,
                bytes_recv: rate_recv,
            }
        });

        // ---- Update shared state ----
        {
            let mut m = monitor.write();
            m.metrics = RemoteMetrics {
                latency_ms,
                memory,
                network,
            };
        }
    }
}

/// Measure round-trip latency by executing `echo ping` over SSH.
async fn measure_latency(handle: &client::Handle<SshHandler>) -> Option<u32> {
    let start = std::time::Instant::now();
    match handle.channel_open_session().await {
        Ok(mut ch) => {
            if ch.exec(true, "echo ping").await.is_err() {
                return None;
            }
            // Drain output until channel closes
            loop {
                match ch.wait().await {
                    Some(ChannelMsg::Data { .. }) => {}
                    Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
                    _ => {}
                }
            }
            let elapsed = start.elapsed().as_millis() as u32;
            Some(elapsed)
        }
        Err(_) => None,
    }
}

/// Collect remote memory stats via `cat /proc/meminfo`.
async fn collect_memory(handle: &client::Handle<SshHandler>) -> Option<MemoryStats> {
    let output = exec_and_read(handle, "cat /proc/meminfo").await?;
    let mut total: u64 = 0;
    let mut available: u64 = 0;

    for line in output.lines() {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let value = parts[1].parse::<u64>().unwrap_or(0);
        // /proc/meminfo values are in kB
        if parts[0].starts_with("MemTotal") {
            total = value * 1024;
        } else if parts[0].starts_with("MemAvailable") {
            available = value * 1024;
        }
    }

    if total == 0 {
        return None;
    }

    Some(MemoryStats {
        total,
        used: total.saturating_sub(available),
    })
}

/// Collect remote network stats via `cat /proc/net/dev`.
/// Sums across all interfaces.
async fn collect_network(handle: &client::Handle<SshHandler>) -> Option<NetworkStats> {
    let output = exec_and_read(handle, "cat /proc/net/dev").await?;
    let mut bytes_recv: u64 = 0;
    let mut bytes_sent: u64 = 0;

    for line in output.lines() {
        let trimmed = line.trim();
        // Skip header lines
        if !trimmed.contains(':') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split(':').collect();
        if parts.len() < 2 {
            continue;
        }
        let fields: Vec<&str> = parts[1].split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }
        // Fields: receive bytes (0) | ... | transmit bytes (8) | ...
        bytes_recv += fields[0].parse::<u64>().unwrap_or(0);
        bytes_sent += fields[8].parse::<u64>().unwrap_or(0);
    }

    Some(NetworkStats {
        bytes_sent,
        bytes_recv,
    })
}

/// Execute a command over SSH and read all its stdout output.
async fn exec_and_read(handle: &client::Handle<SshHandler>, cmd: &str) -> Option<String> {
    let mut ch = handle.channel_open_session().await.ok()?;
    if ch.exec(true, cmd).await.is_err() {
        return None;
    }

    let mut output = Vec::new();
    loop {
        match ch.wait().await {
            Some(ChannelMsg::Data { data }) => {
                output.extend_from_slice(&data);
            }
            Some(ChannelMsg::Eof) | Some(ChannelMsg::Close) | None => break,
            _ => {}
        }
    }

    String::from_utf8(output).ok()
}

/// Execute a command over SSH, returning `(exit_code, combined_stdout_stderr)`.
///
/// Used by the gzip-staged transfer methods to run `gzip`/`gunzip` on the
/// remote and verify they succeeded. stdout and stderr are merged so error
/// messages reach the caller regardless of which stream the server wrote
/// them to.
///
/// If the channel dies before delivering an `ExitStatus` (e.g. the server
/// killed the process), returns `(127, <captured output>)` — 127 is the
/// conventional "command not found / abnormal exit" code.
async fn exec_with_status(handle: &client::Handle<SshHandler>, cmd: &str) -> (u32, String) {
    let mut ch = match handle.channel_open_session().await {
        Ok(ch) => ch,
        Err(e) => return (127, format!("failed to open channel: {e}")),
    };
    if ch.exec(true, cmd).await.is_err() {
        return (127, "failed to start exec".to_string());
    }

    let mut output = Vec::new();
    // Default to 127 so a missing `ExitStatus` (e.g. the channel was closed
    // by the remote before reporting one) is treated as a failure rather
    // than silently succeeding. The actual exit status, when delivered,
    // overrides this below.
    let mut exit_code = 127;
    // Track whether we've seen an explicit `ExitStatus`. russh *can*
    // deliver `Eof` before `ExitStatus` on some servers, so we must not
    // break out of the loop on `Eof` alone — we keep draining until we
    // either see `ExitStatus` or the channel is fully closed (`Close`/
    // `None`). Without this, we'd return the 127 default for commands
    // that actually succeeded.
    let mut saw_exit_status = false;
    loop {
        match ch.wait().await {
            // russh delivers stdout and stderr on separate message variants
            // — capture both into the same buffer.
            Some(ChannelMsg::Data { data }) => output.extend_from_slice(&data),
            Some(ChannelMsg::ExtendedData { data, .. }) => output.extend_from_slice(&data),
            Some(ChannelMsg::ExitStatus { exit_status }) => {
                exit_code = exit_status;
                saw_exit_status = true;
            }
            // Only break once we've either seen the exit status or the
            // channel is fully closed. `Eof` alone is not enough — the
            // `ExitStatus` may still arrive afterwards.
            Some(ChannelMsg::Close) | None => break,
            Some(ChannelMsg::Eof) if saw_exit_status => break,
            _ => {}
        }
    }

    (exit_code, String::from_utf8_lossy(&output).into_owned())
}

/// Build a unique remote tmp path for a single transfer.
///
/// Uses a `crabport-` prefix and a 16-hex-digit token derived from the
/// current time + a per-process counter. The token only needs to be unique
/// among concurrent transfers from this process; the prefix keeps us from
/// colliding with other tools that might write to `/tmp`.
fn remote_tmp_path() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    // Mix in the PID + a coarse timestamp so two Crabport processes running
    // at the same time don't collide.
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0);
    let token = nanos ^ ((pid as u64) << 32) ^ (n << 16);
    format!("/tmp/crabport-{token:016x}.gz")
}

/// Quote a path for inclusion in a shell command on the remote.
///
/// Wraps the path in single quotes and escapes any embedded single quotes
/// via the standard `'\''` idiom. This is the only fully-safe way to embed
/// an arbitrary path in a POSIX shell command without relying on the remote
/// shell being bash.
fn shell_quote(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// Lightweight borrowed view of the shared SFTP-related fields on
/// [`SshBackend`].
///
/// This exists so the transfer orchestration can live in free functions
/// that take only what they need (the SSH handle + the SFTP session cache),
/// without borrowing `&SshBackend` — which would prevent the orchestration
/// from being awaited inside a `TOKIO.spawn` future (those require
/// `'static`).
struct SftpTransferHandle {
    handle: Arc<TokioMutex<Option<Arc<TokioMutex<client::Handle<SshHandler>>>>>>,
    sftp_session: Arc<TokioMutex<Option<crabport_sftp::SftpBackend>>>,
    /// Optional broadcast sink for live progress events. `None` when the
    /// handle is constructed by code paths that don't have an `event_tx`
    /// (e.g. tests); in that case progress is simply not reported.
    event_tx: Option<BroadcastSender<BackendEvent>>,
}

impl SftpTransferHandle {
    /// Lazily open a fresh SFTP session when the cache is empty. Used by the
    /// transfer methods as a fallback — they prefer to reuse the cached
    /// session, but if a prior error dropped it we reconnect rather than
    /// failing the transfer outright.
    async fn open_sftp_session(&self) -> anyhow::Result<crabport_sftp::SftpBackend> {
        let guard = self.handle.lock().await;
        let shared = guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("SSH handle not available"))?;
        let h = shared.lock().await;
        crabport_sftp::SftpBackend::connect(&*h).await
    }

    /// Take the cached SFTP session if present, else open a fresh one.
    async fn take_or_open_sftp(&self) -> anyhow::Result<crabport_sftp::SftpBackend> {
        if let Some(s) = self.sftp_session.lock().await.take() {
            return Ok(s);
        }
        self.open_sftp_session().await
    }

    /// Return a live session to the cache. On error, close it instead so the
    /// cache doesn't hold a dead handle. Returns the original result for
    /// ergonomic chaining.
    async fn return_sftp(
        &self,
        s: crabport_sftp::SftpBackend,
        result: anyhow::Result<()>,
    ) -> anyhow::Result<()> {
        match &result {
            Ok(()) => *self.sftp_session.lock().await = Some(s),
            Err(_) => {
                let _ = s.close().await;
            }
        }
        result
    }

    /// Best-effort broadcast of a transfer-progress event. Failures (e.g. no
    /// subscribers) are silently ignored — progress is informational, not
    /// load-bearing, and we must never let a UI-side drop affect the
    /// transfer itself.
    async fn emit_progress(
        &self,
        kind: SftpTransferKind,
        stage: SftpTransferStage,
        message: impl Into<String>,
    ) {
        self.emit_progress_bytes(kind, stage, message, None).await;
    }

    /// Like [`emit_progress`](Self::emit_progress) but carries byte-level
    /// progress for stages that support it (currently only the SFTP
    /// streaming `Transfer` stage).
    async fn emit_progress_bytes(
        &self,
        kind: SftpTransferKind,
        stage: SftpTransferStage,
        message: impl Into<String>,
        bytes: Option<SftpTransferBytes>,
    ) {
        let Some(tx) = self.event_tx.as_ref() else {
            return;
        };
        let _ = tx
            .broadcast(BackendEvent::SftpTransferProgress {
                kind,
                stage,
                message: message.into(),
                bytes,
            })
            .await;
    }

    /// Build a byte-progress callback suitable for handing to the SFTP
    /// streaming layer. The callback emits a `SftpTransferProgress` event
    /// with the given `(kind, stage, message)` and the current `(done, total)`
    /// byte counts. Throttled to one event per ~100ms so a fast transfer
    /// doesn't flood the broadcast channel.
    ///
    /// The returned closure is `Send + Sync` and cheap to clone (it holds an
    /// `Arc`), so it can be passed into `crabport-sftp`'s streaming functions.
    fn make_byte_progress_cb(
        &self,
        kind: SftpTransferKind,
        stage: SftpTransferStage,
        message: String,
        total: u64,
    ) -> Arc<dyn Fn(u64) + Send + Sync> {
        let tx = self.event_tx.clone();
        let last = Arc::new(std::sync::atomic::AtomicU64::new(0));
        let message = std::sync::Arc::new(message);
        Arc::new(move |done: u64| {
            // Throttle: only emit if at least 100ms of wall-clock has passed
            // since the last emit. We approximate "time" with a monotonic
            // nanos counter stored in the atomic — this avoids pulling in a
            // `Mutex<Instant>` just for throttling.
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let prev = last.load(std::sync::atomic::Ordering::Relaxed);
            // 100ms throttle window.
            if now.saturating_sub(prev) < 100 && done != total {
                return;
            }
            last.store(now, std::sync::atomic::Ordering::Relaxed);
            let Some(tx) = tx.as_ref() else {
                return;
            };
            let bytes = SftpTransferBytes { done, total };
            let message = (*message).clone();
            // try_broadcast so we never block the streaming loop if the
            // channel is full — progress is best-effort.
            let _ = tx.try_broadcast(BackendEvent::SftpTransferProgress {
                kind,
                stage,
                message,
                bytes: Some(bytes),
            });
        })
    }
}

/// Download `remote_path` into `local_path`.
///
/// Dispatches based on what `remote_path` is:
///   - regular file → single-file gzip staging ([`sftp_download_file_impl`])
///   - directory → tar.gz staging with recursive SFTP fallback
///     ([`sftp_download_dir_impl`])
async fn sftp_download_impl(
    backend: &SftpTransferHandle,
    remote_path: &str,
    local_path: &str,
) -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    tracing::info!("SFTP download impl: remote={remote_path} local={local_path}");
    let (is_dir, original_size) = {
        let s = backend.take_or_open_sftp().await?;
        let meta_res = s.metadata(remote_path).await;
        let (is_dir, size) = match meta_res {
            Ok(m) => {
                let is_dir = m.file_type().is_dir();
                let size = m.size.unwrap_or(0);
                (is_dir, size)
            }
            Err(e) => {
                let msg = format!("remote stat failed: {e}");
                backend
                    .return_sftp(s, Err(anyhow::anyhow!(msg.clone())))
                    .await
                    .ok();
                return Err(anyhow::anyhow!(msg));
            }
        };
        backend.return_sftp(s, Ok(())).await?;
        (is_dir, size)
    };

    if is_dir {
        sftp_download_dir_impl(backend, remote_path, local_path).await
    } else {
        sftp_download_file_impl(backend, remote_path, local_path, original_size).await
    }
}

/// Upload `local_path` to `remote_path`.
///
/// Dispatches based on what `local_path` is:
///   - regular file → single-file gzip staging ([`sftp_upload_file_impl`])
///   - directory → tar.gz staging with recursive SFTP fallback
///     ([`sftp_upload_dir_impl`])
async fn sftp_upload_impl(
    backend: &SftpTransferHandle,
    local_path: &str,
    remote_path: &str,
) -> anyhow::Result<()> {
    let meta = std::fs::metadata(local_path)?;
    if meta.is_dir() {
        sftp_upload_dir_impl(backend, local_path, remote_path).await
    } else {
        sftp_upload_file_impl(backend, local_path, remote_path).await
    }
}

/// Delete a remote file or directory. Stats the path first to choose
/// `remove_file` vs `remove_dir` — SFTP's `remove_dir` only works on empty
/// directories, so for non-empty dirs we fall back to a recursive walk that
/// deletes contents depth-first.
async fn sftp_delete_impl(backend: &SftpTransferHandle, remote_path: &str) -> anyhow::Result<()> {
    #[cfg(debug_assertions)]
    tracing::info!("SFTP delete: remote={remote_path}");
    let s = backend.take_or_open_sftp().await?;
    let meta_res = s.metadata(remote_path).await;
    let is_dir = match &meta_res {
        Ok(m) => m.file_type().is_dir(),
        Err(e) => {
            let msg = format!("remote stat failed: {e}");
            backend
                .return_sftp(s, Err(anyhow::anyhow!(msg.clone())))
                .await
                .ok();
            return Err(anyhow::anyhow!(msg));
        }
    };
    backend.return_sftp(s, meta_res.map(|_| ())).await?;

    if !is_dir {
        let s = backend.take_or_open_sftp().await?;
        let res = s.remove_file(remote_path).await;
        backend.return_sftp(s, res).await?;
        return Ok(());
    }

    let s = backend.take_or_open_sftp().await?;
    let direct = s.remove_dir(remote_path).await;
    let direct_ok = direct.is_ok();
    backend.return_sftp(s, direct).await.ok();
    if direct_ok {
        return Ok(());
    }

    sftp_delete_dir_recursive(backend, remote_path).await
}

/// Recursively delete a non-empty remote directory: list entries, delete
/// each child (files directly, subdirs recursively), then remove the now-
/// empty directory itself. Depth-first so the final `remove_dir` succeeds.
async fn sftp_delete_dir_recursive(
    backend: &SftpTransferHandle,
    remote_path: &str,
) -> anyhow::Result<()> {
    let s = backend.take_or_open_sftp().await?;
    let entries_res = s.read_dir(remote_path).await;
    let entries = match entries_res {
        Ok(e) => {
            backend.return_sftp(s, Ok(())).await?;
            e
        }
        Err(e) => {
            let msg = format!("read_dir failed: {e}");
            backend.return_sftp(s, Err(e)).await.ok();
            return Err(anyhow::anyhow!(msg));
        }
    };

    for (name, is_dir) in entries {
        if name == "." || name == ".." {
            continue;
        }
        let child = join_remote_path(remote_path, &name);
        if is_dir {
            Box::pin(sftp_delete_dir_recursive(backend, &child)).await?;
        } else {
            let s = backend.take_or_open_sftp().await?;
            let res = s.remove_file(&child).await;
            backend.return_sftp(s, res).await?;
        }
    }

    // Now the directory should be empty — remove it.
    let s = backend.take_or_open_sftp().await?;
    let res = s.remove_dir(remote_path).await;
    backend.return_sftp(s, res).await?;
    Ok(())
}

/// Download a single remote file into `local_path` using gzip staging.
///
/// Steps:
///   1. `ssh exec`: `gzip -c -- <remote> > /tmp/crabport-XXXX.gz`
///   2. SFTP-stream the .gz down with in-flight gunzip into `local_path`.
///   3. `ssh exec`: `rm -f -- /tmp/crabport-XXXX.gz`
async fn sftp_download_file_impl(
    backend: &SftpTransferHandle,
    remote_path: &str,
    local_path: &str,
    _original_size: u64,
) -> anyhow::Result<()> {
    // gzip-staging download flow:
    //   1. Remote `gzip -c` the file into a tmp `.gz` (off-loads compression
    //      to the server, doesn't touch the original).
    //   2. `download_file_gz` downloads the `.gz` via parallel segmented
    //      SFTP reads (full throughput) and decompresses locally — no
    //      network round-trip per decompressed chunk.
    //   3. Clean up the remote tmp.
    //
    // The previous streaming version (`GzipDecoder` over the live SFTP file)
    // was slow because each `decoder.read()` blocked on a network round-trip
    // and gzip can't be split into parallel segments. Splitting download and
    // decompress into two phases lets the download phase hit full SFTP
    // throughput (4× parallel `SSH_FXP_READ`).
    let tmp = remote_tmp_path();

    // Acquire the SSH handle for the exec step.
    let handle_guard = backend.handle.lock().await;
    let shared = handle_guard
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("SSH handle not available"))?
        .clone();
    drop(handle_guard);
    let h = shared.lock().await;

    // 1. Compress remotely into the tmp file.
    backend
        .emit_progress(
            SftpTransferKind::Download,
            SftpTransferStage::Compress,
            remote_path,
        )
        .await;
    let cmd = format!(
        "gzip -c -- {remote_q} > {tmp_q} && printf ok",
        remote_q = shell_quote(remote_path),
        tmp_q = shell_quote(&tmp),
    );
    #[cfg(debug_assertions)]
    tracing::info!("SFTP download file: compress cmd={cmd}");
    let (code, out) = exec_with_status(&h, &cmd).await;
    if code != 0 || !out.ends_with("ok") {
        return Err(anyhow::anyhow!("remote gzip failed (exit {code}): {out}"));
    }
    drop(h);

    // 2. Stat the remote `.gz` for the progress bar total.
    let total = {
        let s = backend.take_or_open_sftp().await?;
        let meta_res = s.metadata(&tmp).await;
        let total = match &meta_res {
            Ok(m) => m.size.unwrap_or(0),
            Err(_) => 0,
        };
        backend.return_sftp(s, meta_res.map(|_| ())).await?;
        total
    };
    let progress_cb = backend.make_byte_progress_cb(
        SftpTransferKind::Download,
        SftpTransferStage::Transfer,
        local_path.to_string(),
        total,
    );
    progress_cb(0);
    let s = backend.take_or_open_sftp().await?;
    let res = s
        .download_file_gz(&tmp, local_path, Some(progress_cb))
        .await;
    #[cfg(debug_assertions)]
    if let Err(ref e) = res {
        tracing::warn!("SFTP download file: transfer failed: {e}");
    }
    backend.return_sftp(s, res).await?;

    // 3. Clean up the remote tmp regardless of step 2's outcome.
    backend
        .emit_progress(SftpTransferKind::Download, SftpTransferStage::CleanUp, &tmp)
        .await;
    let h = shared.lock().await;
    let _ = exec_with_status(&h, &format!("rm -f -- {}", shell_quote(&tmp))).await;
    Ok(())
}

/// Upload a single local file to `remote_path` using gzip staging.
///
/// Steps:
///   1. SFTP-stream-upload the local file with in-flight gzip into
///      `/tmp/crabport-XXXX.gz`.
///   2. `ssh exec`: `gunzip -c -- /tmp/crabport-XXXX.gz > <remote>`
///   3. `ssh exec`: `rm -f -- /tmp/crabport-XXXX.gz` (folded into step 2 on
///      success, run separately on failure)
async fn sftp_upload_file_impl(
    backend: &SftpTransferHandle,
    local_path: &str,
    remote_path: &str,
) -> anyhow::Result<()> {
    let tmp = remote_tmp_path();

    // Stat the local file so we have a byte total for the progress bar.
    // `upload_file_gz` reports bytes fed into the encoder (original size),
    // so the total is the original local file size.
    let total = std::fs::metadata(local_path).map(|m| m.len()).unwrap_or(0);
    let progress_cb = backend.make_byte_progress_cb(
        SftpTransferKind::Upload,
        SftpTransferStage::Transfer,
        local_path.to_string(),
        total,
    );
    progress_cb(0);

    // 1. Stream-compress the local file up to the remote tmp .gz.
    tracing::info!(
        "SFTP upload file: step 1 transfer+compress local={local_path} -> remote_tmp={tmp} total={total}"
    );
    let s = backend.take_or_open_sftp().await?;
    let upload_res = s.upload_file_gz(local_path, &tmp, Some(progress_cb)).await;
    backend.return_sftp(s, upload_res).await?;

    // Acquire the SSH handle for the exec step.
    let handle_guard = backend.handle.lock().await;
    let shared = handle_guard
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("SSH handle not available"))?
        .clone();
    drop(handle_guard);
    let h = shared.lock().await;

    // 2. Decompress remotely into the final destination.
    backend
        .emit_progress(
            SftpTransferKind::Upload,
            SftpTransferStage::Decompress,
            remote_path,
        )
        .await;
    let cmd = format!(
        "gunzip -c -- {tmp_q} > {remote_q} && rm -f -- {tmp_q} && printf ok",
        tmp_q = shell_quote(&tmp),
        remote_q = shell_quote(remote_path),
    );
    let (code, out) = exec_with_status(&h, &cmd).await;
    if code != 0 || !out.ends_with("ok") {
        // Best-effort cleanup of the tmp file on failure.
        backend
            .emit_progress(SftpTransferKind::Upload, SftpTransferStage::CleanUp, &tmp)
            .await;
        let _ = exec_with_status(&h, &format!("rm -f -- {}", shell_quote(&tmp))).await;
        return Err(anyhow::anyhow!("remote gunzip failed (exit {code}): {out}"));
    }

    Ok(())
}

/// Conventional shell exit code for "command not found". Used to detect
/// when the remote lacks `tar` so we can fall back to recursive SFTP.
const EXIT_COMMAND_NOT_FOUND: u32 = 127;

/// Download a remote directory into `local_path`.
///
/// Primary path (1A): `tar czf` remotely → SFTP download `.tar.gz` →
/// client `tar::Archive::unpack`.
///
/// Fallback (1B): if the remote `tar` is missing (exit 127), recurse via
/// pure SFTP `read_dir` + per-file [`sftp_download_file_impl`].
async fn sftp_download_dir_impl(
    backend: &SftpTransferHandle,
    remote_path: &str,
    local_path: &str,
) -> anyhow::Result<()> {
    match sftp_download_dir_via_tar(backend, remote_path, local_path).await {
        Ok(()) => Ok(()),
        Err(e) if e.downcast_ref::<RemoteCommandNotFound>().is_some() => {
            tracing::warn!(
                "SFTP download: remote tar unavailable, falling back to recursive SFTP ({e})"
            );
            sftp_download_dir_recursive(backend, remote_path, local_path).await
        }
        Err(e) => Err(e),
    }
}

/// Upload a local directory to `remote_path`.
///
/// Primary path (1A): client `tar+gz` → SFTP upload `.tar.gz` →
/// `tar xzf` remotely.
///
/// Fallback (1B): if the remote `tar` is missing (exit 127), recurse via
/// pure SFTP `create_dir` + per-file [`sftp_upload_file_impl`].
async fn sftp_upload_dir_impl(
    backend: &SftpTransferHandle,
    local_path: &str,
    remote_path: &str,
) -> anyhow::Result<()> {
    match sftp_upload_dir_via_tar(backend, local_path, remote_path).await {
        Ok(()) => Ok(()),
        Err(e) if e.downcast_ref::<RemoteCommandNotFound>().is_some() => {
            tracing::warn!(
                "SFTP upload: remote tar unavailable, falling back to recursive SFTP ({e})"
            );
            sftp_upload_dir_recursive(backend, local_path, remote_path).await
        }
        Err(e) => Err(e),
    }
}

/// Marker error type for "the remote shell reported command not found",
/// used to trigger the recursive-SFTP fallback. We implement `Error`
/// manually (rather than pulling in `thiserror`) so the dependency surface
/// stays minimal.
#[derive(Debug)]
struct RemoteCommandNotFound(String);

impl std::fmt::Display for RemoteCommandNotFound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "remote command not found (exit 127): {}", self.0)
    }
}

impl std::error::Error for RemoteCommandNotFound {}

/// Directory download via remote `tar czf` + client unpack.
async fn sftp_download_dir_via_tar(
    backend: &SftpTransferHandle,
    remote_path: &str,
    local_path: &str,
) -> anyhow::Result<()> {
    let tmp = remote_tmp_path();

    // Split remote_path into parent + basename so we can run
    // `tar czf tmp -C <parent> <basename>`, which packs the directory
    // without its absolute path prefix.
    let (remote_parent, remote_base) = split_parent_basename(remote_path)?;

    let handle_guard = backend.handle.lock().await;
    let shared = handle_guard
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("SSH handle not available"))?
        .clone();
    drop(handle_guard);
    let h = shared.lock().await;

    // 1. Compress the remote directory into a tmp .tar.gz.
    backend
        .emit_progress(
            SftpTransferKind::Download,
            SftpTransferStage::Compress,
            remote_path,
        )
        .await;
    let cmd = format!(
        "tar czf {tmp_q} -C {parent_q} {base_q} && printf ok",
        tmp_q = shell_quote(&tmp),
        parent_q = shell_quote(&remote_parent),
        base_q = shell_quote(&remote_base),
    );
    #[cfg(debug_assertions)]
    tracing::info!("SFTP download dir: tar czf cmd={cmd}");
    let (code, out) = exec_with_status(&h, &cmd).await;
    if code == EXIT_COMMAND_NOT_FOUND {
        return Err(RemoteCommandNotFound(out).into());
    }
    if code != 0 || !out.ends_with("ok") {
        return Err(anyhow::anyhow!(
            "remote tar czf failed (exit {code}): {out}"
        ));
    }
    drop(h);

    // 2. Download + unpack. Stat the remote .tar.gz for the progress bar total.
    let total = {
        let s = backend.take_or_open_sftp().await?;
        let meta_res = s.metadata(&tmp).await;
        let total = match &meta_res {
            Ok(m) => m.size.unwrap_or(0),
            Err(_) => 0,
        };
        backend.return_sftp(s, meta_res.map(|_| ())).await?;
        total
    };
    let progress_cb = backend.make_byte_progress_cb(
        SftpTransferKind::Download,
        SftpTransferStage::Transfer,
        local_path.to_string(),
        total,
    );
    progress_cb(0);
    let s = backend.take_or_open_sftp().await?;
    // The tar archive's top-level entry is the remote basename, so unpacking
    // into `local_path` would add an extra nesting level. Unpack into
    // `local_path`'s parent instead.
    let unpack_dir = std::path::Path::new(local_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let unpack_dir_str = unpack_dir.to_string_lossy().into_owned();
    let res = s
        .download_dir(&tmp, &unpack_dir_str, Some(progress_cb))
        .await;
    backend.return_sftp(s, res).await?;

    // 3. Clean up the remote tmp.
    backend
        .emit_progress(SftpTransferKind::Download, SftpTransferStage::CleanUp, &tmp)
        .await;
    let h = shared.lock().await;
    let _ = exec_with_status(&h, &format!("rm -f -- {}", shell_quote(&tmp))).await;
    Ok(())
}

/// Directory upload via client `tar+gz` + remote `tar xzf`.
async fn sftp_upload_dir_via_tar(
    backend: &SftpTransferHandle,
    local_path: &str,
    remote_path: &str,
) -> anyhow::Result<()> {
    let tmp = remote_tmp_path();

    // The archive's top-level entry is the remote basename so that
    // `tar xzf tmp -C <remote_parent>` extracts to `<remote_parent>/<remote_base>`.
    let (remote_parent, remote_base) = split_parent_basename(remote_path)?;

    // 1. Client: build tar.gz and upload it to the remote tmp path.
    //    We don't know the compressed archive size until after `upload_dir`
    //    builds it internally, so pass total=0 — the progress bar will
    //    render indeterminate for directory uploads. (The byte counter still
    //    ticks, just without a meaningful percentage.)
    let progress_cb = backend.make_byte_progress_cb(
        SftpTransferKind::Upload,
        SftpTransferStage::Transfer,
        local_path.to_string(),
        0,
    );
    progress_cb(0);
    let s = backend.take_or_open_sftp().await?;
    let res = s
        .upload_dir(local_path, &tmp, &remote_base, Some(progress_cb))
        .await;
    backend.return_sftp(s, res).await?;

    // 2. Remote: ensure the target parent exists, then extract.
    backend
        .emit_progress(
            SftpTransferKind::Upload,
            SftpTransferStage::Decompress,
            remote_path,
        )
        .await;
    let handle_guard = backend.handle.lock().await;
    let shared = handle_guard
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("SSH handle not available"))?
        .clone();
    drop(handle_guard);
    let h = shared.lock().await;

    let cmd = format!(
        "mkdir -p {parent_q} && tar xzf {tmp_q} -C {parent_q} && rm -f -- {tmp_q} && printf ok",
        parent_q = shell_quote(&remote_parent),
        tmp_q = shell_quote(&tmp),
    );
    let (code, out) = exec_with_status(&h, &cmd).await;
    if code == EXIT_COMMAND_NOT_FOUND {
        // Best-effort cleanup of the tmp file before falling back.
        backend
            .emit_progress(SftpTransferKind::Upload, SftpTransferStage::CleanUp, &tmp)
            .await;
        let _ = exec_with_status(&h, &format!("rm -f -- {}", shell_quote(&tmp))).await;
        return Err(RemoteCommandNotFound(out).into());
    }
    if code != 0 || !out.ends_with("ok") {
        backend
            .emit_progress(SftpTransferKind::Upload, SftpTransferStage::CleanUp, &tmp)
            .await;
        let _ = exec_with_status(&h, &format!("rm -f -- {}", shell_quote(&tmp))).await;
        return Err(anyhow::anyhow!(
            "remote tar xzf failed (exit {code}): {out}"
        ));
    }

    Ok(())
}

/// Fallback directory download: recurse via pure SFTP, no remote `tar`.
async fn sftp_download_dir_recursive(
    backend: &SftpTransferHandle,
    remote_path: &str,
    local_path: &str,
) -> anyhow::Result<()> {
    std::fs::create_dir_all(local_path)?;

    let s = backend.take_or_open_sftp().await?;
    // Borrow the session for the listing, then return it. We re-acquire per
    // file below (the per-file impl does its own take/return).
    let entries = s.read_dir(remote_path).await;
    // Capture whether the listing succeeded so we can return the session
    // without consuming the entries we want to iterate.
    let entries = match entries {
        Ok(e) => {
            backend.return_sftp(s, Ok(())).await?;
            e
        }
        Err(e) => {
            let msg = format!("remote read_dir failed: {e}");
            backend.return_sftp(s, Err(e)).await?;
            return Err(anyhow::anyhow!(msg));
        }
    };

    for (name, is_dir) in entries {
        if name == "." || name == ".." {
            continue;
        }
        let remote_child = join_remote_path(remote_path, &name);
        let local_child = std::path::Path::new(local_path).join(&name);
        if is_dir {
            Box::pin(sftp_download_dir_recursive(
                backend,
                &remote_child,
                local_child.to_str().unwrap(),
            ))
            .await?;
        } else {
            // Recursive fallback: we don't pre-stat each child, so pass 0
            // as the size — the progress bar will render indeterminate
            // (no total) for these per-file transfers.
            sftp_download_file_impl(backend, &remote_child, local_child.to_str().unwrap(), 0)
                .await?;
        }
    }
    Ok(())
}

/// Fallback directory upload: recurse via pure SFTP, no remote `tar`.
async fn sftp_upload_dir_recursive(
    backend: &SftpTransferHandle,
    local_path: &str,
    remote_path: &str,
) -> anyhow::Result<()> {
    // Ensure the remote target directory exists.
    {
        let s = backend.take_or_open_sftp().await?;
        let res = s.create_dir(remote_path).await;
        // `create_dir` fails if the dir already exists — treat that as ok.
        let res = res.or_else(|e| {
            // SFTP returns Failure(4) for "failure" which covers
            // already-exists; we don't have the status code here so just
            // log and continue.
            tracing::debug!("remote mkdir {remote_path} returned {e} (assuming exists)");
            Ok(())
        });
        backend.return_sftp(s, res).await?;
    }

    let mut entries = tokio::fs::read_dir(local_path).await?;
    while let Some(entry) = entries.next_entry().await? {
        let name = entry.file_name().to_string_lossy().into_owned();
        let local_child = entry.path();
        let remote_child = join_remote_path(remote_path, &name);
        let file_type = entry.file_type().await?;
        if file_type.is_dir() {
            Box::pin(sftp_upload_dir_recursive(
                backend,
                local_child.to_str().unwrap(),
                &remote_child,
            ))
            .await?;
        } else if file_type.is_file() {
            sftp_upload_file_impl(backend, local_child.to_str().unwrap(), &remote_child).await?;
        }
        // Symlinks and other types are skipped.
    }
    Ok(())
}

/// Split a remote POSIX path into `(parent, basename)`. The parent of a
/// top-level path like `/foo` is `/`.
fn split_parent_basename(path: &str) -> anyhow::Result<(String, String)> {
    let p = std::path::Path::new(path);
    let basename = p
        .file_name()
        .ok_or_else(|| anyhow::anyhow!("remote path has no basename: {path}"))?
        .to_string_lossy()
        .into_owned();
    let parent = p
        .parent()
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|| "/".to_string());
    let parent = if parent.is_empty() {
        "/".to_string()
    } else {
        parent
    };
    Ok((parent, basename))
}

/// Join a remote directory path with an entry name, inserting a `/` separator
/// only when needed (i.e. not when the parent already ends with one).
fn join_remote_path(parent: &str, name: &str) -> String {
    if parent.ends_with('/') {
        format!("{parent}{name}")
    } else {
        format!("{parent}/{name}")
    }
}

// ---------------------------------------------------------------------------
// Private key decoding
// ---------------------------------------------------------------------------

fn decode_private_key(
    key_str: &str,
    passphrase: Option<&str>,
) -> Result<KeyPair, Box<dyn std::error::Error + Send + Sync>> {
    // Try PEM-encoded key first (OpenSSH format: "-----BEGIN OPENSSH PRIVATE KEY-----")
    if key_str.contains("BEGIN") {
        let pair = russh::keys::decode_secret_key(key_str, passphrase)?;
        return Ok(pair);
    }

    // Otherwise treat as a raw key file path or content — try as file path first
    if let Ok(content) = std::fs::read_to_string(key_str) {
        let pair = russh::keys::decode_secret_key(&content, passphrase)?;
        return Ok(pair);
    }

    Err("cannot decode private key: not a valid PEM key or file path".into())
}

// ---------------------------------------------------------------------------
// CrabPortTerminal impl
// ---------------------------------------------------------------------------

impl CrabPortTerminal for SshBackend {
    fn write(&self, data: &[u8]) {
        let _ = self.command_tx.try_send(Command::Write(data.to_vec()));
    }

    fn resize(&self, cols: u16, rows: u16) {
        let _ = self.command_tx.try_send(Command::Resize(cols, rows));
    }

    fn close(&self) {
        let _ = self.command_tx.try_send(Command::Close);
    }

    fn subscribe(&self) -> BroadcastReceiver<BackendEvent> {
        self.event_tx.new_receiver()
    }

    fn as_monitor(&self) -> Option<&dyn CrabPortMonitor> {
        Some(self)
    }

    fn allow_sftp(&self) -> bool {
        true
    }

    fn sftp_entries(&self) -> Option<Arc<Vec<(String, bool)>>> {
        self.sftp_entries.read().clone()
    }

    fn sftp_cwd(&self) -> Option<Arc<String>> {
        self.sftp_cwd.read().clone()
    }

    fn sftp_navigate(&self, path: &str) {
        let handle = self.handle.clone();
        let entries = self.sftp_entries.clone();
        let cwd = self.sftp_cwd.clone();
        let sftp_session = self.sftp_session.clone();
        let path = path.to_string();
        TOKIO.spawn(async move {
            // Reuse the cached SFTP session if we still have one. Only
            // (re)connect when the cache is empty — e.g. on the very first
            // navigate after a connect that didn't establish SFTP, or after
            // the session was dropped following an error. This avoids paying
            // the ~24ms SFTP handshake on every directory change.
            let sftp = {
                let mut guard = sftp_session.lock().await;
                if guard.is_none() {
                    let hg = handle.lock().await;
                    let Some(h) = hg.as_ref() else {
                        return;
                    };
                    let h = h.lock().await;
                    match crabport_sftp::SftpBackend::connect(&*h).await {
                        Ok(s) => *guard = Some(s),
                        Err(e) => {
                            tracing::warn!("SFTP navigate: connect failed ({e})");
                            return;
                        }
                    }
                }
                // Take the session out of the cache for the duration of this
                // operation so concurrent navigations don't fight over the
                // same channel. We put it back (or drop it on error) below.
                guard.take().expect("just ensured Some")
            };

            // Resolve the target path
            let resolved = match sftp.canonicalize(&path).await {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("SFTP navigate: canonicalize '{}' failed ({e})", path);
                    // Drop the session — the channel may be dead.
                    let _ = sftp.close().await;
                    return;
                }
            };
            match sftp.read_dir(&resolved).await {
                Ok(dir_entries) => {
                    *entries.write() = Some(Arc::new(dir_entries));
                    *cwd.write() = Some(Arc::new(resolved));
                    // Return the live session to the cache.
                    *sftp_session.lock().await = Some(sftp);
                }
                Err(e) => {
                    tracing::warn!("SFTP navigate: read_dir failed ({e})");
                    let _ = sftp.close().await;
                }
            }
        });
    }

    fn sftp_download(&self, remote_path: &str, local_path: &str) {
        let backend = SftpTransferHandle {
            handle: self.handle.clone(),
            sftp_session: self.sftp_session.clone(),
            event_tx: Some(self.event_tx.clone()),
        };
        let event_tx = self.event_tx.clone();
        let remote_path = remote_path.to_string();
        let local_path = local_path.to_string();
        TOKIO.spawn(async move {
            let result = sftp_download_impl(&backend, &remote_path, &local_path).await;
            let (success, message) = match &result {
                Ok(()) => (true, format!("downloaded {local_path}")),
                Err(e) => (false, format!("download failed: {e}")),
            };
            let _ = event_tx
                .broadcast(BackendEvent::SftpTransferFinished {
                    kind: SftpTransferKind::Download,
                    success,
                    message,
                })
                .await;
        });
    }

    fn sftp_upload(&self, local_path: &str, remote_path: &str) {
        let backend = SftpTransferHandle {
            handle: self.handle.clone(),
            sftp_session: self.sftp_session.clone(),
            event_tx: Some(self.event_tx.clone()),
        };
        let event_tx = self.event_tx.clone();
        let remote_path = remote_path.to_string();
        let local_path = local_path.to_string();
        TOKIO.spawn(async move {
            let result = sftp_upload_impl(&backend, &local_path, &remote_path).await;
            let (success, message) = match result {
                Ok(()) => (true, format!("uploaded {remote_path}")),
                Err(e) => (false, format!("upload failed: {e}")),
            };
            let _ = event_tx
                .broadcast(BackendEvent::SftpTransferFinished {
                    kind: SftpTransferKind::Upload,
                    success,
                    message,
                })
                .await;
        });
    }

    fn sftp_delete(&self, remote_path: &str) {
        // Reuse the SftpTransferHandle so we get the cached session + event
        // sink. There's no actual transfer, but we emit a `SftpTransferFinished`
        // so the existing UI finish handling (toolbar clear, overlay log)
        // applies. We use the Download kind arbitrarily — the message text
        // carries the real semantics.
        let backend = SftpTransferHandle {
            handle: self.handle.clone(),
            sftp_session: self.sftp_session.clone(),
            event_tx: Some(self.event_tx.clone()),
        };
        let event_tx = self.event_tx.clone();
        let remote_path = remote_path.to_string();
        TOKIO.spawn(async move {
            let result = sftp_delete_impl(&backend, &remote_path).await;
            let (success, message) = match result {
                Ok(()) => (true, format!("deleted {remote_path}")),
                Err(e) => (false, format!("delete failed: {e}")),
            };
            let _ = event_tx
                .broadcast(BackendEvent::SftpTransferFinished {
                    kind: SftpTransferKind::Download,
                    success,
                    message,
                })
                .await;
        });
    }
}

// ---------------------------------------------------------------------------
// CrabPortMonitor impl
// ---------------------------------------------------------------------------

impl CrabPortMonitor for SshBackend {
    fn status(&self) -> RemoteStatus {
        self.monitor.read().status
    }

    fn metrics(&self) -> RemoteMetrics {
        self.monitor.read().metrics
    }
}

impl Drop for SshBackend {
    fn drop(&mut self) {
        let _ = self.command_tx.try_send(Command::Close);
    }
}
