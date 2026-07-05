//! Tunnels management view — the full-page sidebar view for managing saved
//! SSH port-forwarding tunnels (Local / Remote / Dynamic).
//!
//! Mirrors [`crate::views::hosts::HostsView`] in structure: a header with a
//! "New" button, a scrollable list of rows with hover-fade action buttons,
//! and a right-click context menu (Start/Stop, Edit, Delete) plus an alert
//! confirmation dialog for delete.
//!
//! The view is stateless beyond hover + the "menu-triggering row" highlight.
//! External state (tunnel list, host list, callbacks, global controllers) is
//! pushed in via [`TunnelsView::set_state`] immediately before each render by
//! the parent (`render_content`).

/// Tunnel create/edit form dialog (lives in `form.rs`).
pub mod form;
/// Runtime state + registry for tunnels (lives in `state.rs`).
pub mod state;

// Re-export the commonly-used types so callers can reach them via
// `crate::views::tunnels::TunnelRegistry` / `TunnelView` / `TunnelFormState`
// etc. without an extra `state::` / `form::`.
pub use form::{TunnelFormOutput, TunnelFormState, TunnelFormView};
pub use state::{TunnelRegistry, TunnelView};

use std::rc::Rc;
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_animation::{animation::TransitionExt, transition::general::Linear};
use gpui_component::scroll::ScrollableElement as _;
use rust_i18n::t;

use crate::app::CrabportApp;
use crate::color::*;
use crate::components::button::Button;
use crate::components::context_menu::{ContextMenuController, ContextMenuItem, ContextMenuState};
use crate::components::dialog::{AlertController, AlertSeverity, AlertState};
use crate::views::hosts::ConnectionHost;

use crabport_core::credential::TunnelKind;

/// Color accents for the kind badge (subtle tint, not the full primary blue).
const KIND_LOCAL_COLOR: u32 = 0x89b4fa; // blue-ish
const KIND_REMOTE_COLOR: u32 = 0xf9c2ff; // mauve-ish
const KIND_DYNAMIC_COLOR: u32 = 0xf9e2af; // yellow-ish
const STATUS_RUNNING_COLOR: u32 = 0xa6e3a1; // green
const STATUS_STOPPED_COLOR: u32 = 0x585b70; // muted

/// Tunnels sidebar view. Holds its own hover state so the action buttons can
/// fade in with easing when the row is hovered — without polluting
/// `CrabportApp` state.
pub struct TunnelsView {
    /// The tunnel row currently being hovered, if any.
    hovered_tunnel_id: Option<i64>,
    /// The tunnel row that triggered the currently-open context menu, if any.
    /// While set, that row stays highlighted in the hover color even though
    /// the mouse has moved to the overlay.
    context_menu_tunnel_id: Option<i64>,
    // External data pushed in before each render.
    tunnels: Vec<TunnelView>,
    hosts: Vec<ConnectionHost>,
    /// Held for the context-menu/alert wiring (mirrors `HostsView`). Not yet
    /// read inside render — kept so future versions can reach the app entity
    /// without changing the public API.
    #[allow(dead_code)]
    app: Entity<CrabportApp>,
    // Global context menu host, used for the right-click menu on each row.
    context_menu: Option<Entity<ContextMenuController>>,
    // Global alert dialog host, used for the delete-confirmation prompt.
    alert_controller: Option<Entity<AlertController>>,
    // Callbacks
    on_new: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_start: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
    on_stop: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
    on_edit: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
    on_remove: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
    // The tunnel form dialog state, pushed in before each render. When
    // `Some` and `is_open()`, the view renders the `TunnelFormView` overlay
    // on top of the list — mirroring how `HostsView` renders
    // `ConnectionFormView`.
    form_state: Option<TunnelFormState>,
}

impl TunnelsView {
    pub fn new(app: Entity<CrabportApp>) -> Self {
        Self {
            hovered_tunnel_id: None,
            context_menu_tunnel_id: None,
            tunnels: Vec::new(),
            hosts: Vec::new(),
            app,
            context_menu: None,
            alert_controller: None,
            on_new: None,
            on_start: None,
            on_stop: None,
            on_edit: None,
            on_remove: None,
            form_state: None,
        }
    }

