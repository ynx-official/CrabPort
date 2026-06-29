use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_animation::{animation::TransitionExt, transition::general::Linear};
use gpui_component::InteractiveElementExt;
use gpui_component::scroll::ScrollableElement as _;
use rust_i18n::t;
use std::time::Duration;

use crate::app::CrabportApp;
use crate::color::*;
use crate::components::button::Button;
use crate::layouts::connection_form::{ConnectionFormState, ConnectionFormView};

/// A saved connection host entry.
#[derive(Clone)]
pub struct ConnectionHost {
    pub id: i64,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub kind: crate::layouts::connection_form::ConnectionKind,
    pub credential_id: Option<i64>,
    pub last_login: Option<i64>,
    pub favorite: bool,
}

/// Hosts sidebar view.
///
/// Holds its own hover state (`hovered_host_id`) so the action buttons can
/// fade in with easing when the row is hovered — without polluting
/// `CrabportApp` state or risking "already being updated" panics.
pub struct HostsView {
    /// The host row currently being hovered, if any.
    hovered_host_id: Option<i64>,
    // External data pushed in before each render.
    hosts: Vec<ConnectionHost>,
    form_state: Option<ConnectionFormState>,
    app: Entity<CrabportApp>,
    // Callbacks
    on_new: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
    on_connect: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
    on_edit: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
    on_remove: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
}

impl HostsView {
    pub fn new(app: Entity<CrabportApp>) -> Self {
        Self {
            hovered_host_id: None,
            hosts: Vec::new(),
            form_state: None,
            app,
            on_new: None,
            on_connect: None,
            on_edit: None,
            on_remove: None,
        }
    }

    /// Push the latest external state into the view before render.
    pub fn set_state(
        &mut self,
        hosts: Vec<ConnectionHost>,
        form_state: Option<ConnectionFormState>,
        on_new: Option<Rc<dyn Fn(&mut Window, &mut App)>>,
        on_connect: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
        on_edit: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
        on_remove: Option<Rc<dyn Fn(i64, &mut Window, &mut App)>>,
        cx: &mut Context<Self>,
    ) {
        // Clear stale hover if the host disappeared.
        if let Some(id) = self.hovered_host_id
            && !hosts.iter().any(|h| h.id == id)
        {
            self.hovered_host_id = None;
        }
        self.hosts = hosts;
        self.form_state = form_state;
        self.on_new = on_new;
        self.on_connect = on_connect;
        self.on_edit = on_edit;
        self.on_remove = on_remove;
        // Note: do NOT call cx.notify() here — set_state is invoked every
        // render from render_content, so notifying would cause an infinite
        // loop. The HostsView re-renders naturally because its parent
        // (CrabportApp) re-renders.
        let _ = cx;
    }
}

impl Render for HostsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let hosts = self.hosts.clone();
        let form_state = self.form_state.clone();
        let app = self.app.clone();
        let on_new = self.on_new.clone();
        let on_connect = self.on_connect.clone();
        let on_edit = self.on_edit.clone();
        let on_remove = self.on_remove.clone();
        let hovered_host_id = self.hovered_host_id;

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
                            .child(t!("sidebar.sessions").to_string()),
                    )
                    .child(
                        Button::new("hosts-new-btn")
                            .primary()
                            .icon("icons/plus.svg")
                            .w_auto()
                            .px_2()
                            .child(t!("sessions.new_button").to_string())
                            .on_click(move |_e, w, cx| {
                                if let Some(ref cb) = on_new {
                                    cb(w, cx);
                                }
                            }),
                    ),
            )
            // --- Separator ---
            .child(div().h_px().bg(rgb(BORDER)).mx_4())
            // --- Hosts list (or empty state) ---
            .child(
                div()
                    .flex_1()
                    .overflow_y_scrollbar()
                    .px_4()
                    .py_2()
                    .when_else(
                        hosts.is_empty(),
                        |el| {
                            el.flex().items_center().justify_center().child(
                                div()
                                    .text_color(rgb(TEXT_MUTED))
                                    .text_sm()
                                    .child(t!("sessions.empty").to_string()),
                            )
                        },
                        |el| {
                            el.flex().flex_col().gap_1().children(hosts.iter().map(|h| {
                                let host = h.clone();
                                let on_connect = on_connect.clone();
                                let on_edit = on_edit.clone();
                                let on_remove = on_remove.clone();
                                let is_hovered = hovered_host_id == Some(h.id);
                                let entity = _cx.entity().downgrade();

                                host_row(
                                    &host,
                                    is_hovered,
                                    entity,
                                    move |w, cx| {
                                        if let Some(ref cb) = on_connect {
                                            cb(host.id, w, cx);
                                        }
                                    },
                                    move |w, cx| {
                                        if let Some(ref cb) = on_edit {
                                            cb(host.id, w, cx);
                                        }
                                    },
                                    move |w, cx| {
                                        if let Some(ref cb) = on_remove {
                                            cb(host.id, w, cx);
                                        }
                                    },
                                )
                                .into_any_element()
                            }))
                        },
                    ),
            )
            // --- Connection form overlay ---
            .when_some(form_state, |el, state| {
                el.child(ConnectionFormView::new(&state, app))
            })
    }
}

