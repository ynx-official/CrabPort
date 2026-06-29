use gpui::*;
use gpui_animation::{animation::TransitionExt, transition::general::EaseInOutCubic};

use crate::color::*;
use crate::components::tabs::{TabPane, Tabs};
use crate::views::panel::sftp::SftpPanel;

const PANEL_WIDTH: f32 = 220.0;

pub fn render_panel(
    show: bool,
    active_tab: usize,
    has_sftp: bool,
    sftp_panel: Entity<SftpPanel>,
) -> impl IntoElement {
    let visible = show && has_sftp;

    // The inner content is always rendered so the width transition has
    // something to reveal/crop. When `visible` is false the outer div
    // animates to w_0 and `overflow_hidden` clips the content away —
    // giving a smooth shrink instead of the content vanishing instantly.
    //
    // `flex_shrink_0` is essential: this panel sits in a `flex_row` next to
    // the terminal view (which requests `size_full` = 100% width). Without
    // it, flex would shrink the panel below its target 220px, but the inner
    // div is pinned to `w(PANEL_WIDTH)` so the scrollbar would render at
    // 220px and get clipped by the shrunk outer box — making the scrollbar
    // invisible.
    div()
        .id("panel-sidebar")
        .h_full()
        .overflow_hidden()
        .flex_shrink_0()
        .w_0()
        .with_transition("panel-sidebar-width")
        .transition_when_else(
            visible,
            std::time::Duration::from_millis(500),
            EaseInOutCubic,
            |el| el.w(px(PANEL_WIDTH)),
            |el| el.w_0(),
        )
        .child(
            div()
                .h_full()
                .w(px(PANEL_WIDTH))
                .border_l_1()
                .border_color(rgb(BORDER))
                .bg(rgb(BG_SIDEBAR))
                .child(
                    Tabs::new("panel-tabs")
                        .ctrl_style(|s| s.bg(rgb(BG_SIDEBAR)).rounded_none().p_0())
                        .active(active_tab)
                        .pane(TabPane::new("SFTP", sftp_panel))
                        // Future tabs:
                        // .pane(TabPane::new("History", render_history_panel()))
                        // .pane(TabPane::new("Snippets", render_snippets_panel()))
                        .h_full(),
                ),
        )
}
