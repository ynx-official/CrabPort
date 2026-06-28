use gpui::*;
use gpui_animation::{animation::TransitionExt, transition::general::EaseInOutCubic};
use gpui_component::scroll::ScrollableElement;

use crate::color::*;

const SFTP_SIDEBAR_WIDTH: f32 = 220.0;

pub fn render_sftp_sidebar(entries: &[(String, bool)], has_sftp: bool) -> impl IntoElement {
    div()
        .id("sftp-sidebar")
        .h_full()
        .overflow_hidden()
        .w_0()
        .with_transition("sftp-sidebar-width")
        .transition_when_else(
            has_sftp,
            std::time::Duration::from_millis(500),
            EaseInOutCubic,
            |el| el.w(px(SFTP_SIDEBAR_WIDTH)),
            |el| el.w_0(),
        )
        .child(
            div()
                .h_full()
                .border_l_1()
                .border_color(rgb(BORDER))
                .bg(rgb(BG_SIDEBAR))
                .flex()
                .flex_col()
                .pt_2()
                .px_2()
                .child(
                    div()
                        .px_2()
                        .pb_2()
                        .text_xs()
                        .text_color(rgb(TEXT_MUTED))
                        .child("SFTP"),
                )
                .child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .overflow_y_scrollbar()
                        .children(entries.iter().map(|(name, is_dir)| {
                            let icon_path = if *is_dir {
                                "icons/folder.svg"
                            } else {
                                "icons/file.svg"
                            };
                            div()
                                .id(ElementId::Name(format!("sftp-{}", name).into()))
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_1p5()
                                .px_2()
                                .py_1()
                                .rounded(px(4.0))
                                .hover(|s| s.bg(rgb(SURFACE_HOVER)))
                                .child(
                                    svg()
                                        .path(icon_path)
                                        .size(px(14.0))
                                        .flex_shrink_0()
                                        .text_color(rgb(TEXT_MUTED)),
                                )
                                .child(
                                    div()
                                        .text_xs()
                                        .text_color(rgb(TEXT_PRIMARY))
                                        .whitespace_nowrap()
                                        .overflow_hidden()
                                        .child(name.clone()),
                                )
                        })),
                ),
        )
}