    /// Push the latest external state into the view before render.
    pub fn set_state(
        &mut self,
        tunnels: Vec<TunnelView>,
        hosts: Vec<ConnectionHost>,
        on_new: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
        on_start: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
        on_stop: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
        on_edit: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
        on_remove: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
        context_menu: Entity<ContextMenuController>,
        alert_controller: Entity<AlertController>,
        form_state: Option<TunnelFormState>,
        cx: &mut Context<Self>,
    ) {
        // Clear stale hover if the tunnel disappeared.
        if let Some(id) = self.hovered_tunnel_id
            && !tunnels.iter().any(|t| t.id == id)
        {
            self.hovered_tunnel_id = None;
        }
        self.tunnels = tunnels;
        self.hosts = hosts;
        self.on_new = on_new;
        self.on_start = on_start;
        self.on_stop = on_stop;
        self.on_edit = on_edit;
        self.on_remove = on_remove;
        self.context_menu = Some(context_menu);
        self.alert_controller = Some(alert_controller);
        self.form_state = form_state;
        // Note: do NOT call cx.notify() here — set_state is invoked every
        // render from render_content, so notifying would cause an infinite
        // loop. The TunnelsView re-renders naturally because its parent
        // (CrabportApp) re-renders.
        let _ = cx;
    }
}

impl Render for TunnelsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let tunnels = self.tunnels.clone();
        let hosts = self.hosts.clone();
        let on_new = self.on_new.clone();
        let on_start = self.on_start.clone();
        let on_stop = self.on_stop.clone();
        let on_edit = self.on_edit.clone();
        let on_remove = self.on_remove.clone();
        let context_menu = self.context_menu.clone();
        let alert_controller = self.alert_controller.clone();
        let hovered_tunnel_id = self.hovered_tunnel_id;
        let form_state = self.form_state.clone();
        let app = self.app.clone();
        let form_hosts = self.hosts.clone();

        // If the global context menu is no longer active, clear the
        // "menu-triggering row" highlight.
        let menu_active = self
            .context_menu
            .as_ref()
            .map(|cm| cm.read_with(_cx, |c, _| c.is_active()))
            .unwrap_or(false);
        if !menu_active {
            self.context_menu_tunnel_id = None;
        }
        let context_menu_tunnel_id = self.context_menu_tunnel_id;

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
                            .text_color(rgb(TEXT_PRIMARY))
                            .child(t!("sidebar.tunnels").to_string()),
                    )
                    .child(
                        Button::new("tunnels-new-btn")
                            .primary()
                            .icon("icons/plus.svg")
                            .w_auto()
                            .px_2()
                            .child(t!("tunnels.new_button").to_string())
                            .on_click(move |_e, w, cx| {
                                if let Some(ref cb) = on_new {
                                    cb(w, cx);
                                }
                            }),
                    ),
            )
            // --- Separator ---
            .child(div().h_px().bg(rgb(BORDER)).mx_4())
            // --- Tunnels list (or empty state) ---
            .child(
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .px_4()
                    .py_2()
                    .when_else(
                        tunnels.is_empty(),
                        |el| {
                            el.flex().items_center().justify_center().child(
                                div()
                                    .text_color(rgb(TEXT_MUTED))
                                    .text_sm()
                                    .child(t!("tunnels.empty").to_string()),
                            )
                        },
                        |el| {
                            el.flex()
                                .flex_col()
                                .gap_1()
                                .children(tunnels.iter().map(|t| {
                                    let tunnel = t.clone();
                                    let host_name = hosts
                                        .iter()
                                        .find(|h| h.id == tunnel.host_id)
                                        .map(|h| h.name.clone())
                                        .unwrap_or_else(|| "?".to_string());
                                    let on_start = on_start.clone();
                                    let on_stop = on_stop.clone();
                                    let on_edit = on_edit.clone();
                                    let on_remove = on_remove.clone();
                                    let context_menu = context_menu.clone();
                                    let alert_controller = alert_controller.clone();
                                    let is_hovered = hovered_tunnel_id == Some(t.id);
                                    let force_highlight = context_menu_tunnel_id == Some(t.id);
                                    let entity = _cx.entity().downgrade();

                                    tunnel_row(
                                        &tunnel,
                                        &host_name,
                                        is_hovered,
                                        force_highlight,
                                        entity,
                                        context_menu,
                                        alert_controller,
                                        move |w, cx| {
                                            if let Some(ref cb) = on_start {
                                                cb(tunnel.id, w, cx);
                                            }
                                        },
                                        move |w, cx| {
                                            if let Some(ref cb) = on_stop {
                                                cb(tunnel.id, w, cx);
                                            }
                                        },
                                        move |w, cx| {
                                            if let Some(ref cb) = on_edit {
                                                cb(tunnel.id, w, cx);
                                            }
                                        },
                                        move |w, cx| {
                                            if let Some(ref cb) = on_remove {
                                                cb(tunnel.id, w, cx);
                                            }
                                        },
                                    )
                                    .into_any_element()
                                }))
                        },
                    ),
            )
            // --- Tunnel form overlay (create/edit) ---
            // Mirrors `HostsView`'s rendering of `ConnectionFormView`: when
            // the form state is `Some`, render the overlay on top of the list.
            .when_some(form_state, move |el, state| {
                el.child(TunnelFormView::new(&state, app, form_hosts))
            })
    }
}

