use gpui::{prelude::FluentBuilder, *};
use gpui_animation::{animation::TransitionExt, transition::general::Linear};
use gpui_component::input::InputState;
use gpui_component::scroll::ScrollableElement as _;
use rust_i18n::t;
use std::time::Duration;

use crate::app::CrabportApp;
use crate::color::*;
use crate::components::input::{StyledInput, StyledPasswordInput};
use crate::components::tabs::{TabPane, Tabs};
use crabport_core::credential::{CredentialEntry, CredentialKind as CoreCredentialKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PasswordSubKind {
    Temporary,
    Saved,
}

#[derive(IntoElement)]
pub struct WithPasswordForm {
    pub sub_kind: PasswordSubKind,
    pub user_input: Entity<InputState>,
    pub pass_input: Entity<InputState>,
    pub saved_user_input: Entity<InputState>,
    pub saved_pass_input: Entity<InputState>,
    pub user_focused: bool,
    pub pass_focused: bool,
    pub saved_user_focused: bool,
    pub saved_pass_focused: bool,
    pub credentials: Vec<CredentialEntry>,
    pub selected_credential_id: Option<i64>,
    pub app: Entity<CrabportApp>,
}

impl RenderOnce for WithPasswordForm {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let sub_index = match self.sub_kind {
            PasswordSubKind::Temporary => 0,
            PasswordSubKind::Saved => 1,
        };

        let selected = self.selected_credential_id;
        let app = self.app.clone();

        Tabs::new("conn-password-sub-tabs")
            .h_full()
            .active(sub_index)
            .pane(TabPane::new(
                t!("connection_form.auth_temporary").to_string(),
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div().child(
                            StyledInput::new("username", self.user_input)
                                .label(t!("connection_form.username").to_string())
                                .focused(self.user_focused),
                        ),
                    )
                    .child(
                        div().child(
                            StyledPasswordInput::new("password", self.pass_input)
                                .label(t!("connection_form.password").to_string())
                                .focused(self.pass_focused)
                                .on_toggle(|_, _| {}),
                        ),
                    ),
            ))
            .pane(TabPane::new(
                t!("connection_form.auth_saved").to_string(),
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div().child(
                            StyledInput::new("saved-username", self.saved_user_input)
                                .label(t!("connection_form.username").to_string())
                                .focused(self.saved_user_focused),
                        ),
                    )
                    .child(
                        div().child(
                            StyledPasswordInput::new("saved-password", self.saved_pass_input)
                                .label(t!("connection_form.password").to_string())
                                .focused(self.saved_pass_focused)
                                .on_toggle(|_, _| {}),
                        ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(TEXT_MUTED))
                                    .child(t!("connection_form.select_credential").to_string()),
                            )
                            .when_else(
                                self.credentials.is_empty(),
                                |el| {
                                    el.child(
                                        div()
                                            .text_sm()
                                            .text_color(rgb(TEXT_MUTED))
                                            .child(t!("credentials.empty").to_string()),
                                    )
                                },
                                |el| {
                                    el.child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap_1()
                                            .max_h(px(160.0))
                                            .overflow_y_scrollbar()
                                            .children(
                                                self.credentials.into_iter().map(|c| {
                                                    credential_row(c, selected, app.clone())
                                                }),
                                            ),
                                    )
                                },
                            ),
                    ),
            ))
            .on_change({
                let app = self.app.clone();
                move |index, _w, cx| {
                    app.update(cx, |app, cx| {
                        if let Some(ref mut form) = app.connection_form {
                            form.password_sub_kind = match index {
                                0 => PasswordSubKind::Temporary,
                                _ => PasswordSubKind::Saved,
                            };
                            cx.notify();
                        }
                    });
                }
            })
    }
}

fn credential_row(
    cred: CredentialEntry,
    selected_id: Option<i64>,
    app: Entity<CrabportApp>,
) -> impl IntoElement {
    let is_selected = selected_id == Some(cred.id);
    let cred_id = cred.id;

    let kind_label = match cred.kind {
        CoreCredentialKind::Password => t!("credential_form.type_password").to_string(),
        CoreCredentialKind::Certificate => t!("credential_form.type_certificate").to_string(),
    };

    let row_id = ElementId::Name(format!("conn-cred-row-{}", cred.id).into());

    div()
        .id(row_id.clone())
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_3()
        .py_2()
        .rounded_md()
        .cursor_pointer()
        .when_else(
            is_selected,
            |el| {
                el.bg(rgb(SURFACE_ACTIVE))
                    .border_1()
                    .border_color(rgb(BTN_PRIMARY_BORDER))
            },
            |el| el.bg(rgb(BG_BASE)),
        )
        .on_click(move |_e, _w, cx| {
            app.update(cx, |app, cx| {
                if let Some(ref mut form) = app.connection_form {
                    form.selected_credential_id = if is_selected { None } else { Some(cred_id) };
                    cx.notify();
                }
            });
        })
        .with_transition(row_id)
        .transition_on_hover(Duration::from_millis(120), Linear, move |hovered, s| {
            if *hovered && !is_selected {
                s.bg(rgb(SURFACE_ACTIVE))
            } else {
                s
            }
        })
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(TEXT_PRIMARY))
                        .child(cred.name.clone()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(TEXT_MUTED))
                        .child(kind_label),
                ),
        )
}