// ---------------------------------------------------------------------------
// Host row
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn host_row(
    host: &ConnectionHost,
    is_hovered: bool,
    entity: WeakEntity<HostsView>,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
    on_edit: impl Fn(&mut Window, &mut App) + 'static,
    on_remove: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let row_id = ElementId::Name(format!("host-row-{}", host.id).into());
    let row_id_clone = row_id.clone();

    let edit_btn_id = ElementId::Name(format!("host-edit-{}", host.id).into());
    let remove_btn_id = ElementId::Name(format!("host-remove-{}", host.id).into());
    let edit_opacity_id = ElementId::Name(format!("host-edit-op-{}", host.id).into());
    let remove_opacity_id = ElementId::Name(format!("host-remove-op-{}", host.id).into());

    let host_id = host.id;

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
        .on_double_click(move |_, w, cx| {
            gpui_animation::reset_transition(&row_id_clone);
            on_click(w, cx);
        })
        // Track hover of the whole row so the action buttons can fade in
        // with easing via `transition_when_else` below. State lives in the
        // HostsView entity itself. This `.on_hover` is chained on the same
        // animated wrapper as `transition_on_hover` (gpui-animation allows
        // both; only gpui's native `on_hover` forbids duplicates).
        .with_transition(row_id)
        .on_hover(move |hovered, _w, cx| {
            let _ = entity.update(cx, |view, cx| {
                if *hovered {
                    // Entering this row — claim hover unconditionally.
                    view.hovered_host_id = Some(host_id);
                } else {
                    // Leaving this row — only clear if we still own hover.
                    // Otherwise we'd clobber the new row's hover claim when
                    // moving downward (leave-old fires after enter-new).
                    if view.hovered_host_id == Some(host_id) {
                        view.hovered_host_id = None;
                    }
                }
                cx.notify();
            });
        })
        .transition_on_hover(Duration::from_millis(120), Linear, |hovered, s| {
            if *hovered {
                s.bg(rgb(SURFACE_ACTIVE))
            } else {
                s.bg(rgb(BG_BASE))
            }
        })
        // Host info (name + address)
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .flex_1()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(TEXT_PRIMARY))
                        .child(host.name.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(TEXT_MUTED))
                        .child(format!("{}@{}:{}", host.username, host.host, host.port)),
                ),
        )
        // Edit button (fades in on row hover)
        .child(
            div()
                .id(edit_opacity_id.clone())
                .flex()
                .items_center()
                .justify_center()
                .opacity(0.)
                .child(
                    Button::new(edit_btn_id)
                        .ghost()
                        .centered(true)
                        .w(px(28.0))
                        .h(px(28.0))
                        .border_0()
                        .rounded_sm()
                        .child(
                            svg()
                                .path("icons/square-pen.svg")
                                .size_4()
                                .text_color(rgb(TEXT_MUTED)),
                        )
                        .on_click(move |_e, w, cx| {
                            on_edit(w, cx);
                        }),
                )
                .with_transition(edit_opacity_id)
                .transition_when_else(
                    is_hovered,
                    Duration::from_millis(120),
                    Linear,
                    |el| el.opacity(0.7),
                    |el| el.opacity(0.),
                ),
        )
        // Remove button (fades in on row hover)
        .child(
            div()
                .id(remove_opacity_id.clone())
                .flex()
                .items_center()
                .justify_center()
                .opacity(0.)
                .child(
                    Button::new(remove_btn_id)
                        .ghost()
                        .centered(true)
                        .w(px(28.0))
                        .h(px(28.0))
                        .border_0()
                        .rounded_sm()
                        .child(
                            svg()
                                .path("icons/trash.svg")
                                .size_4()
                                .text_color(rgb(TEXT_MUTED)),
                        )
                        .on_click(move |_e, w, cx| {
                            on_remove(w, cx);
                        }),
                )
                .with_transition(remove_opacity_id)
                .transition_when_else(
                    is_hovered,
                    Duration::from_millis(120),
                    Linear,
                    |el| el.opacity(0.7),
                    |el| el.opacity(0.),
                ),
        )
}
