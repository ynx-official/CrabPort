use std::sync::Arc;

use parking_lot::RwLock;
use russh::{ChannelMsg, client};
use tokio::sync::Mutex as TokioMutex;

use crabport_terminal::terminal::{MemoryStats, NetworkStats, RemoteMetrics, RemoteStatus};

use crate::backend::MonitorState;
use crate::handler::SshHandler;
use crate::session::SshConnectionInfo;

// ---------------------------------------------------------------------------
// Monitor loop — periodically collects latency / memory / network via SSH exec
// ---------------------------------------------------------------------------

pub(crate) async fn monitor_loop(
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
pub(crate) async fn measure_latency(handle: &client::Handle<SshHandler>) -> Option<u32> {
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
pub(crate) async fn exec_and_read(
    handle: &client::Handle<SshHandler>,
    cmd: &str,
) -> Option<String> {
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
pub(crate) async fn exec_with_status(
    handle: &client::Handle<SshHandler>,
    cmd: &str,
) -> (u32, String) {
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