// ---------------------------------------------------------------------------
// Legacy free-function render path
// ---------------------------------------------------------------------------
//
// `render_content` in `layouts/content.rs` still calls this for the
// `SidebarItem::Tunnels` arm. It will be updated separately to construct a
// `TunnelsView` entity and push state via `set_state`. Until then, this shim
// renders a minimal placeholder (header + empty state) so the crate keeps
// compiling.

#[allow(dead_code)]
pub fn render_tunnels_view(on_new: impl Fn(&mut Window, &mut App) + 'static) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .relative()
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
                        .text_color(rgb(TEXT_PRIMARY))
                        .child(t!("sidebar.tunnels").to_string()),
                )
                .child(
                    Button::new("tunnels-new-btn")
                        .primary()
                        .icon("icons/plus.svg")
                        .w_auto()
                        .px_2()
                        .child(t!("tunnels.new_button").to_string())
                        .on_click(move |_e, w, cx| {
                            on_new(w, cx);
                        }),
                ),
        )
        .child(div().h_px().bg(rgb(BORDER)).mx_4())
        .child(
            div()
                .flex_1()
                .overflow_y_scrollbar()
                .px_4()
                .py_2()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(rgb(TEXT_MUTED))
                        .text_sm()
                        .child(t!("tunnels.empty").to_string()),
                ),
        )
}

