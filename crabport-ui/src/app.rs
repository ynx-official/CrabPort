use gpui::*;
use gpui_animation::{animation::TransitionExt, transition::general::EaseInOutCubic};
use rust_i18n::t;

use crate::color::*;
use crate::layouts::content::render_content;
use crate::layouts::sidebar::render_sidebar;

actions!(app, [ToggleCommand]);

#[derive(Clone, Debug)]
pub struct Tab {
    pub id: u64,
    pub title: String,
    pub kind: TabKind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabKind {
    Home,
    Ssh,
}

pub struct CrabportApp {
    pub sidebar_item: SidebarItem,
    pub tabs: Vec<Tab>,
    pub active_tab_id: u64,
    pub next_tab_id: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SidebarItem {
    Hosts,
    Credentials,
    Snippets,
    History,
}

impl SidebarItem {
    pub fn label(&self) -> SharedString {
        match self {
            SidebarItem::Hosts => t!("sidebar.hosts").into(),
            SidebarItem::Credentials => t!("sidebar.credentials").into(),
            SidebarItem::Snippets => t!("sidebar.snippets").into(),
            SidebarItem::History => t!("sidebar.history").into(),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            SidebarItem::Hosts => "icons/server.svg",
            SidebarItem::Credentials => "icons/key.svg",
            SidebarItem::Snippets => "icons/braces.svg",
            SidebarItem::History => "icons/clock.svg",
        }
    }

    pub fn all() -> [SidebarItem; 4] {
        [
            SidebarItem::Hosts,
            SidebarItem::Credentials,
            SidebarItem::Snippets,
            SidebarItem::History,
        ]
    }
}

impl CrabportApp {
    pub fn new() -> Self {
        rust_i18n::set_locale("zh-CN");
        let home_tab = Tab {
            id: 0,
            title: "Home".into(),
            kind: TabKind::Home,
        };
        Self {
            sidebar_item: SidebarItem::Hosts,
            tabs: vec![home_tab],
            active_tab_id: 0,
            next_tab_id: 1,
        }
    }

    pub fn is_home_active(&self) -> bool {
        self.tabs
            .iter()
            .find(|t| t.id == self.active_tab_id)
            .map(|t| t.kind == TabKind::Home)
            .unwrap_or(false)
    }

    pub fn add_tab(&mut self) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        self.tabs.push(Tab {
            id,
            title: format!("SSH-{}", id),
            kind: TabKind::Ssh,
        });
        self.active_tab_id = id;
    }

    pub fn activate_tab(&mut self, id: u64) {
        if self.tabs.iter().any(|t| t.id == id) {
            self.active_tab_id = id;
        }
    }
}

impl Render for CrabportApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let handle = cx.entity().clone();
        let show_sidebar = self.is_home_active();

        div()
            .size_full()
            .bg(rgb(BG_BASE))
            .flex()
            .flex_row()
            .key_context("App")
            .child(
                div()
                    .id("sidebar-container")
                    .h_full()
                    .bg(rgb(BG_SIDEBAR))
                    .overflow_x_hidden()
                    .with_transition("sidebar-container")
                    .transition_when(
                        show_sidebar,
                        std::time::Duration::from_millis(300),
                        EaseInOutCubic,
                        |el| el.w(px(200.0)),
                    )
                    .transition_when(
                        !show_sidebar,
                        std::time::Duration::from_millis(300),
                        EaseInOutCubic,
                        |el| el.w_0(),
                    )
                    .child(render_sidebar(self.sidebar_item, &handle)),
            )
            .child(render_content(
                self.sidebar_item,
                &handle,
                &self.tabs,
                self.active_tab_id,
            ))
    }
}
