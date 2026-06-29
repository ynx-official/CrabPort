use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_animation::{animation::TransitionExt, transition::general::EaseInOutCubic};
use parking_lot::Mutex;
use rust_i18n::t;

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
}

impl ConnectionOverlayState {
    pub fn new() -> Self {
        Self {
            logs: Vec::new(),
            status: RemoteStatus::Connecting,
            fade_out_started: false,
            hidden: false,
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
        !self.hidden
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
