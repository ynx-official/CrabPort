use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_animation::{animation::TransitionExt, transition::general::EaseInOutCubic};

use crate::color::*;
use crate::components::tabs::{TabPane, Tabs};
use crate::views::panel::PanelKind;
use crate::views::panel::history_command_panel::HistoryCommandPanel;
use crate::views::panel::sftp::SftpPanel;
use crate::views::panel::snippets_panel::SnippetsPanel;

pub const PANEL_WIDTH: f32 = 220.0;

/// Right-hand panel capability set for the active terminal backend. Each
/// flag maps to a `CrabPortTerminal` capability method; the panel renders
/// only the panes whose flag is `true`, so a Telnet tab shows History +
/// Snippets (no SFTP / Tunnels) while an SSH tab shows all four.
pub struct PanelCaps {
    pub sftp: bool,
    pub history: bool,
    pub snippets: bool,
    pub tunnels: bool,
}

pub fn render_panel(
    show: bool,
    active_kind: PanelKind,
    caps: PanelCaps,
    sftp_panel: Entity<SftpPanel>,
    snippets_panel: Entity<SnippetsPanel>,
    history_panel: Entity<HistoryCommandPanel>,
    tunnels_panel: Entity<crate::views::panel::tunnels_panel::TunnelsPanel>,
    on_change: Option<std::rc::Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
) -> impl IntoElement {
    // Fixed pane order so the positional index is stable for a given
    // capability set. SFTP is first (only on SSH), then History, Snippets,
    // and Tunnels (only on SSH).
    let any_visible = caps.sftp || caps.history || caps.snippets || caps.tunnels;
    let visible = show && any_visible;

    // Derive the positional index the active `PanelKind` maps to in the
    // filtered pane list. Falls back to 0 (first visible pane) when the
    // stored kind isn't available for this backend — e.g. switching from
    // an SSH tab (Tunnels selected) to a Telnet tab.
    let mut kinds: Vec<PanelKind> = Vec::with_capacity(4);
    if caps.sftp {
        kinds.push(PanelKind::Sftp);
    }
    if caps.history {
        kinds.push(PanelKind::History);
    }
    if caps.snippets {
        kinds.push(PanelKind::Snippets);
    }
    if caps.tunnels {
        kinds.push(PanelKind::Tunnels);
    }
    let active_idx = kinds
        .iter()
        .position(|k| *k == active_kind)
        .unwrap_or(0)
        .min(kinds.len().saturating_sub(1));

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
    // The Tabs element ID encodes the capability signature so that
    // switching between backends with different pane counts (SSH = 4 panes
    // vs Telnet/local = 2 panes) gets a fresh set of gpui-animation
    // transition IDs. Without this, the track / panel transition state
    // cached for the old pane count persists and drives the layout to a
    // wrong position (e.g. track stuck at left(-3) from a 4-pane render
    // while only 2 panes exist), causing the panel to render clipped /
    // half-width.
    let tabs_id = SharedString::from(format!(
        "panel-tabs-{}{}{}{}",
        caps.sftp as u8, caps.history as u8, caps.snippets as u8, caps.tunnels as u8
    ));
    let mut tabs = Tabs::new(tabs_id)
        .ctrl_style(|s| s.rounded_none())
        .active(active_idx)
        .when_some(on_change, |tabs, cb| {
            tabs.on_change(move |idx, w, cx| cb(idx, w, cx))
        });
    // Add panes in the same fixed order as `kinds` above so the positional
    // index stays consistent.
    if caps.sftp {
        tabs = tabs.pane(TabPane::new("", sftp_panel).icon("icons/folder.svg"));
    }
    if caps.history {
        tabs = tabs.pane(TabPane::new("", history_panel).icon("icons/clock.svg"));
    }
    if caps.snippets {
        tabs = tabs.pane(TabPane::new("", snippets_panel).icon("icons/braces.svg"));
    }
    if caps.tunnels {
        tabs = tabs.pane(TabPane::new("", tunnels_panel).icon("icons/waypoints.svg"));
    }

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
                .border_color(rgb(border()))
                .bg(rgb(bg_sidebar()))
                .child(tabs.h_full()),
        )
}
