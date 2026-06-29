use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_animation::{animation::TransitionExt, transition::general::EaseInOutCubic};
use parking_lot::Mutex;
use rust_i18n::t;
use tokio::sync::oneshot;

use crabport_ssh::backend::{HostKeyInfo, HostKeyVerifier};
use crabport_terminal::terminal::RemoteStatus;

use crate::components::button::Button;
use crate::views::terminal::color::*;

/// A single log entry shown on the connection overlay.
#[derive(Debug, Clone)]
pub struct ConnectionLogEntry {
    pub message: String,
    pub level: ConnectionLogLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionLogLevel {
    Info,
    Success,
    Warning,
    Error,
}

impl ConnectionLogLevel {
    pub fn color(&self) -> u32 {
        match self {
            Self::Info => TERM_FG,
            Self::Success => 0xa6e3a1, // TERM_GREEN
            Self::Warning => 0xf9e2af, // TERM_YELLOW
            Self::Error => 0xf38ba8,   // TERM_RED
        }
    }

    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Info => "  ",
            Self::Success => "✓ ",
            Self::Warning => "⚠ ",
            Self::Error => "✗ ",
        }
    }
}

/// Shared state for the connection overlay.
/// Stored inside `TerminalView` so both the wakeup listener and the render method
/// can update / read it.
pub struct ConnectionOverlayState {
    /// Collected log entries.
    pub logs: Vec<ConnectionLogEntry>,
    /// The last observed remote status.
    pub status: RemoteStatus,
    /// Whether the "connected" fade-out animation has started.
    pub fade_out_started: bool,
    /// Whether the overlay should be completely hidden after fade-out.
    pub hidden: bool,
    /// A pending host-key verification prompt. While `Some`, the overlay
    /// renders a confirmation dialog instead of the usual connecting /
    /// connected view. The `oneshot::Sender` resolves the verifier future
    /// that the SSH backend is awaiting in `check_server_key`.
    pub pending_host_key: Option<PendingHostKey>,
    /// Set to `true` whenever the overlay state changes from a non-gpui
    /// thread (e.g. the SSH backend pushing a host-key prompt). The
    /// `TerminalView` frame pump polls this and folds it into its own
    /// `needs_repaint` flag — that way we get a repaint without needing
    /// an `AsyncApp` (which is not `Send`) inside the verifier closure.
    pub dirty: Arc<AtomicBool>,
}

/// A pending host-key confirmation request from the SSH backend.
pub struct PendingHostKey {
    pub info: HostKeyInfo,
    /// `Some(true)` => trust & continue, `Some(false)` => abort.
    /// `None` (dropped sender) is treated as abort.
    pub responder: Option<oneshot::Sender<bool>>,
}

impl PendingHostKey {
    /// Resolve the prompt. Returns `true` if the backend will continue.
    pub fn resolve(&mut self, accept: bool) {
        if let Some(tx) = self.responder.take() {
            let _ = tx.send(accept);
        }
    }
}

