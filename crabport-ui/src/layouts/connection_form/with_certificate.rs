use gpui::*;
use gpui_component::input::InputState;
use rust_i18n::t;

use crate::components::input::{StyledInput, StyledPasswordInput};

#[derive(IntoElement)]
pub struct WithCertificateForm {
    pub user_input: Entity<InputState>,
    pub pass_input: Entity<InputState>,
    pub private_key_input: Entity<InputState>,
    pub public_key_input: Entity<InputState>,
    pub certificate_input: Entity<InputState>,
    pub user_focused: bool,
    pub pass_focused: bool,
    pub private_key_focused: bool,
    pub public_key_focused: bool,
    pub certificate_focused: bool,
}

impl RenderOnce for WithCertificateForm {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap_4()
            // Username
            .child(
                div().child(
                    StyledInput::new("username", self.user_input)
                        .label(t!("connection_form.username").to_string())
                        .focused(self.user_focused),
                ),
            )
            // Passphrase
            .child(
                div().child(
                    StyledPasswordInput::new("password", self.pass_input)
                        .label(t!("credential_form.passphrase").to_string())
                        .focused(self.pass_focused)
                        .on_toggle(|_, _| {}),
                ),
            )
            // Private Key
            .child(
                div().child(
                    StyledInput::new("conn-private-key", self.private_key_input)
                        .label(t!("credential_form.private_key").to_string())
                        .focused(self.private_key_focused),
                ),
            )
            // Public Key
            .child(
                div().child(
                    StyledInput::new("conn-public-key", self.public_key_input)
                        .label(t!("credential_form.public_key").to_string())
                        .focused(self.public_key_focused),
                ),
            )
            // Certificate
            .child(
                div().child(
                    StyledInput::new("conn-certificate", self.certificate_input)
                        .label(t!("credential_form.certificate").to_string())
                        .focused(self.certificate_focused),
                ),
            )
    }
}
