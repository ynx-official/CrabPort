use gpui::{prelude::FluentBuilder, *};
use gpui_animation::transition::general;
use gpui_animation::{animation::TransitionExt, transition::general::Linear};
use gpui_component::input::InputState;
use rust_i18n::t;
use std::rc::Rc;
use std::time::Duration;

use crate::color::*;
use crate::components::button::Button;
use crate::components::input::{StyledInput, StyledPasswordInput};
use crate::components::segmented_control::{Segment, SegmentedControl};

// ---------------------------------------------------------------------------
// Credential type
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum CredentialKind {
    Password,
    Certificate,
}

// ---------------------------------------------------------------------------
// CredentialFormView
// ---------------------------------------------------------------------------

pub struct CredentialFormView {
    pub active: bool,
    pub kind: CredentialKind,
    name_input: Entity<InputState>,
    username_input: Entity<InputState>,
    password_input: Entity<InputState>,
    private_key_input: Entity<InputState>,
    public_key_input: Entity<InputState>,
    certificate_input: Entity<InputState>,
    name_focused: bool,
    username_focused: bool,
    password_focused: bool,
    private_key_focused: bool,
    public_key_focused: bool,
    certificate_focused: bool,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_save: Option<Rc<dyn Fn(CredentialKind, &mut Window, &mut App) + 'static>>,
    on_kind_change: Option<Rc<dyn Fn(CredentialKind, &mut Window, &mut App) + 'static>>,
}

