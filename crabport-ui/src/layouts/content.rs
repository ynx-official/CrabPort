use gpui::*;

use crate::app::{SidebarItem, Tab, TabKind};
use crate::color::*;
use crate::layouts::tabbar::render_tab_bar;
use crate::views;

pub fn render_content(
    selected: SidebarItem,
    handle: &Entity<crate::app::CrabportApp>,
    tabs: &[Tab],
    active_tab_id: u64,
) -> Div {
    let active_tab = tabs.iter().find(|t| t.id == active_tab_id);

    let view: AnyElement = match active_tab.map(|t| t.kind) {
        Some(TabKind::Home) => match selected {
            SidebarItem::Hosts => views::hosts::render_hosts_view().into_any_element(),
            SidebarItem::Credentials => {
                views::credentials::render_credentials_view().into_any_element()
            }
            SidebarItem::Snippets => views::snippets::render_snippets_view().into_any_element(),
            SidebarItem::History => div()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .text_color(rgb(TEXT_MUTED))
                        .child(selected.label().to_string()),
                )
                .into_any_element(),
        },
        Some(TabKind::Ssh) => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(div().text_color(rgb(TEXT_MUTED)).child("Terminal"))
            .into_any_element(),
        None => div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(div().text_color(rgb(TEXT_MUTED)).child("No tab"))
            .into_any_element(),
    };

    div()
        .flex_1()
        .h_full()
        .bg(rgb(BG_BASE))
        .flex()
        .flex_col()
        .child(render_tab_bar(
            handle,
            tabs,
            active_tab_id,
            active_tab.map(|t| t.kind == TabKind::Home).unwrap_or(false),
        ))
        .child(view)
}
