use gpui::*;
use rust_i18n::t;

use crate::color::*;

pub fn render_snippets_view() -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .child(
            div()
                .text_color(rgb(TEXT_MUTED))
                .child(t!("sidebar.snippets").to_string()),
        )
}
