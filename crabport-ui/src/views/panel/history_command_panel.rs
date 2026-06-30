//! History-command panel — a side panel listing recently-run terminal
//! commands.
//!
//! Sibling of [`super::sftp::SftpPanel`] / [`super::snippets_panel::SnippetsPanel`]:
//! renders inside the right-hand panel strip's "History" tab (see
//! `crabport-ui/src/layouts/panel.rs`). The sidebar's `History` item is a
//! separate top-level view — the two are intentionally distinct (sidebar =
//! full-page history browser, this panel = quick-rerun overlay next to the
//! terminal).
//!
//! Currently a skeleton: `set_state` accepts history data but the render
//! only shows an empty placeholder. Real listing / rerun-on-click / search
//! will follow once the command-history store backend lands.

use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::scroll::ScrollableElement as _;
use rust_i18n::t;

use crate::color::*;

/// A single previously-run terminal command entry.
///
/// `command` is the literal text that was executed. `timestamp` is an
/// optional display string (e.g. "2 min ago") rendered muted under the
/// command — kept as a pre-formatted string so this view doesn't need to
/// know about time formatting.
#[derive(Clone, Debug)]
pub struct HistoryCommand {
    pub command: String,
    pub timestamp: Option<String>,
}

/// History-command panel view.
///
/// Holds the current history list (pushed in via `set_state` before each
/// render) and an optional rerun callback. Like [`SftpPanel`], it's an
/// `Entity` so it can be shared across renders and wrapped in a
/// `TabPane`.
pub struct HistoryCommandPanel {
    /// Current history list, most-recent-first. Empty until `set_state`
    /// is called.
    history: Arc<Vec<HistoryCommand>>,
    /// Rerun callback — invoked with the command text when the user clicks
    /// a row. Injected from `content.rs` so this view stays agnostic of
    /// the terminal/backend wiring (mirrors `SftpPanel`'s `on_navigate`).
    on_rerun: Option<std::rc::Rc<dyn Fn(String, &mut App)>>,
}

impl HistoryCommandPanel {
    pub fn new() -> Self {
        Self {
            history: Arc::new(Vec::new()),
            on_rerun: None,
        }
    }

    /// Update the history list + rerun callback from the active context.
    /// Called by the content layout each render (same pattern as
    /// `SftpPanel::set_state`).
    #[allow(dead_code)]
    pub fn set_state(
        &mut self,
        history: Arc<Vec<HistoryCommand>>,
        on_rerun: Option<std::rc::Rc<dyn Fn(String, &mut App)>>,
    ) {
        self.history = history;
        self.on_rerun = on_rerun;
    }
}

impl Default for HistoryCommandPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for HistoryCommandPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let history = self.history.clone();
        let on_rerun = self.on_rerun.clone();

        div()
            .h_full()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .pt_1()
            .px_1()
            .when_some((!history.is_empty()).then_some(()), |el, _| {
                el.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scrollbar()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .children(history.iter().enumerate().map(|(i, h)| {
                            let on_rerun = on_rerun.clone();
                            let cmd = h.command.clone();
                            div()
                                .id(ElementId::Name(format!("history-cmd-{i}").into()))
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(SURFACE_HOVER)))
                                .when_some(on_rerun, |el, cb| {
                                    el.on_click(move |_e, _w, cx| {
                                        cb(cmd.clone(), cx);
                                    })
                                })
                                .child(div().child(h.command.clone()))
                                .when_some(h.timestamp.clone(), |el, ts| {
                                    el.child(div().text_color(rgb(TEXT_MUTED)).child(ts))
                                })
                        })),
                )
            })
            .when(history.is_empty(), |el| {
                el.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scrollbar()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .text_color(rgb(TEXT_MUTED))
                                .text_sm()
                                .child(t!("sidebar.history").to_string()),
                        ),
                )
            })
    }
}
