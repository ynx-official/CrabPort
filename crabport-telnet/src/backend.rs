//! `TelnetBackend` — raw-TCP + minimal-IAC telnet transport.
//!
//! Connects via [`crabport_proxy::connect`] (direct / SOCKS5 / HTTP CONNECT /
//! HTTPS CONNECT) so proxy support is shared with SSH for free. The resulting
//! stream is a tokio `AsyncRead + AsyncWrite`, so — like `crabport_ssh` — we
//! drive all I/O on a shared tokio runtime (`TOKIO`) and bridge to the
//! frontend's `broadcast`/`async-channel` primitives.
//!
//! Wire format (RFC 854): the reader task runs a small state machine that
//! strips IAC command bytes from the visible output and answers every option
//! negotiation by refusing it (`WILL → DONT`, `DO → WONT`). Refusing keeps
//! the session in the safe default NVT (network virtual terminal) mode and
//! avoids option-enable loops. Subnegotiations (`SB … IAC SE`) are skipped
//! silently. Auth is intentionally not automated for v1 — the server's
//! `login:` / `Password:` prompts reach the terminal and the user types into
//! them, matching how standalone telnet clients behave.

use std::sync::{Arc, LazyLock};

use async_broadcast::{InactiveReceiver, Sender as BroadcastSender, broadcast};
use async_channel::{Sender as MpscSender, unbounded};
use parking_lot::RwLock;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::{runtime::Runtime, select};

use crabport_terminal::terminal::{
    BackendEvent, CrabPortMonitor, CrabPortTerminal, RemoteMetrics, RemoteStatus,
};

use crate::session::TelnetConnectionInfo;

// ---------------------------------------------------------------------------
// Telnet protocol constants (RFC 854)
// ---------------------------------------------------------------------------

const IAC: u8 = 255;
const DONT: u8 = 254;
const DO: u8 = 253;
const WONT: u8 = 252;
const WILL: u8 = 251;
const SB: u8 = 250;
const SE: u8 = 240;

// ---------------------------------------------------------------------------
// Tokio runtime (the proxy stream is tokio-based, same as SSH)
// ---------------------------------------------------------------------------

/// Tokio runtime shared by all telnet backends in this process. The proxy
/// crate returns a tokio `AsyncRead + AsyncWrite` stream, so we need a tokio
/// runtime to drive it — same rationale as `crabport_ssh::backend::TOKIO`.
pub static TOKIO: LazyLock<Runtime> =
    LazyLock::new(|| Runtime::new().expect("failed to create tokio runtime for telnet"));

// ---------------------------------------------------------------------------
// Internal command queue (frontend → backend)
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum Command {
    Write(Vec<u8>),
    Resize(u16, u16),
    Close,
}

// ---------------------------------------------------------------------------
// Shared monitor state
// ---------------------------------------------------------------------------

/// State shared between the I/O tasks and the `CrabPortMonitor` impl.
struct MonitorState {
    status: RemoteStatus,
    metrics: RemoteMetrics,
}

// ---------------------------------------------------------------------------
// IAC parser state machine
// ---------------------------------------------------------------------------

/// Parser state carried across read boundaries. Telnet IAC sequences may
/// straddle `read` calls, so we can't process a buffer in isolation.
#[derive(Clone, Copy, PartialEq)]
enum IacState {
    /// Normal visible data.
    Normal,
    /// Just consumed an IAC (255); the next byte is the command.
    Iac,
    /// Consumed `IAC WILL`; next byte is the option being offered.
    Will,
    /// Consumed `IAC WONT`; next byte is the option (no reply needed).
    Wont,
    /// Consumed `IAC DO`; next byte is the option being requested.
    Do,
    /// Consumed `IAC DONT`; next byte is the option (no reply needed).
    Dont,
    /// Inside a subnegotiation (`IAC SB …`); skip until `IAC SE`.
    Sb,
    /// Inside a subnegotiation and just consumed an IAC; next byte is either
    /// SE (end) or 255 (escaped literal) or another command.
    SbIac,
}

// ---------------------------------------------------------------------------
// TelnetBackend
// ---------------------------------------------------------------------------