// ---------------------------------------------------------------------------
// Tunnel row
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn tunnel_row(
    tunnel: &TunnelView,
    host_name: &str,
    is_hovered: bool,
    force_highlight: bool,
    entity: WeakEntity<TunnelsView>,
    context_menu: Option<Entity<ContextMenuController>>,
    alert_controller: Option<Entity<AlertController>>,
    on_start: impl Fn(&mut Window, &mut App) + 'static,
    on_stop: impl Fn(&mut Window, &mut App) + 'static,
    on_edit: impl Fn(&mut Window, &mut App) + 'static,
    on_remove: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let row_id = ElementId::Name(format!("tunnel-row-{}", tunnel.id).into());

    let tunnel_id = tunnel.id;
    let tunnel_running = tunnel.running;
    let tunnel_borrowed = tunnel.borrowed_tab_id.is_some();
    let is_highlighted = is_hovered || force_highlight;

    // Kind badge label + accent color + secondary address line.
    let (kind_letter, kind_label, kind_color) = match tunnel.kind {
        TunnelKind::Local => ("L", t!("tunnels.kind_local").to_string(), KIND_LOCAL_COLOR),
        TunnelKind::Remote => (
            "R",
            t!("tunnels.kind_remote").to_string(),
            KIND_REMOTE_COLOR,
        ),
        TunnelKind::Dynamic => (
            "D",
            t!("tunnels.kind_dynamic").to_string(),
            KIND_DYNAMIC_COLOR,
        ),
    };
    let bind_display = if tunnel.bind_addr.is_empty() {
        format!("*:{}", tunnel.bind_port)
    } else {
        format!("{}:{}", tunnel.bind_addr, tunnel.bind_port)
    };
    let address_line = match tunnel.kind {
        TunnelKind::Local | TunnelKind::Remote => format!(
            "{}  {} → {}:{}",
            kind_letter, bind_display, tunnel.target_host, tunnel.target_port
        ),
        TunnelKind::Dynamic => format!("{}  {} (SOCKS5)", kind_letter, bind_display),
    };

    // Status pill content.
    let (status_dot, status_text) = if tunnel_running {
        let suffix = if tunnel_borrowed {
            t!("tunnels.borrowed").to_string()
        } else {
            t!("tunnels.owned").to_string()
        };
        (
            STATUS_RUNNING_COLOR,
            format!("{} ({})", t!("tunnels.running").to_string(), suffix),
        )
    } else {
        (STATUS_STOPPED_COLOR, t!("tunnels.stopped").to_string())
    };

    // Wrap the action callbacks in Rc so they can be cloned into both the
    // inline buttons and the context-menu items.
    let on_start_rc = Rc::new(on_start);
    let on_stop_rc = Rc::new(on_stop);
    let on_edit_rc = Rc::new(on_edit);
    let on_remove_rc = Rc::new(on_remove);

    div()
        .id(row_id.clone())
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_3()
        .py_2()
        .rounded_md()
        .bg(rgb(BG_BASE))
        // Right-click context menu: Start/Stop (contextual), Edit, Delete.
        .on_mouse_down(MouseButton::Right, {
            let on_edit = on_edit_rc.clone();
            let on_remove = on_remove_rc.clone();
            let on_start = on_start_rc.clone();
            let on_stop = on_stop_rc.clone();
            let entity = entity.clone();
            // Clone these here so the closure captures fresh copies, leaving
            // the originals available for the inline action buttons below.
            let alert_controller = alert_controller.clone();
            move |event, _w, cx| {
                let Some(ref cm) = context_menu else {
                    return;
                };
                // Mark this row as the menu-triggering row so it keeps the
                // hover background while the overlay is up.
                let _ = entity.update(cx, |view, cx| {
                    view.context_menu_tunnel_id = Some(tunnel_id);
                    cx.notify();
                });
                let pos = event.position;
                let on_edit = on_edit.clone();
                let on_remove = on_remove.clone();
                let on_start = on_start.clone();
                let on_stop = on_stop.clone();
                let alert_controller = alert_controller.clone();
                cm.update(cx, |c, cx| {
                    // Build the contextual Start/Stop item based on current
                    // running state.
                    let toggle_item = if tunnel_running {
                        ContextMenuItem::new(t!("tunnels.stop").to_string(), {
                            let on_stop = on_stop.clone();
                            move |w, cx| {
                                on_stop(w, cx);
                            }
                        })
                    } else {
                        ContextMenuItem::new(t!("tunnels.start").to_string(), {
                            let on_start = on_start.clone();
                            move |w, cx| {
                                on_start(w, cx);
                            }
                        })
                    };
                    c.show(
                        ContextMenuState {
                            position: pos,
                            items: vec![
                                toggle_item,
                                ContextMenuItem::new(t!("tunnels.edit").to_string(), {
                                    let on_edit = on_edit.clone();
                                    move |w, cx| {
                                        on_edit(w, cx);
                                    }
                                }),
                                ContextMenuItem::new(t!("tunnels.delete").to_string(), {
                                    let on_remove = on_remove.clone();
                                    let alert_controller = alert_controller.clone();
                                    move |_w, cx| {
                                        let Some(ref ac) = alert_controller else {
                                            return;
                                        };
                                        let on_remove = on_remove.clone();
                                        ac.update(cx, |c, cx| {
                                            c.show(
                                                AlertState {
                                                    severity: AlertSeverity::Danger,
                                                    title: t!("tunnels.delete_confirm_title")
                                                        .to_string()
                                                        .into(),
                                                    description: Some(
                                                        t!("tunnels.delete_confirm_msg")
                                                            .to_string()
                                                            .into(),
                                                    ),
                                                    confirm_label: t!("tunnels.delete")
                                                        .to_string()
                                                        .into(),
                                                    cancel_label: t!("terminal.host_key_cancel")
                                                        .to_string()
                                                        .into(),
                                                    on_confirm: Some(Rc::new(move |w, cx| {
                                                        on_remove(w, cx);
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
        // Double-click toggles start/stop. Must be on the pre-transition
        // `div` (the `AnimatedWrapper` produced by `with_transition`
        // below doesn't expose `on_mouse_down`).
        .on_mouse_down(MouseButton::Left, {
            let on_start = on_start_rc.clone();
            let on_stop = on_stop_rc.clone();
            move |event, w, cx| {
                if event.click_count >= 2 {
                    if tunnel_running {
                        on_stop(w, cx);
                    } else {
                        on_start(w, cx);
                    }
                }
            }
        })
        // Track hover of the whole row so the background color eases in.
        .with_transition(row_id)
        .on_hover(move |hovered, _w, cx| {
            let _ = entity.update(cx, |view, cx| {
                if *hovered {
                    view.hovered_tunnel_id = Some(tunnel_id);
                } else if view.hovered_tunnel_id == Some(tunnel_id) {
                    view.hovered_tunnel_id = None;
                }
                cx.notify();
            });
        })
        .transition_when_else(
            is_highlighted,
            Duration::from_millis(120),
            Linear,
            |el| el.bg(rgb(SURFACE_ACTIVE)),
            |el| el.bg(rgb(BG_BASE)),
        )
        // --- Left: kind badge + tunnel info ---
        .child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap_2()
                .min_w_0()
                .flex_1()
                // Kind badge (single letter, color-coded)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size_5()
                        .rounded_md()
                        .bg(rgba((kind_color << 8) | 0x22))
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(rgb(kind_color))
                        .child(kind_letter.to_string()),
                )
                // Name + address + host
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .min_w_0()
                        .flex_1()
                        .gap_0p5()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(rgb(TEXT_PRIMARY))
                                .child(tunnel.name.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(TEXT_MUTED))
                                .child(address_line),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(TEXT_MUTED))
                                .child(format!("{} · {}", host_name, kind_label)),
                        ),
                ),
        )
        // --- Right: status pill only ---
        // Start/Stop/Edit/Delete live in the right-click context menu
        // (see `on_mouse_down` above). Double-click the row toggles
        // start/stop.
        .child(
            div().flex().flex_row().items_center().gap_2().child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_1()
                    .child(div().size_2().rounded_full().bg(rgb(status_dot)))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(TEXT_MUTED))
                            .child(status_text),
                    ),
            ),
        )
}
