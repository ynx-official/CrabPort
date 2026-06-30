use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use alacritty_terminal::{
    Term,
    event::{Event, EventListener},
    sync::FairMutex,
    term::{Config, test::TermSize},
    vte::ansi::{Processor, StdSyncHandler},
};
use async_broadcast::{
    InactiveReceiver, Receiver as BroadcastReceiver, Sender as BroadcastSender, broadcast,
};
use parking_lot::Mutex;
use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub enum BackendEvent {
    Data(Vec<u8>),
    Closed,
    Error(String),
    /// A file transfer (download or upload) finished.
    ///
    /// `kind` identifies which direction, `success` is true on completion
    /// and false on failure, and `message` is a short human-readable
    /// description (the destination path on success, the error text on
    /// failure).
    SftpTransferFinished {
        kind: SftpTransferKind,
        success: bool,
        message: String,
    },
    /// Live progress for an in-flight SFTP transfer. Emitted at each stage
    /// boundary of the gzip/tmp staging flow (compress → transfer →
    /// decompress → cleanup) so the UI can surface a stage-aware progress
    /// log. A `SftpTransferFinished` always follows the last progress
    /// event for a given transfer.
    SftpTransferProgress {
        kind: SftpTransferKind,
        stage: SftpTransferStage,
        /// Short human-readable detail — typically the path being worked
        /// on, so the user can tell which file in a batch is current.
        message: String,
        /// Byte-level progress for the current stage. `None` for stages
        /// that don't have a meaningful byte count (e.g. remote `gzip`
        /// which runs as an opaque exec). When present, the UI renders a
        /// determinate progress bar.
        bytes: Option<SftpTransferBytes>,
    },
}

/// Byte-level progress snapshot for a transfer stage.
#[derive(Debug, Clone, Copy)]
pub struct SftpTransferBytes {
    /// Bytes processed so far in the current stage.
    pub done: u64,
    /// Total bytes expected for the current stage. Zero means "unknown";
    /// the UI should render an indeterminate (animated) bar in that case.
    pub total: u64,
}

/// Which direction an SFTP transfer ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpTransferKind {
    Download,
    Upload,
}

/// A coarse stage in the gzip/tmp staging flow used by SFTP transfers.
///
/// The ordering reflects the typical sequence for a download (compress
/// remotely → stream the .gz down → decompress on the client → clean up
/// the remote tmp); uploads run the mirror image. Not every transfer
/// touches every stage — e.g. a recursive fallback skips compress/
/// decompress and goes straight to per-file transfer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SftpTransferStage {
    /// Compressing the source (remote `gzip -c` / `tar czf` for downloads,
    /// client-side `tar+gz` for uploads).
    Compress,
    /// Streaming the staged archive over SFTP (download or upload of the
    /// `.gz` / `.tar.gz`).
    Transfer,
    /// Decompressing the staged archive into its final location (client
    /// `tar::Archive::unpack` for downloads, remote `gunzip`/`tar xzf`
    /// for uploads).
    Decompress,
    /// Removing the remote tmp staging file. Best-effort; failures here
    /// don't fail the overall transfer.
    CleanUp,
}

pub trait CrabPortTerminal: Send + Sync {
    fn write(&self, data: &[u8]);
    fn resize(&self, cols: u16, rows: u16);
    fn close(&self);
    fn subscribe(&self) -> BroadcastReceiver<BackendEvent>;

    fn as_monitor(&self) -> Option<&dyn CrabPortMonitor> {
        None
    }

    /// Whether this backend supports SFTP.
    fn allow_sftp(&self) -> bool {
        false
    }

    /// Current SFTP directory entries. Returns None if not yet loaded.
    fn sftp_entries(&self) -> Option<std::sync::Arc<Vec<(String, bool)>>> {
        None
    }

    /// Current SFTP working directory. Returns None if not yet loaded.
    fn sftp_cwd(&self) -> Option<std::sync::Arc<String>> {
        None
    }

    /// Navigate to a new directory via SFTP. The backend updates entries + cwd
    /// asynchronously and notifies the UI.
    fn sftp_navigate(&self, _path: &str) {}

    /// Download a remote file to `local_path`, using implicit gzip staging
    /// (see `SshBackend::sftp_download`).
    ///
    /// Completion is reported via a [`BackendEvent::SftpTransferFinished`]
    /// event on the backend's event stream — the caller does not need to pass
    /// a callback.
    fn sftp_download(&self, _remote_path: &str, _local_path: &str) {}

    /// Upload `local_path` to `remote_path`, using implicit gzip staging
    /// (see `SshBackend::sftp_upload`). Completion is reported via
    /// [`BackendEvent::SftpTransferFinished`].
    fn sftp_upload(&self, _local_path: &str, _remote_path: &str) {}

