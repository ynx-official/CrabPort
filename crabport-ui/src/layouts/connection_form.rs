use gpui::{prelude::FluentBuilder, *};
use gpui_animation::{animation::TransitionExt, transition::general::Linear};
use gpui_component::input::InputState;
use rust_i18n::t;
use std::rc::Rc;
use std::time::Duration;

use crate::color::*;
use crate::components::button::Button;
use crate::components::input::{StyledInput, StyledPasswordInput};

// ---------------------------------------------------------------------------
// Connection type
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ConnectionKind {
    SSH,
    Telnet,
    Serial,
}

// ---------------------------------------------------------------------------
// ConnectionFormView
// ---------------------------------------------------------------------------

pub struct ConnectionFormView {
    pub active: bool,
    kind: ConnectionKind,
    name_input: Entity<InputState>,
    host_input: Entity<InputState>,
    port_input: Entity<InputState>,
    user_input: Entity<InputState>,
    pass_input: Entity<InputState>,
    name_focused: bool,
    host_focused: bool,
    port_focused: bool,
    user_focused: bool,
    pass_focused: bool,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_connect: Option<Rc<dyn Fn(ConnectionKind, &mut Window, &mut App) + 'static>>,
}

impl ConnectionFormView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_input = cx.new(|cx| InputState::new(window, cx));
        let host_input = cx.new(|cx| InputState::new(window, cx));
        let port_input = cx.new(|cx| InputState::new(window, cx));
        let user_input = cx.new(|cx| InputState::new(window, cx));
        let pass_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_masked(true, window, cx);
            state
        });

        Self {
            active: false,
            kind: ConnectionKind::SSH,
            name_input,
            host_input,
            port_input,
            user_input,
            pass_input,
            name_focused: false,
            host_focused: false,
            port_focused: false,
            user_focused: false,
            pass_focused: false,
            on_close: None,
            on_connect: None,
        }
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.active = true;
        self.name_input.update(cx, |state, cx| {
            state.focus(window, cx);
        });
        self.port_input.update(cx, |state, cx| {
            state.set_value("22", window, cx);
        });
        cx.notify();
    }

    pub fn close(&mut self, cx: &mut Context<Self>) {
        if self.active {
            self.active = false;
            cx.notify();
        }
    }

    pub fn set_on_close(&mut self, f: impl Fn(&mut Window, &mut App) + 'static) {
        self.on_close = Some(Rc::new(f));
    }

    pub fn set_on_connect(&mut self, f: impl Fn(ConnectionKind, &mut Window, &mut App) + 'static) {
        self.on_connect = Some(Rc::new(f));
    }

    pub fn name_text(&self, cx: &App) -> String {
        self.name_input.read(cx).text().to_string()
    }

    pub fn host_text(&self, cx: &App) -> String {
        self.host_input.read(cx).text().to_string()
    }

    pub fn port_text(&self, cx: &App) -> String {
        self.port_input.read(cx).text().to_string()
    }

    pub fn user_text(&self, cx: &App) -> String {
        self.user_input.read(cx).text().to_string()
    }

    pub fn pass_text(&self, cx: &App) -> String {
        self.pass_input.read(cx).text().to_string()
    }
}

impl Render for ConnectionFormView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active;
        let kind = self.kind;
        let name_focused = self.name_focused;
        let host_focused = self.host_focused;
        let port_focused = self.port_focused;
        let user_focused = self.user_focused;
        let pass_focused = self.pass_focused;

        let name_input = self.name_input.clone();
        let host_input = self.host_input.clone();
        let port_input = self.port_input.clone();
        let user_input = self.user_input.clone();
        let pass_input = self.pass_input.clone();

        render_overlay(
            active,
            &self.on_close,
            render_dialog(
                active,
                kind,
                name_input,
                host_input,
                port_input,
                user_input,
                pass_input,
                name_focused,
                host_focused,
                port_focused,
                user_focused,
                pass_focused,
                &self.on_close,
                &self.on_connect,
            ),
        )
    }
}

// ---------------------------------------------------------------------------
// Render helpers
// ---------------------------------------------------------------------------

fn render_overlay(
    active: bool,
    on_close: &Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    child: impl IntoElement,
) -> impl IntoElement {
    let overlay_id = ElementId::Name("conn-form-overlay".into());

    div()
        .id(overlay_id.clone())
        .absolute()
        .size_full()
        .top_0()
        .left_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(0x00000000))
        .when(active, |el| {
            el.occlude().on_mouse_down(MouseButton::Left, {
                let on_close = on_close.clone();
                move |_e, w, cx| {
                    if let Some(ref cb) = on_close {
                        cb(w, cx);
                    }
                }
            })
        })
        .with_transition(overlay_id)
        .transition_when_else(
            active,
            Duration::from_millis(150),
            Linear,
            |el| el.bg(rgba(0x00000080)),
            |el| el.bg(rgba(0x00000000)),
        )
        .child(child)
}