impl CredentialFormView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let name_input = cx.new(|cx| InputState::new(window, cx));
        let username_input = cx.new(|cx| InputState::new(window, cx));
        let password_input = cx.new(|cx| {
            let mut state = InputState::new(window, cx);
            state.set_masked(true, window, cx);
            state
        });
        let private_key_input = cx.new(|cx| InputState::new(window, cx));
        let public_key_input = cx.new(|cx| InputState::new(window, cx));
        let certificate_input = cx.new(|cx| InputState::new(window, cx));

        Self {
            active: false,
            kind: CredentialKind::Password,
            name_input,
            username_input,
            password_input,
            private_key_input,
            public_key_input,
            certificate_input,
            name_focused: false,
            username_focused: false,
            password_focused: false,
            private_key_focused: false,
            public_key_focused: false,
            certificate_focused: false,
            on_close: None,
            on_save: None,
            on_kind_change: None,
        }
    }

    pub fn open(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.active = true;
        self.name_input.update(cx, |state, cx| {
            state.focus(window, cx);
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

    pub fn set_on_save(&mut self, f: impl Fn(CredentialKind, &mut Window, &mut App) + 'static) {
        self.on_save = Some(Rc::new(f));
    }

    pub fn set_on_kind_change(
        &mut self,
        f: impl Fn(CredentialKind, &mut Window, &mut App) + 'static,
    ) {
        self.on_kind_change = Some(Rc::new(f));
    }

    pub fn name_text(&self, cx: &App) -> String {
        self.name_input.read(cx).text().to_string()
    }

    pub fn username_text(&self, cx: &App) -> String {
        self.username_input.read(cx).text().to_string()
    }

    /// Returns the secret text (password in Password mode, passphrase in Certificate mode)
    pub fn secret_text(&self, cx: &App) -> String {
        self.password_input.read(cx).text().to_string()
    }

    pub fn private_key_text(&self, cx: &App) -> String {
        self.private_key_input.read(cx).text().to_string()
    }

    pub fn public_key_text(&self, cx: &App) -> String {
        self.public_key_input.read(cx).text().to_string()
    }

    pub fn certificate_text(&self, cx: &App) -> String {
        self.certificate_input.read(cx).text().to_string()
    }
}

impl Render for CredentialFormView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let active = self.active;
        let kind = self.kind;
        let name_focused = self.name_focused;
        let username_focused = self.username_focused;
        let password_focused = self.password_focused;
        let private_key_focused = self.private_key_focused;
        let public_key_focused = self.public_key_focused;
        let certificate_focused = self.certificate_focused;

        let name_input = self.name_input.clone();
        let username_input = self.username_input.clone();
        let password_input = self.password_input.clone();
        let private_key_input = self.private_key_input.clone();
        let public_key_input = self.public_key_input.clone();
        let certificate_input = self.certificate_input.clone();

        render_overlay(
            active,
            &self.on_close,
            render_dialog(
                active,
                kind,
                name_input,
                username_input,
                password_input,
                private_key_input,
                public_key_input,
                certificate_input,
                name_focused,
                username_focused,
                password_focused,
                private_key_focused,
                public_key_focused,
                certificate_focused,
                &self.on_close,
                &self.on_save,
                &self.on_kind_change,
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
    let overlay_id = ElementId::Name("cred-form-overlay".into());

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

#[allow(clippy::too_many_arguments)]
fn render_dialog(
    active: bool,
    kind: CredentialKind,
    name_input: Entity<InputState>,
    username_input: Entity<InputState>,
    password_input: Entity<InputState>,
    private_key_input: Entity<InputState>,
    public_key_input: Entity<InputState>,
    certificate_input: Entity<InputState>,
    name_focused: bool,
    username_focused: bool,
    password_focused: bool,
    private_key_focused: bool,
    public_key_focused: bool,
    certificate_focused: bool,
    on_close: &Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_save: &Option<Rc<dyn Fn(CredentialKind, &mut Window, &mut App) + 'static>>,
    on_kind_change: &Option<Rc<dyn Fn(CredentialKind, &mut Window, &mut App) + 'static>>,
) -> impl IntoElement {
    let dialog_id = ElementId::Name("cred-form-dialog".into());

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
                .child(t!("credential_form.title").to_string()),
        )
        // Name
        .child(
            div().child(
                StyledInput::new("cred-name", name_input)
                    .label(t!("credential_form.name").to_string())
                    .focused(name_focused),
            ),
        )
        // Type selector
        .child(render_type_selector(kind, on_kind_change))
        // Fields
        .child(render_fields(
            kind,
            username_input,
            password_input,
            private_key_input,
            public_key_input,
            certificate_input,
            username_focused,
            password_focused,
            private_key_focused,
            public_key_focused,
            certificate_focused,
        ))
        // Buttons
        .child(render_buttons(kind, on_close, on_save))
}

// ---------------------------------------------------------------------------
// Type selector with sliding indicator
// ---------------------------------------------------------------------------

fn render_type_selector(
    kind: CredentialKind,
    on_kind_change: &Option<Rc<dyn Fn(CredentialKind, &mut Window, &mut App) + 'static>>,
) -> impl IntoElement {
    let on_change_pw = on_kind_change.clone();
    let on_change_cert = on_kind_change.clone();

    let active_index = if kind == CredentialKind::Password {
        0
    } else {
        1
    };

    SegmentedControl::new("cred-type-selector")
        .active(active_index)
        .segment(
            Segment::new(t!("credential_form.type_password").to_string()).on_select(
                move |w, cx| {
                    if let Some(ref cb) = on_change_pw {
                        cb(CredentialKind::Password, w, cx);
                    }
                },
            ),
        )
        .segment(
            Segment::new(t!("credential_form.type_certificate").to_string()).on_select(
                move |w, cx| {
                    if let Some(ref cb) = on_change_cert {
                        cb(CredentialKind::Certificate, w, cx);
                    }
                },
            ),
        )
}

// ---------------------------------------------------------------------------
// Fields — shared username/secret + animated certificate extras
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn render_fields(
    kind: CredentialKind,
    username_input: Entity<InputState>,
    password_input: Entity<InputState>,
    private_key_input: Entity<InputState>,
    public_key_input: Entity<InputState>,
    certificate_input: Entity<InputState>,
    username_focused: bool,
    password_focused: bool,
    private_key_focused: bool,
    public_key_focused: bool,
    certificate_focused: bool,
) -> impl IntoElement {
    let is_password = kind == CredentialKind::Password;

    // Shared: password / passphrase — always visible, label changes by kind
    let secret_label = if is_password {
        t!("credential_form.password").to_string()
    } else {
        t!("credential_form.passphrase").to_string()
    };

    // Certificate-only extras (expand/collapse)
    let cert_extras_id = ElementId::Name("cred-cert-extras".into());
    let cert_extras = div()
        .id(cert_extras_id.clone())
        .flex()
        .flex_col()
        .gap_4()
        .overflow_hidden()
        .with_transition(cert_extras_id)
        .transition_when_else(
            !is_password,
            Duration::from_millis(200),
            general::EaseInQuad,
            |s| s.max_h(px(200.0)).opacity(1.0),
            |s| s.max_h(px(0.0)).opacity(0.0),
        )
        .child(
            div().child(
                StyledInput::new("cred-private-key", private_key_input)
                    .label(t!("credential_form.private_key").to_string())
                    .focused(private_key_focused),
            ),
        )
        .child(
            div().child(
                StyledInput::new("cred-public-key", public_key_input)
                    .label(t!("credential_form.public_key").to_string())
                    .focused(public_key_focused),
            ),
        )
        .child(
            div().child(
                StyledInput::new("cred-certificate", certificate_input)
                    .label(t!("credential_form.certificate").to_string())
                    .focused(certificate_focused),
            ),
        );

    div()
        .flex()
        .flex_col()
        .gap_4()
        // Username — shared
        .child(
            div().child(
                StyledInput::new("cred-username", username_input)
                    .label(t!("credential_form.username").to_string())
                    .focused(username_focused),
            ),
        )
        // Password / Passphrase — shared
        .child(
            div().child(
                StyledPasswordInput::new("cred-secret", password_input)
                    .label(secret_label)
                    .focused(password_focused),
            ),
        )
        // Certificate extras — animated
        .child(cert_extras)
}

// ---------------------------------------------------------------------------
// Buttons
// ---------------------------------------------------------------------------

fn render_buttons(
    kind: CredentialKind,
    on_close: &Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    on_save: &Option<Rc<dyn Fn(CredentialKind, &mut Window, &mut App) + 'static>>,
) -> impl IntoElement {
    let on_close_btn = on_close.clone();
    let on_save_btn = on_save.clone();
    div()
        .flex()
        .flex_row()
        .gap_3()
        .justify_end()
        .child(
            Button::new("cred-cancel")
                .centered(true)
                .child(t!("credential_form.cancel").to_string())
                .on_click(move |_e, w, cx| {
                    if let Some(ref cb) = on_close_btn {
                        cb(w, cx);
                    }
                }),
        )
        .child(
            Button::new("cred-save")
                .primary()
                .centered(true)
                .child(t!("credential_form.save").to_string())
                .on_click(move |_e, w, cx| {
                    if let Some(ref cb) = on_save_btn {
                        cb(kind, w, cx);
                    }
                }),
        )
}