/// Telnet terminal backend.
///
/// Connects over raw TCP (optionally through a proxy), strips IAC
/// negotiations from the visible stream, and bridges the resulting byte
/// stream to the frontend via the standard `CrabPortTerminal` trait.
pub struct TelnetBackend {
    command_tx: MpscSender<Command>,
    event_tx: BroadcastSender<BackendEvent>,
    _event_rx: InactiveReceiver<BackendEvent>,
    monitor: Arc<RwLock<MonitorState>>,
    #[allow(dead_code)]
    _on_status: Arc<dyn Fn(String) + Send + Sync>,
}

impl TelnetBackend {
    /// Create and connect a telnet backend.
    ///
    /// `on_status` receives human-readable connection-state updates
    /// ("Connecting to …", "TCP connection established", …) mirroring
    /// `SshBackend::new`.
    pub fn new(
        info: TelnetConnectionInfo,
        cols: u16,
        rows: u16,
        on_status: Arc<dyn Fn(String) + Send + Sync>,
    ) -> Self {
        let (event_tx, event_rx) = broadcast(1024);
        let _event_rx = event_rx.deactivate();
        let (command_tx, command_rx) = unbounded::<Command>();
        // Reader → command-loop channel for IAC negotiation replies.
        let (resp_tx, resp_rx) = unbounded::<Vec<u8>>();

        let monitor = Arc::new(RwLock::new(MonitorState {
            status: RemoteStatus::Connecting,
            metrics: RemoteMetrics::default(),
        }));

        let event_tx2 = event_tx.clone();
        let monitor2 = monitor.clone();
        let on_status2 = on_status.clone();

        TOKIO.spawn(async move {
            let addr = format!("{}:{}", info.host, info.port);
            #[cfg(debug_assertions)]
            tracing::info!("telnet: connecting to {}", addr);
            on_status2(format!("Connecting to {}", addr));

            let stream = match crabport_proxy::connect(&info.proxy, &info.host, info.port).await {
                Ok(s) => {
                    on_status2("TCP connection established".into());
                    s
                }
                Err(e) => {
                    tracing::error!("telnet: connect failed: {e}");
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

            // Split so the reader and the command loop can run concurrently.
            let (read_half, write_half) = tokio::io::split(stream);

            // ---- Reader task ----
            // Reads from the socket, runs the IAC state machine, broadcasts
            // visible output, and forwards negotiation replies to the command
            // loop via `resp_tx` (so all writes are serialized on one task).
            {
                let event_tx = event_tx2.clone();
                let monitor = monitor2.clone();
                let resp_tx = resp_tx;
                let _on_status = on_status2.clone();

                TOKIO.spawn(async move {
                    let _ = _on_status;
                    let mut reader = read_half;
                    let mut buf = [0u8; 8192];
                    let mut state = IacState::Normal;
                    let mut data_out: Vec<u8> = Vec::with_capacity(8192);

                    loop {
                        match reader.read(&mut buf).await {
                            Ok(0) => {
                                #[cfg(debug_assertions)]
                                tracing::info!("telnet reader: EOF");
                                flush_data(&event_tx, &mut data_out).await;
                                {
                                    let mut m = monitor.write();
                                    m.status = RemoteStatus::Disconnected;
                                }
                                let _ = event_tx.broadcast(BackendEvent::Closed).await;
                                break;
                            }
                            Ok(n) => {
                                process_iac(&buf[..n], &mut state, &mut data_out, &resp_tx);

                                if !data_out.is_empty() {
                                    let chunk = std::mem::take(&mut data_out);
                                    #[cfg(debug_assertions)]
                                    tracing::debug!("telnet reader: {} data bytes", chunk.len());
                                    let _ = event_tx.broadcast(BackendEvent::Data(chunk)).await;
                                }
                            }
                            Err(e) => {
                                tracing::error!("telnet reader error: {e}");
                                flush_data(&event_tx, &mut data_out).await;
                                {
                                    let mut m = monitor.write();
                                    m.status = RemoteStatus::Disconnected;
                                }
                                let _ =
                                    event_tx.broadcast(BackendEvent::Error(e.to_string())).await;
                                break;
                            }
                        }
                    }
                });
            }

            // ---- Command loop (writes + resize + close) ----
            let mut writer = write_half;
            loop {
                select! {
                    biased;

                    // Negotiation replies from the reader — drain first so the
                    // server doesn't time out waiting on our WONT/DONT.
                    Ok(resp) = resp_rx.recv() => {
                        if let Err(e) = writer.write_all(&resp).await {
                            #[cfg(debug_assertions)]
                            tracing::warn!("telnet: IAC reply write error: {e}");
                        }
                        let _ = writer.flush().await;
                    }

                    cmd = command_rx.recv() => {
                        match cmd {
                            Ok(Command::Write(data)) => {
                                if let Err(e) = writer.write_all(&data).await {
                                    #[cfg(debug_assertions)]
                                    tracing::warn!("telnet: write error: {e}");
                                }
                                let _ = writer.flush().await;
                            }
                            Ok(Command::Resize(_cols, _rows)) => {
                                // v1: no-op. Proper NAWS would negotiate
                                // `IAC WILL NAWS` then send
                                // `IAC SB NAWS cols rows IAC SE`; left for a
                                // follow-up since most telnet servers tolerate
                                // a missing window size.
                                #[cfg(debug_assertions)]
                                tracing::debug!("telnet: resize (no-op for v1)");
                            }
                            Ok(Command::Close) | Err(_) => {
                                #[cfg(debug_assertions)]
                                tracing::info!("telnet: closing connection");
                                let _ = writer.shutdown().await;
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

        // Silence unused-variable warnings for cols/rows in release builds
        // where the debug resize log is compiled out.
        let _ = (cols, rows);

        // Silence unused-variable warnings for cols/rows in release builds
        // where the debug resize log is compiled out.
        let _ = (cols, rows);

        Self {
            command_tx,
            event_tx,
            _event_rx,
            monitor,
            _on_status: on_status,
        }
    }
}

/// Flush any accumulated visible data before emitting a terminal event.
async fn flush_data(event_tx: &BroadcastSender<BackendEvent>, data_out: &mut Vec<u8>) {
    if !data_out.is_empty() {
        let chunk = std::mem::take(data_out);
        let _ = event_tx.broadcast(BackendEvent::Data(chunk)).await;
    }
}

/// Run the IAC state machine over one read chunk.
///
/// - Visible bytes are appended to `data_out` (caller broadcasts them).
/// - Negotiation replies are forwarded to `resp_tx` (caller's command loop
///   writes them to the socket, keeping all writes on one task).
fn process_iac(
    chunk: &[u8],
    state: &mut IacState,
    data_out: &mut Vec<u8>,
    resp_tx: &MpscSender<Vec<u8>>,
) {
    let mut resp: Vec<u8> = Vec::new();

    for &b in chunk {
        match *state {
            IacState::Normal => {
                if b == IAC {
                    *state = IacState::Iac;
                } else {
                    data_out.push(b);
                }
            }
            IacState::Iac => match b {
                IAC => {
                    // Escaped 255 → literal 0xff in the data stream.
                    data_out.push(IAC);
                    *state = IacState::Normal;
                }
                WILL => *state = IacState::Will,
                WONT => *state = IacState::Wont,
                DO => *state = IacState::Do,
                DONT => *state = IacState::Dont,
                SB => *state = IacState::Sb,
                // SE / NOP / DM / BRK / … — single-byte commands we don't
                // act on; consume and return to normal data.
                _ => *state = IacState::Normal,
            },
            IacState::Will => {
                // Server offers to enable option `b`; refuse → `IAC DONT b`.
                resp.extend_from_slice(&[IAC, DONT, b]);
                *state = IacState::Normal;
            }
            IacState::Wont => {
                // Server says it won't; nothing to do (already disabled).
                *state = IacState::Normal;
            }
            IacState::Do => {
                // Server asks us to enable option `b`; refuse → `IAC WONT b`.
                resp.extend_from_slice(&[IAC, WONT, b]);
                *state = IacState::Normal;
            }
            IacState::Dont => {
                // Server asks us to disable; nothing to do.
                *state = IacState::Normal;
            }
            IacState::Sb => {
                if b == IAC {
                    *state = IacState::SbIac;
                }
                // Otherwise stay in Sb, skipping subneg body.
            }
            IacState::SbIac => match b {
                SE => *state = IacState::Normal,
                IAC => {
                    // Escaped 255 inside subneg; skip it, stay in subneg.
                    *state = IacState::Sb;
                }
                _ => *state = IacState::Sb,
            },
        }
    }

    if !resp.is_empty() {
        let _ = resp_tx.try_send(resp);
    }
}

// ---------------------------------------------------------------------------
// CrabPortTerminal impl
// ---------------------------------------------------------------------------

impl CrabPortTerminal for TelnetBackend {
    fn write(&self, data: &[u8]) {
        let _ = self.command_tx.try_send(Command::Write(data.to_vec()));
    }

    fn resize(&self, cols: u16, rows: u16) {
        let _ = self.command_tx.try_send(Command::Resize(cols, rows));
    }

    fn close(&self) {
        let _ = self.command_tx.try_send(Command::Close);
    }

    fn subscribe(&self) -> async_broadcast::Receiver<BackendEvent> {
        self.event_tx.new_receiver()
    }

    fn as_monitor(&self) -> Option<&dyn CrabPortMonitor> {
        Some(self)
    }

    fn allow_sftp(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// CrabPortMonitor impl
// ---------------------------------------------------------------------------

impl CrabPortMonitor for TelnetBackend {
    fn status(&self) -> RemoteStatus {
        self.monitor.read().status
    }

    fn metrics(&self) -> RemoteMetrics {
        self.monitor.read().metrics
    }
}

impl Drop for TelnetBackend {
    fn drop(&mut self) {
        let _ = self.command_tx.try_send(Command::Close);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(chunk: &[u8]) -> (Vec<u8>, Vec<Vec<u8>>) {
        // Collect data and any negotiation-reply batches sent via the
        // (unbounded) channel. The reader loop calls `process_iac` once per
        // read; we mirror that by invoking it once per chunk.
        let (tx, rx) = unbounded::<Vec<u8>>();
        let mut state = IacState::Normal;
        let mut data_out = Vec::new();
        process_iac(chunk, &mut state, &mut data_out, &tx);
        let mut replies = Vec::new();
        while let Ok(r) = rx.try_recv() {
            replies.push(r);
        }
        (data_out, replies)
    }

    #[test]
    fn plain_data_passes_through() {
        let (data, replies) = parse(b"hello\r\n");
        assert_eq!(data, b"hello\r\n");
        assert!(replies.is_empty());
    }

    #[test]
    fn escaped_iac_yields_literal_255() {
        let (data, _replies) = parse(&[IAC, IAC, b'x']);
        assert_eq!(data, vec![IAC, b'x']);
    }

    #[test]
    fn do_option_refused_with_wont() {
        let (_data, replies) = parse(&[IAC, DO, 31]); // DO NAWS
        assert_eq!(replies, vec![vec![IAC, WONT, 31]]);
    }

    #[test]
    fn will_option_refused_with_dont() {
        let (_data, replies) = parse(&[IAC, WILL, 1]); // WILL ECHO
        assert_eq!(replies, vec![vec![IAC, DONT, 1]]);
    }

    #[test]
    fn wont_and_dont_produce_no_reply() {
        let (_data, replies) = parse(&[IAC, WONT, 3, IAC, DONT, 5]);
        assert!(replies.is_empty());
    }

    #[test]
    fn subnegotiation_is_skipped() {
        let (data, replies) = parse(&[
            b'a', b'b', // visible
            IAC, SB, 31, 0, 80, 0, 24, IAC, SE, // NAWS subneg
            b'c',
        ]);
        assert_eq!(data, b"abc");
        assert!(replies.is_empty());
    }

    #[test]
    fn iac_split_across_chunks() {
        let (tx, rx) = unbounded::<Vec<u8>>();
        let mut state = IacState::Normal;
        let mut data = Vec::new();
        process_iac(&[b'x', IAC], &mut state, &mut data, &tx);
        process_iac(&[DO, 24], &mut state, &mut data, &tx);
        assert_eq!(data, b"x");
        let mut replies = Vec::new();
        while let Ok(r) = rx.try_recv() {
            replies.push(r);
        }
        assert_eq!(replies, vec![vec![IAC, WONT, 24]]);
    }
}