fn render_dialog(
    active: bool,
    kind: ConnectionKind,
    name_input: Entity<InputState>,
    host_input: Entity<InputState>,
    port_input: Entity<InputState>,
    user_input: Entity<InputState>,
    pass_input: Entity<InputState>,
    name_focused: bool,
    host_focused: bool,
    port_focused: bool,
    user_focused: bool,
    pass_focused: bool,
    on_close: &Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_connect: &Option<Rc<dyn Fn(ConnectionKind, &mut Window, &mut App) + 'static>>,
) -> impl IntoElement {
    let dialog_id = ElementId::Name("conn-form-dialog".into());

    div()
        .id(dialog_id.clone())
        .w(px(420.0))
        .bg(rgb(BG_BASE))
        .border_1()
        .border_color(rgb(BORDER))
        .rounded_lg()
        .shadow_lg()
        .flex()
        .flex_col()
        .p_6()
        .gap_4()
        .opacity(0.0)
        .mt(px(-16.0))
        .when(active, |el| {
            el.on_mouse_down(MouseButton::Left, |_, _, cx| {
                cx.stop_propagation();
            })
        })
        .with_transition(dialog_id)
        .transition_when_else(
            active,
            Duration::from_millis(150),
            Linear,
            |el| el.opacity(1.0).mt_0(),
            |el| el.opacity(0.0).mt(px(-16.0)),
        )
        // Title
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(rgb(TEXT_PRIMARY))
                .child(t!("connection_form.title").to_string()),
        )
        // Name
        .child(
            div().child(
                StyledInput::new("name", name_input)
                    .label(t!("connection_form.name").to_string())
                    .focused(name_focused),
            ),
        )
        // Type selector
        .child(render_type_selector(kind))
        // Host + Port row
        .child(render_host_port_row(
            host_input,
            port_input,
            host_focused,
            port_focused,
        ))
        // Username
        .child(
            div().child(
                StyledInput::new("username", user_input)
                    .label(t!("connection_form.username").to_string())
                    .focused(user_focused),
            ),
        )
        // Password
        .child(
            div().child(
                StyledPasswordInput::new("password", pass_input)
                    .label(t!("connection_form.password").to_string())
                    .focused(pass_focused)
                    .on_toggle(|_, _| {}),
            ),
        )
        // Buttons
        .child(render_buttons(kind, on_close, on_connect))
}

fn render_type_selector(kind: ConnectionKind) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .gap_1()
        .bg(rgb(SURFACE_ACTIVE))
        .rounded_md()
        .p_0p5()
        .child(type_tab(
            ConnectionKind::SSH,
            kind,
            t!("new_connection.ssh"),
        ))
        .child(type_tab(
            ConnectionKind::Telnet,
            kind,
            t!("new_connection.telnet"),
        ))
        .child(type_tab(
            ConnectionKind::Serial,
            kind,
            t!("new_connection.serial"),
        ))
}

fn render_host_port_row(
    host_input: Entity<InputState>,
    port_input: Entity<InputState>,
    host_focused: bool,
    port_focused: bool,
) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .gap_3()
        .child(
            div().flex_1().child(
                StyledInput::new("host", host_input)
                    .label(t!("connection_form.host").to_string())
                    .focused(host_focused),
            ),
        )
        .child(
            div().w(px(96.0)).child(
                StyledInput::new("port", port_input)
                    .label(t!("connection_form.port").to_string())
                    .focused(port_focused),
            ),
        )
}

fn render_buttons(
    kind: ConnectionKind,
    on_close: &Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_connect: &Option<Rc<dyn Fn(ConnectionKind, &mut Window, &mut App) + 'static>>,
) -> impl IntoElement {
    let on_close_btn = on_close.clone();
    let on_connect_btn = on_connect.clone();
    div()
        .flex()
        .flex_row()
        .gap_3()
        .justify_end()
        .child(
            Button::new("conn-cancel")
                .centered(true)
                .child(t!("connection_form.cancel").to_string())
                .on_click(move |_e, w, cx| {
                    if let Some(ref cb) = on_close_btn {
                        cb(w, cx);
                    }
                }),
        )
        .child(
            Button::new("conn-connect")
                .primary()
                .centered(true)
                .child(t!("connection_form.connect").to_string())
                .on_click(move |_e, w, cx| {
                    if let Some(ref cb) = on_connect_btn {
                        cb(kind, w, cx);
                    }
                }),
        )
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn type_tab(
    kind: ConnectionKind,
    active: ConnectionKind,
    label: impl Into<SharedString>,
) -> impl IntoElement {
    let is_active = kind == active;
    div()
        .flex_1()
        .px_3()
        .py_1()
        .rounded_sm()
        .text_sm()
        .text_center()
        .when(is_active, |el| {
            el.bg(rgb(BG_BASE)).text_color(rgb(TEXT_PRIMARY))
        })
        .when(!is_active, |el| el.text_color(rgb(TEXT_MUTED)))
        .child(label.into())
}
