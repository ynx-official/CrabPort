//! Snippets panel — a side panel listing reusable command snippets.
//!
//! This is the panel-tab sibling of [`super::sftp::SftpPanel`]: it renders
//! inside the right-hand panel strip's "Snippets" tab (see
//! `crabport-ui/src/layouts/panel.rs`). The sidebar's `Snippets` item is a
//! separate top-level view (`crate::views::snippets`) — the two are
//! intentionally distinct: the sidebar entry is the full-page management
//! surface, this panel is the quick-insert overlay shown next to the
//! terminal.
//!
//! Currently a skeleton: `set_state` accepts snippet data but the render
//! only shows an empty placeholder. Real listing / insert-on-click /
//! search will follow once the snippet store backend lands.

use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::scroll::ScrollableElement as _;
use rust_i18n::t;

use crate::color::*;

/// A single reusable command snippet.
///
/// `command` is the literal text to paste/insert into the terminal. `name`
/// is the human label; `description` is optional help text shown muted
/// under the name.
#[derive(Clone, Debug)]
pub struct Snippet {
    pub name: String,
    pub command: String,
    pub description: Option<String>,
}

/// Snippets panel view.
///
/// Holds the current snippet list (pushed in via `set_state` before each
/// render) and an optional insert callback. Like [`SftpPanel`], it's an
/// `Entity` so it can be shared across renders and wrapped in a
/// `TabPane`.
pub struct SnippetsPanel {
    /// Current snippet list. Empty until `set_state` is called.
    snippets: Arc<Vec<Snippet>>,
    /// Insert callback — invoked with the snippet's command text when the
    /// user clicks a row. Injected from `content.rs` so this view stays
    /// agnostic of the terminal/backend wiring (mirrors `SftpPanel`'s
    /// `on_navigate`).
    on_insert: Option<std::rc::Rc<dyn Fn(String, &mut App)>>,
}

impl SnippetsPanel {
    pub fn new() -> Self {
        Self {
            snippets: Arc::new(Vec::new()),
            on_insert: None,
        }
    }

    /// Update the snippet list + insert callback from the active context.
    /// Called by the content layout each render (same pattern as
    /// `SftpPanel::set_state`).
    #[allow(dead_code)]
    pub fn set_state(
        &mut self,
        snippets: Arc<Vec<Snippet>>,
        on_insert: Option<std::rc::Rc<dyn Fn(String, &mut App)>>,
    ) {
        self.snippets = snippets;
        self.on_insert = on_insert;
    }
}

impl Default for SnippetsPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for SnippetsPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let snippets = self.snippets.clone();
        let on_insert = self.on_insert.clone();

        div()
            .h_full()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .pt_1()
            .px_1()
            .when_some((!snippets.is_empty()).then_some(()), |el, _| {
                el.child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .overflow_y_scrollbar()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .children(snippets.iter().enumerate().map(|(i, s)| {
                            let on_insert = on_insert.clone();
                            let cmd = s.command.clone();
                            div()
                                .id(ElementId::Name(format!("snippet-{i}").into()))
                                .flex()
                                .flex_col()
                                .gap_0p5()
                                .px_2()
                                .py_1()
                                .rounded_md()
                                .text_xs()
                                .text_color(rgb(TEXT_PRIMARY))
                                .hover(|s| s.bg(rgb(SURFACE_HOVER)))
                                .when_some(on_insert, |el, cb| {
                                    el.on_click(move |_e, _w, cx| {
                                        cb(cmd.clone(), cx);
                                    })
                                })
                                .child(div().child(s.name.clone()))
                                .child(div().text_color(rgb(TEXT_MUTED)).child(s.command.clone()))
                                .when_some(s.description.clone(), |el, desc| {
                                    el.child(div().text_color(rgb(TEXT_MUTED)).child(desc))
                                })
                        })),
                )
            })
            .when(snippets.is_empty(), |el| {
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
                                .child(t!("sidebar.snippets").to_string()),
                        ),
                )
            })
    }
}
