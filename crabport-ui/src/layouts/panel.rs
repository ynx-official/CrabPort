pub mod sftp;

use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_animation::{animation::TransitionExt, transition::general::EaseInOutCubic};

use crate::color::*;
use crate::components::tabs::{TabPane, Tabs};

const PANEL_WIDTH: f32 = 220.0;

pub fn render_panel(
    show: bool,
    active_tab: usize,
    sftp_entries: std::sync::Arc<Vec<(String, bool)>>,
    sftp_cwd: Option<std::sync::Arc<String>>,
    on_navigate: Option<Rc<dyn Fn(String, &mut Window, &mut App)>>,
) -> impl IntoElement {
    let has_sftp = !sftp_entries.is_empty();
    let visible = show && has_sftp;

    div()
        .id("panel-sidebar")
        .h_full()
        .overflow_hidden()
        .w_0()
        .with_transition("panel-sidebar-width")
        .transition_when_else(
            visible,
            std::time::Duration::from_millis(500),
            EaseInOutCubic,
            |el| el.w(px(PANEL_WIDTH)),
            |el| el.w_0(),
        )
        .when(visible, |el| {
            el.child(
                div()
                    .h_full()
                    .border_l_1()
                    .border_color(rgb(BORDER))
                    .bg(rgb(BG_SIDEBAR))
                    .child(
                        Tabs::new("panel-tabs")
                            .ctrl_style(|s| s.bg(rgb(BG_SIDEBAR)).rounded_none().p_0())
                            .active(active_tab)
                            .pane(TabPane::new(
                                "SFTP",
                                sftp::render_sftp_panel(sftp_entries, sftp_cwd, on_navigate),
                            ))
                            // Future tabs:
                            // .pane(TabPane::new("History", render_history_panel()))
                            // .pane(TabPane::new("Snippets", render_snippets_panel()))
                            .h_full(),
                    ),
            )
        })
}