    /// Delete a remote file or directory at `remote_path`. The backend
    /// stats the path to decide between `remove_file` and `remove_dir`.
    /// Completion is reported via [`BackendEvent::SftpTransferFinished`] with
    /// `kind = Delete` (a synthetic kind — there's no actual transfer, but
    /// we reuse the event so the UI's existing finish handling applies).
    fn sftp_delete(&self, _remote_path: &str) {}
}

// ---------------------------------------------------------------------------
// Remote performance monitoring
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RemoteStatus {
    Local,
    Connected,
    Connecting,
    Disconnected,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct NetworkStats {
    pub bytes_sent: u64,
    pub bytes_recv: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MemoryStats {
    pub total: u64,
    pub used: u64,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RemoteMetrics {
    pub latency_ms: Option<u32>,
    pub memory: Option<MemoryStats>,
    pub network: Option<NetworkStats>,
}

pub trait CrabPortMonitor: Send + Sync {
    fn status(&self) -> RemoteStatus;
    fn metrics(&self) -> RemoteMetrics;
}

#[derive(Clone)]
pub struct EventProxy {
    wakeup_tx: BroadcastSender<()>,
}

impl EventProxy {
    pub fn new(wakeup_tx: BroadcastSender<()>) -> Self {
        Self { wakeup_tx }
    }
}

impl EventListener for EventProxy {
    fn send_event(&self, event: Event) {
        match event {
            Event::Wakeup => {
                #[cfg(debug_assertions)]
                tracing::debug!("EventProxy: Wakeup event received");
                let _ = self.wakeup_tx.try_broadcast(());
            }
            _ => {
                #[cfg(debug_assertions)]
                tracing::debug!("EventProxy: Other event {:?}", event);
                let _ = self.wakeup_tx.try_broadcast(());
            }
        }
    }
}

/// Maximum number of commands retained per session. Matches the Store
/// limit so the in-memory buffer and the persisted table stay in sync.
/// Older entries are dropped once this limit is exceeded (LRU by most
/// recent use).
const MAX_COMMAND_HISTORY: usize = 300;

pub struct TerminalSession {
    backend: Arc<dyn CrabPortTerminal>,
    term: Arc<FairMutex<Term<EventProxy>>>,
    wakeup_tx: BroadcastSender<()>,
    started: AtomicBool,
    _wakeup_rx: InactiveReceiver<()>,
    /// Command history, most-recent-first. Capped at [`MAX_COMMAND_HISTORY`]
    /// entries; the oldest is evicted when full.
    command_history: Arc<Mutex<VecDeque<String>>>,
    /// In-progress input line being accumulated by [`Self::write`]. Submitted
    /// to `command_history` on Enter (CR/LF). Backspace deletes the last
    /// char; other control bytes are ignored.
    line_buffer: Arc<Mutex<String>>,
    /// Optional callback invoked whenever a new command is captured. The UI
    /// layer (TerminalView) uses this to persist the command to the Store
    /// — TerminalSession itself stays free of any storage dependency.
    /// Receives the captured command text.
    on_command: Arc<Mutex<Option<Arc<dyn Fn(&str) + Send + Sync>>>>,
}

impl TerminalSession {
    pub fn new(backend: Arc<dyn CrabPortTerminal>, cols: usize, rows: usize) -> Self {
        let (wakeup_tx, wakeup_rx) = broadcast(256);
        let _wakeup_rx = wakeup_rx.deactivate();

        let term = Arc::new(FairMutex::new(Term::new(
            Config::default(),
            &TermSize::new(cols, rows),
            EventProxy::new(wakeup_tx.clone()),
        )));

        Self {
            backend,
            term,
            wakeup_tx,
            started: AtomicBool::new(false),
            _wakeup_rx,
            command_history: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_COMMAND_HISTORY))),
            line_buffer: Arc::new(Mutex::new(String::new())),
            on_command: Arc::new(Mutex::new(None)),
        }
    }

    pub fn start(&self) {
        if self.started.swap(true, Ordering::SeqCst) {
            return;
        }

        let mut rx = self.backend.subscribe();
        let term = self.term.clone();
        let wakeup_tx = self.wakeup_tx.clone();

        smol::spawn(async move {
            let mut parser = Processor::<StdSyncHandler>::new();

            loop {
                match rx.recv().await {
                    Ok(event) => match event {
                        BackendEvent::Data(data) => {
                            #[cfg(debug_assertions)]
                            tracing::debug!("session: received {} bytes", data.len());
                            // Batch-drain: hold the term lock once and advance all
                            // currently-queued chunks. Cuts lock churn and wakeup
                            // storms when the PTY floods (cat / top / build logs).
                            let mut terminal = term.lock();
                            parser.advance(&mut *terminal, &data);
                            loop {
                                match rx.try_recv() {
                                    Ok(BackendEvent::Data(more)) => {
                                        parser.advance(&mut *terminal, &more);
                                    }
                                    Ok(BackendEvent::Closed) => {
                                        drop(terminal);
                                        let _ = wakeup_tx.try_broadcast(());
                                        return;
                                    }
                                    Ok(BackendEvent::Error(err)) => {
                                        tracing::error!("terminal backend error: {}", err);
                                    }
                                    Ok(BackendEvent::SftpTransferFinished { .. }) => {
                                        // UI-only event; ignore during batch drain.
                                    }
                                    Ok(BackendEvent::SftpTransferProgress { .. }) => {
                                        // UI-only event; ignore during batch drain.
                                    }
                                    Err(_) => break, // queue drained
                                }
                            }
                            drop(terminal);
                            let _ = wakeup_tx.try_broadcast(());
                        }
                        BackendEvent::Closed => {
                            #[cfg(debug_assertions)]
                            tracing::info!("session: backend closed");
                            let _ = wakeup_tx.try_broadcast(());
                            break;
                        }
                        BackendEvent::Error(err) => {
                            tracing::error!("terminal backend error: {}", err);
                            let _ = wakeup_tx.try_broadcast(());
                        }
                        // Transfer-finished events are for the UI, not the
                        // terminal parser. Ignore them here.
                        BackendEvent::SftpTransferFinished { .. } => {}
                        BackendEvent::SftpTransferProgress { .. } => {}
                    },
                    Err(_e) => {
                        #[cfg(debug_assertions)]
                        tracing::warn!("session: recv error: {:?}", _e);
                        let _ = wakeup_tx.try_broadcast(());
                        break;
                    }
                }
            }
        })
        .detach();
    }

    pub fn with_term<R>(&self, f: impl FnOnce(&Term<EventProxy>) -> R) -> R {
        let term = self.term.lock();
        f(&*term)
    }

    /// Mutable access — needed to read & reset alacritty damage.
    pub fn with_term_mut<R>(&self, f: impl FnOnce(&mut Term<EventProxy>) -> R) -> R {
        let mut term = self.term.lock();
        f(&mut *term)
    }

    /// Non-blocking mutable access. Returns `None` if the reader thread currently
    /// holds the lock — the caller should reuse the previous frame's snapshot
    /// instead of stalling the render thread.
    pub fn try_with_term_mut<R>(&self, f: impl FnOnce(&mut Term<EventProxy>) -> R) -> Option<R> {
        self.term.try_lock_unfair().map(|mut t| f(&mut *t))
    }

    pub fn feed_escape(&self, data: &[u8]) {
        let mut term = self.term.lock();
        let mut parser = Processor::<StdSyncHandler>::new();
        parser.advance(&mut *term, data);
    }

    pub fn write(&self, data: &[u8]) {
        self.capture_command(data);
        self.backend.write(data);
    }

    /// Write raw bytes to the backend **without** capturing them into the
    /// command history. Used by the History panel's "paste" action so
    /// inserting a historical command into the input line doesn't re-record
    /// it as a new entry.
    pub fn write_raw(&self, data: &[u8]) {
        self.backend.write(data);
    }

    /// Snapshot of the command history, most-recent-first. Cheap clone —
    /// the caller typically hands this to a UI panel each render.
    pub fn command_history(&self) -> Vec<String> {
        self.command_history.lock().iter().cloned().collect()
    }

    /// Direct mutable access to the underlying history deque. Used by the
    /// UI layer to pre-seed the in-memory buffer with persisted history on
    /// session creation. Returns a guard; caller assigns the whole deque.
    pub fn command_history_deque(&self) -> parking_lot::MutexGuard<'_, VecDeque<String>> {
        self.command_history.lock()
    }

    /// Register a callback invoked whenever a new command is captured
    /// (submitted via Enter). The UI layer uses this to persist commands
    /// to the Store — `TerminalSession` itself has no storage dependency.
    /// Pass `None` to clear a previously-set callback.
    pub fn set_on_command(&self, cb: Option<Arc<dyn Fn(&str) + Send + Sync>>) {
        *self.on_command.lock() = cb;
    }

    /// Best-effort command capture from the raw input stream.
    ///
    /// Accumulates printable bytes into `line_buffer`, treats Backspace
    /// (DEL `0x7f` or BS `0x08`) as a one-char deletion, and submits the
    /// buffer as a history entry on CR/LF (`0x0d` / `0x0a`). Empty results
    /// and exact-duplicates of the most recent entry are skipped.
    ///
    /// This is intentionally simple — it doesn't parse ANSI escapes or
    /// track cursor movement, so commands edited with arrow keys / Ctrl-U
    /// may be captured with noise. For typical typed-and-Enter usage it's
    /// accurate enough, and the cost (a lock + small alloc) is negligible.
    fn capture_command(&self, data: &[u8]) {
        let mut history = self.command_history.lock();
        let mut buf = self.line_buffer.lock();
        for &b in data {
            match b {
                // CR or LF — submit the line.
                0x0d | 0x0a => {
                    let cmd = buf.trim().to_string();
                    buf.clear();
                    if cmd.is_empty() {
                        continue;
                    }
                    // LRU dedup: if the command already exists in
                    // history, remove it from its current position so it
                    // can be re-inserted at the front (most-recently-used).
                    // This mirrors the Store's `updated_at` promotion.
                    if let Some(pos) = history.iter().position(|c| c == &cmd) {
                        history.remove(pos);
                    }
                    if history.len() >= MAX_COMMAND_HISTORY {
                        history.pop_back();
                    }
                    history.push_front(cmd.clone());
                    // Notify the UI layer so it can persist the command.
                    // The callback is cloned out of the Mutex to avoid
                    // calling it while holding the lock.
                    let cb = self.on_command.lock().clone();
                    if let Some(cb) = cb {
                        cb(&cmd);
                    }
                }
                // Backspace / DEL — delete the last char.
                0x08 | 0x7f => {
                    buf.pop();
                }
                // Other control bytes — ignore (don't pollute the buffer).
                0x00..=0x1f | 0x7f..=0xff if b != 0x09 => {
                    // 0x09 (Tab) is technically control but we keep it so
                    // shell completion entries are captured as-typed.
                }
                // Printable ASCII.
                0x20..=0x7e => {
                    buf.push(b as char);
                }
                // High bytes (UTF-8 continuation / lead) — push raw so
                // non-ASCII commands aren't lost. We don't validate UTF-8
                // here; the buffer is only for display, not execution.
                _ => {
                    buf.push(b as char);
                }
            }
        }
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        {
            let mut term = self.term.lock();
            term.resize(TermSize::new(cols as usize, rows as usize));
        }
        self.backend.resize(cols, rows);
    }

    pub fn close(&self) {
        self.backend.close();
    }

    pub fn subscribe_wakeup(&self) -> BroadcastReceiver<()> {
        self.wakeup_tx.new_receiver()
    }

    pub fn subscribe_backend(&self) -> BroadcastReceiver<BackendEvent> {
        self.backend.subscribe()
    }

    pub fn monitor(&self) -> Option<&dyn CrabPortMonitor> {
        self.backend.as_monitor()
    }

    pub fn allow_sftp(&self) -> bool {
        self.backend.allow_sftp()
    }

    pub fn sftp_entries(&self) -> Option<std::sync::Arc<Vec<(String, bool)>>> {
        self.backend.sftp_entries()
    }

    pub fn sftp_cwd(&self) -> Option<std::sync::Arc<String>> {
        self.backend.sftp_cwd()
    }

    pub fn sftp_navigate(&self, path: &str) {
        self.backend.sftp_navigate(path)
    }

    /// Download a remote file via the implicit-gzip staged flow.
    /// Completion is reported via the backend's event stream as
    /// `BackendEvent::SftpTransferFinished`.
    pub fn sftp_download(&self, remote_path: &str, local_path: &str) {
        self.backend.sftp_download(remote_path, local_path);
    }

    /// Upload a local file via the implicit-gzip staged flow.
    /// Completion is reported via the backend's event stream as
    /// `BackendEvent::SftpTransferFinished`.
    pub fn sftp_upload(&self, local_path: &str, remote_path: &str) {
        self.backend.sftp_upload(local_path, remote_path);
    }

    /// Delete a remote file or directory.
    /// Completion is reported via the backend's event stream as
    /// `BackendEvent::SftpTransferFinished`.
    pub fn sftp_delete(&self, remote_path: &str) {
        self.backend.sftp_delete(remote_path);
    }

    pub fn scroll(&self, delta: i32) {
        let mut term = self.term.lock();
        use alacritty_terminal::grid::Scroll;
        term.scroll_display(Scroll::Delta(delta));
        let _ = self.wakeup_tx.try_broadcast(());
    }

    pub fn scroll_to_bottom(&self) {
        let mut term = self.term.lock();
        use alacritty_terminal::grid::Scroll;
        term.scroll_display(Scroll::Bottom);
        let _ = self.wakeup_tx.try_broadcast(());
    }
}
