//! Snippets management view — the full-page sidebar view for managing saved
//! command snippets.
//!
//! Listed when the sidebar's "Snippets" item is active. Reads/writes the
//! same `snippets` Store table as the panel-tab Snippets view
//! ([`crate::views::panel::snippets_panel`]); the two are intentionally
//! distinct — the panel is a quick-run overlay next to the terminal, this
//! is the management surface (edit / delete).
//!
//! Layout mirrors [`crate::views::hosts::HostsView`]:
//!
//! ```text
//! ┌─────────────────────────────────┐
//! │ Snippets              [+ New]   │
//! │ ─────────────────────────────── │
//! │ snippet_name                    │  ← right-click: Edit / Delete
//! │   command text (muted)          │
//! │ ...                             │
//! └─────────────────────────────────┘
//! ```

use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_animation::{animation::TransitionExt, transition::general::Linear};
use gpui_component::scroll::ScrollableElement as _;
use rust_i18n::t;
use std::time::Duration;

use crate::app::CrabportApp;
use crate::color::*;
use crate::components::button::Button;
use crate::components::context_menu::{ContextMenuController, ContextMenuItem, ContextMenuState};
use crate::components::dialog::{AlertController, AlertSeverity, AlertState};

// ---------------------------------------------------------------------------
// Submodules & re-exports
// ---------------------------------------------------------------------------

pub mod form;
pub use form::{SnippetFormOutput, SnippetFormState, SnippetFormView};

/// A snippet row shown in the management list.
#[derive(Clone)]
pub struct SnippetRow {
    pub id: i64,
    pub name: String,
    pub command: String,
}

/// Snippets management view.
pub struct SnippetsView {
    /// The snippet row currently being hovered, if any.
    hovered_snippet_id: Option<i64>,
    /// The snippet row that triggered the currently-open context menu.
    context_menu_snippet_id: Option<i64>,
    /// Snippet list, most-recently-created first. Reloaded from the Store
    /// before each render via `set_state`.
    snippets: Vec<SnippetRow>,
    /// Owning `CrabportApp` entity. Used to construct `SnippetFormView`
    /// (which needs an `Entity<CrabportApp>` to drive the save callback).
    app: Entity<CrabportApp>,
    /// Global context menu host (right-click Edit / Delete).
    context_menu: Option<Entity<ContextMenuController>>,
    /// Global alert dialog host (delete confirmation).
    alert_controller: Option<Entity<AlertController>>,
    /// "New" button callback — routes to `CrabportApp::open_snippet_form_for_create`.
    on_new: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    /// "Edit" context-menu callback — routes to
    /// `CrabportApp::open_snippet_form_for_edit`. Receives the snippet id.
    on_edit: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
    /// Snippet form dialog state, pushed in before each render. When
    /// `Some`, `SnippetFormView` is rendered on top of the list.
    form_state: Option<SnippetFormState>,
}

impl SnippetsView {
    pub fn new(app: Entity<CrabportApp>) -> Self {
        Self {
            hovered_snippet_id: None,
            context_menu_snippet_id: None,
            snippets: Vec::new(),
            app,
            context_menu: None,
            alert_controller: None,
            on_new: None,
            on_edit: None,
            form_state: None,
        }
    }

    /// Push the latest external state into the view before render.
    /// `snippets` is re-read from the Store by the caller (`render_content`).
    #[allow(clippy::too_many_arguments)]
    pub fn set_state(
        &mut self,
        snippets: Vec<SnippetRow>,
        context_menu: Entity<ContextMenuController>,
        alert_controller: Entity<AlertController>,
        on_new: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
        on_edit: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
        form_state: Option<SnippetFormState>,
        cx: &mut Context<Self>,
    ) {
        // Clear stale hover if the snippet disappeared.
        if let Some(id) = self.hovered_snippet_id
            && !snippets.iter().any(|s| s.id == id)
        {
            self.hovered_snippet_id = None;
        }
        self.snippets = snippets;
        self.context_menu = Some(context_menu);
        self.alert_controller = Some(alert_controller);
        self.on_new = on_new;
        self.on_edit = on_edit;
        self.form_state = form_state;
        let _ = cx;
    }

    /// Delete a snippet by id (after confirmation).
    fn delete_snippet(&mut self, id: i64, cx: &mut Context<Self>) {
        let store = crate::app_state::AppState::store(cx);
        let _ = store.lock().remove_snippet(id);
        cx.notify();
    }
}

