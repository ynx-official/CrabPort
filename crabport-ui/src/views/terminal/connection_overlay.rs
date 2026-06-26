use std::sync::Arc;

use parking_lot::Mutex;

use crabport_terminal::terminal::RemoteStatus;

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
