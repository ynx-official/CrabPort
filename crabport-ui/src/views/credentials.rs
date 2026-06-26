use gpui::{prelude::FluentBuilder, *};
use gpui_component::scroll::ScrollableElement as _;
use rust_i18n::t;

use crate::color::*;
use crate::components::button::Button;
use crate::layouts::credential_form::CredentialFormView;

/// Render the credentials sidebar view.
pub fn render_credentials_view(
    form_entity: Option<&Entity<CredentialFormView>>,
    on_new: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
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
                        .child(t!("sidebar.credentials").to_string()),
                )
                .child(
                    Button::new("creds-new-btn")
                        .primary()
                        .icon("icons/plus.svg")
                        .w_auto()
                        .px_2()
                        .child(t!("credentials.new_button").to_string())
                        .on_click(move |_e, w, cx| {
                            on_new(w, cx);
                        }),
                ),
        )
        // --- Separator ---
        .child(div().h_px().bg(rgb(BORDER)).mx_4())
        // --- Placeholder ---
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
                        .child(t!("credentials.empty").to_string()),
                ),
        )
        // --- Credential form overlay ---
        .when_some(form_entity.cloned(), |el, form| el.child(form))
}