impl ConnectionOverlayState {
    pub fn new() -> Self {
        Self {
            logs: Vec::new(),
            status: RemoteStatus::Connecting,
            fade_out_started: false,
            hidden: false,
            pending_host_key: None,
            dirty: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Push a log entry.
    pub fn log(&mut self, level: ConnectionLogLevel, message: impl Into<String>) {
        self.logs.push(ConnectionLogEntry {
            message: message.into(),
            level,
        });
    }

    /// Update the remote status and automatically push relevant log entries.
    pub fn update_status(&mut self, new_status: RemoteStatus, host: &str) {
        if new_status == self.status {
            return;
        }
        match new_status {
            RemoteStatus::Connecting => {
                // Reset overlay state for a new connection attempt
                self.fade_out_started = false;
                self.hidden = false;
                self.logs.clear();
                // Abort any pending host-key prompt from the previous attempt.
                if let Some(mut p) = self.pending_host_key.take() {
                    p.resolve(false);
                }
                self.log(
                    ConnectionLogLevel::Info,
                    format!("Connecting to {}...", host),
                );
            }
            RemoteStatus::Connected => {
                self.log(
                    ConnectionLogLevel::Success,
                    format!("Connected to {}", host),
                );
                self.fade_out_started = true;
            }
            RemoteStatus::Disconnected => {
                self.log(
                    ConnectionLogLevel::Error,
                    format!("Disconnected from {}", host),
                );
            }
            RemoteStatus::Local => {}
        }
        self.status = new_status;
    }

    /// Returns `true` when the overlay should be rendered.
    pub fn is_visible(&self) -> bool {
        !self.hidden || self.pending_host_key.is_some()
    }

    /// Returns `true` when the fade-out overlay (successful connection) should be rendered.
    pub fn is_fading_out(&self) -> bool {
        self.fade_out_started && !self.hidden
    }

    /// Mark the overlay as fully hidden after fade-out completes.
    pub fn mark_hidden(&mut self) {
        self.hidden = true;
    }
}

/// Shared wrapper so background tasks and the render method can both access it.
pub type SharedOverlayState = Arc<Mutex<ConnectionOverlayState>>;

/// Build the [`HostKeyVerifier`] closure that the SSH backend calls inside
/// `check_server_key`.
///
/// The closure stashes a [`PendingHostKey`] (containing a `oneshot::Sender`)
/// into the shared overlay state so the render method can show a confirm
/// dialog, then awaits the matching receiver.
///
/// # Repaint
///
/// The verifier runs on the SSH backend's tokio runtime, but the overlay is
/// rendered by the gpui foreground. We can't capture an `AsyncApp` here (it
/// is not `Send`), so instead we set the overlay's `dirty` flag — the
/// `TerminalView` frame pump polls it (alongside its own `needs_repaint`)
/// and triggers a `cx.notify()`, which makes the dialog appear promptly.
pub fn make_host_key_verifier(overlay: SharedOverlayState) -> HostKeyVerifier {
    Arc::new(move |info: HostKeyInfo| {
        let (tx, rx) = oneshot::channel::<bool>();
        let dirty;
        {
            let mut ov = overlay.lock();
            // Abort any previous (shouldn't happen) prompt and install the new one.
            if let Some(mut p) = ov.pending_host_key.take() {
                p.resolve(false);
            }
            ov.pending_host_key = Some(PendingHostKey {
                info,
                responder: Some(tx),
            });
            dirty = ov.dirty.clone();
        }
        // Signal the terminal frame pump to repaint so the dialog shows up.
        dirty.store(true, Ordering::Release);
        Box::pin(async move { rx.await.unwrap_or(false) })
    })
}

/// Build an [`AlertState`] for the host-key confirmation prompt backed by the
/// given pending prompt.
///
/// `on_confirm` / `on_cancel` receive `(&mut Window, &mut App)` (matching
/// [`crate::components::dialog::AlertState`]) and are expected to resolve the
/// pending prompt via [`PendingHostKey::resolve`] — typically the caller
/// wires them to `this.overlay.lock().pending_host_key.take()`.
pub fn host_key_alert_state(
    info: &HostKeyInfo,
    on_confirm: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_cancel: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
) -> crate::components::dialog::AlertState {
    use crate::components::dialog::{AlertSeverity, AlertState};

    let host_port = if info.port == 22 {
        info.host.clone()
    } else {
        format!("{}:{}", info.host, info.port)
    };

    AlertState {
        open: true,
        severity: AlertSeverity::Warning,
        title: t!("terminal.host_key_unknown").into(),
        description: Some(
            t!("terminal.host_key_prompt", host = host_port.as_str())
                .to_string()
                .into(),
        ),
        details: vec![
            (
                t!("terminal.host_key_algo").to_string().into(),
                info.algo.clone().into(),
            ),
            (
                t!("terminal.host_key_fingerprint").to_string().into(),
                info.fingerprint.clone().into(),
            ),
        ],
        confirm_label: t!("terminal.host_key_accept").to_string().into(),
        cancel_label: t!("terminal.host_key_cancel").to_string().into(),
        on_confirm,
        on_cancel,
    }
}

// ---- Connection Overlay Rendering ----

pub(crate) fn render_connection_overlay(
    overlay_visible: bool,
    is_fading_out: bool,
    status: RemoteStatus,
    logs: &[ConnectionLogEntry],
    count: u64,
    on_reconnect: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App)>>,
) -> AnyElement {
    if !overlay_visible {
        return div().into_any_element();
    }

    div()
        .id(ElementId::Name(
            format!("connection-overlay-{}", count).into(),
        ))
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .flex()
        .cursor_default()
        .items_center()
        .justify_center()
        .bg(rgb(TERM_BG))
        .opacity(1.0)
        .with_transition(("connection-overlay-opacity", count))
        .transition_when(
            is_fading_out,
            std::time::Duration::from_millis(500),
            EaseInOutCubic,
            |el| el.opacity(0.0),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_6()
                .max_w(px(400.0))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_3()
                        .child(match status {
                            RemoteStatus::Connecting => render_spinner(),
                            RemoteStatus::Disconnected => {
                                div().size(px(12.0)).rounded_full().bg(rgb(0xf38ba8))
                            }
                            _ => div().size(px(12.0)).rounded_full().bg(rgb(0xa6e3a1)),
                        })
                        .child(
                            div()
                                .text_sm()
                                .text_color(rgb(TERM_FG))
                                .child(match status {
                                    RemoteStatus::Connecting => "Connecting…",
                                    RemoteStatus::Disconnected => "Connection failed",
                                    _ => "Connected",
                                }),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .w_full()
                        .children(logs.iter().map(|entry| {
                            let prefix = entry.level.prefix();
                            let color = entry.level.color();
                            let text = format!("{}{}", prefix, entry.message);
                            div()
                                .flex()
                                .flex_row()
                                .items_start()
                                .text_sm()
                                .text_color(rgb(color))
                                .child(text)
                        })),
                )
                .when(status == RemoteStatus::Disconnected, |el| {
                    let mut btn =
                        Button::new(ElementId::Name(format!("reconnect-btn-{}", count).into()))
                            .h_10()
                            .centered(true)
                            .child(t!("terminal.reconnect").to_string());
                    if let Some(cb) = on_reconnect {
                        btn = btn.on_click(move |e, w, a| cb(e, w, a));
                    }
                    el.child(btn)
                }),
        )
        .into_any_element()
}

fn render_spinner() -> Div {
    div()
        .size(px(12.0))
        .rounded_full()
        .border_2()
        .border_color(rgb(TERM_FG))
}