impl Render for SnippetsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let snippets = self.snippets.clone();
        let context_menu = self.context_menu.clone();
        let alert_controller = self.alert_controller.clone();
        let hovered_snippet_id = self.hovered_snippet_id;

        // Clear stale context-menu highlight if the menu closed.
        let menu_active = self
            .context_menu
            .as_ref()
            .map(|cm| cm.read_with(_cx, |c, _| c.is_active()))
            .unwrap_or(false);
        if !menu_active {
            self.context_menu_snippet_id = None;
        }
        let context_menu_snippet_id = self.context_menu_snippet_id;

        let on_new = self.on_new.clone();
        let on_edit = self.on_edit.clone();
        let form_state = self.form_state.clone();
        let app = self.app.clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .relative()
            // --- Header: title + New button ---
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .justify_between()
                    .px_4()
                    .pt_4()
                    .pb_2()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(rgb(text_primary()))
                            .child(t!("sidebar.snippets").to_string()),
                    )
                    .child(
                        Button::new("snippets-new-btn")
                            .primary()
                            .icon("icons/plus.svg")
                            .w_auto()
                            .px_2()
                            .child(t!("snippets.new_button").to_string())
                            .on_click(move |_e, w, cx| {
                                if let Some(ref cb) = on_new {
                                    cb(w, cx);
                                }
                            }),
                    ),
            )
            // --- Separator ---
            .child(div().h_px().bg(rgb(border())).mx_4())
            // --- Snippets list (or empty state) ---
            .child(
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .px_4()
                    .py_2()
                    .when_else(
                        snippets.is_empty(),
                        |el| {
                            el.flex().items_center().justify_center().child(
                                div()
                                    .text_color(rgb(text_muted()))
                                    .text_sm()
                                    .child(t!("snippets.empty").to_string()),
                            )
                        },
                        |el| {
                            el.flex()
                                .flex_col()
                                .gap_1()
                                .children(snippets.iter().map(|s| {
                                    let snippet = s.clone();
                                    let context_menu = context_menu.clone();
                                    let alert_controller = alert_controller.clone();
                                    let is_hovered = hovered_snippet_id == Some(s.id);
                                    let force_highlight = context_menu_snippet_id == Some(s.id);
                                    let entity = _cx.entity().downgrade();
                                    let on_edit = on_edit.clone();

                                    snippet_row(
                                        &snippet,
                                        is_hovered,
                                        force_highlight,
                                        entity,
                                        context_menu,
                                        alert_controller,
                                        on_edit,
                                    )
                                    .into_any_element()
                                }))
                        },
                    ),
            )
            // --- Snippet form overlay (create/edit) ---
            .when_some(form_state, move |el, state| {
                el.child(SnippetFormView::new(&state, app))
            })
    }
}

// ---------------------------------------------------------------------------
// Snippet row
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn snippet_row(
    snippet: &SnippetRow,
    is_hovered: bool,
    force_highlight: bool,
    entity: WeakEntity<SnippetsView>,
    context_menu: Option<Entity<ContextMenuController>>,
    alert_controller: Option<Entity<AlertController>>,
    on_edit: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
) -> impl IntoElement {
    let row_id = ElementId::Name(format!("snippet-row-{}", snippet.id).into());

    let snippet_id = snippet.id;
    let snippet_name = snippet.name.clone();
    let snippet_command = snippet.command.clone();
    let is_highlighted = is_hovered || force_highlight;

    div()
        .id(row_id.clone())
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(bg_base()))
        // Right-click context menu: "Edit" + "Delete".
        .on_mouse_down(MouseButton::Right, {
            let entity = entity.clone();
            move |event, _w, cx| {
                let Some(ref cm) = context_menu else {
                    return;
                };
                let _ = entity.update(cx, |view, cx| {
                    view.context_menu_snippet_id = Some(snippet_id);
                    cx.notify();
                });
                let pos = event.position;
                let entity_for_delete = entity.clone();
                let alert_controller = alert_controller.clone();
                let snippet_name = snippet_name.clone();
                let on_edit = on_edit.clone();
                cm.update(cx, |c, cx| {
                    c.show(
                        ContextMenuState {
                            position: pos,
                            items: vec![
                                ContextMenuItem::new(t!("snippets.edit").to_string(), {
                                    let on_edit = on_edit.clone();
                                    move |w, cx| {
                                        if let Some(ref cb) = on_edit {
                                            cb(snippet_id, w, cx);
                                        }
                                    }
                                }),
                                ContextMenuItem::new(t!("snippets.delete").to_string(), {
                                    let entity = entity_for_delete.clone();
                                    let alert_controller = alert_controller.clone();
                                    let snippet_name = snippet_name.clone();
                                    move |_w, cx| {
                                        let Some(ref ac) = alert_controller else {
                                            return;
                                        };
                                        let entity = entity.clone();
                                        ac.update(cx, |c, cx| {
                                            c.show(
                                                AlertState {
                                                    severity: AlertSeverity::Danger,
                                                    title: t!("snippets.delete_title")
                                                        .to_string()
                                                        .into(),
                                                    description: Some(
                                                        t!(
                                                            "snippets.delete_prompt",
                                                            name = snippet_name.as_str()
                                                        )
                                                        .to_string()
                                                        .into(),
                                                    ),
                                                    confirm_label: t!("snippets.delete_confirm")
                                                        .to_string()
                                                        .into(),
                                                    cancel_label: t!("terminal.host_key_cancel")
                                                        .to_string()
                                                        .into(),
                                                    on_confirm: Some(Rc::new(move |_w, cx| {
                                                        let _ = entity.update(cx, |view, cx| {
                                                            view.delete_snippet(snippet_id, cx);
                                                        });
                                                    })),
                                                    ..AlertState::default()
                                                },
                                                cx,
                                            );
                                        });
                                    }
                                })
                                .danger(true),
                            ],
                            ..ContextMenuState::default()
                        },
                        cx,
                    );
                });
            }
        })
        // Track hover of the whole row.
        .with_transition(row_id)
        .on_hover(move |hovered, _w, cx| {
            let _ = entity.update(cx, |view, cx| {
                if *hovered {
                    view.hovered_snippet_id = Some(snippet_id);
                } else if view.hovered_snippet_id == Some(snippet_id) {
                    view.hovered_snippet_id = None;
                }
                cx.notify();
            });
        })
        .transition_when_else(
            is_highlighted,
            Duration::from_millis(120),
            Linear,
            |el| el.bg(rgb(surface_active())),
            |el| el.bg(rgb(bg_base())),
        )
        // Snippet info (name + command)
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .flex_1()
                .child(div().text_sm().text_color(rgb(text_primary())).child(
                    if snippet.name.is_empty() {
                        snippet_command.clone()
                    } else {
                        snippet.name.clone()
                    },
                ))
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(text_muted()))
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(snippet_command.clone()),
                ),
        )
}
