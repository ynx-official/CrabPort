// Submodules — method groups split out of this file. Each file holds an
// `impl CrabportApp { ... }` block; the methods are reachable on
// `CrabportApp` because all `impl` blocks for the same type compose.
pub mod connection;
pub mod context;
pub mod hosts;
pub mod snippets;
pub mod tabs;
pub mod tunnels;

pub use context::AppCtx;

use std::collections::HashMap;
use std::sync::Arc;

use gpui::*;
use rust_i18n::t;

use crate::app_state::AppState;
use crate::color::*;
use crate::components::context_menu::ContextMenuController;
use crate::components::dialog::AlertController;
use crate::components::notification::{NotificationController, NotificationPosition};
use crate::layouts::command_palette::{CommandView, ConnectionType};
use crate::layouts::sidebar::render_sidebar;
use crate::views::hosts::ConnectionFormState;
use crate::views::hosts::ConnectionHost;
use crate::views::terminal::TerminalView;
use crabport_core::credential::HostKind as CoreHostKind;

// ---- CrabPortTab trait ----

pub trait CrabPortTab: 'static {
    fn close(&mut self);
}

// ---- App ----

actions!(app, [ToggleCommand, TerminalTab, TerminalShiftTab]);

#[derive(Clone, Debug)]
pub struct Tab {
    pub id: u64,
    pub title: String,
    pub kind: TabKind,
    pub is_remote: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TabKind {
    Home,
    Terminal,
}

pub struct CrabportApp {
    pub sidebar_item: SidebarItem,
    pub tabs: Vec<Tab>,
    pub active_tab_id: u64,
    pub hovered_tab_id: Option<u64>,
    pub next_tab_id: u64,
    pub terminal_views: HashMap<u64, Entity<TerminalView>>,
    pub hosts: Vec<ConnectionHost>,
    pub connection_form: Option<ConnectionFormState>,
    pub command_palette: Entity<CommandView>,
    pub sftp_panel: Entity<crate::views::panel::sftp::SftpPanel>,
    pub snippets_panel: Entity<crate::views::panel::snippets_panel::SnippetsPanel>,
    pub history_panel: Entity<crate::views::panel::history_command_panel::HistoryCommandPanel>,
    /// Tunnels side panel (borrowed tunnels reusing the active tab's SSH
    /// connection). Only useful for SSH tabs.
    pub tunnels_panel: Entity<crate::views::panel::tunnels_panel::TunnelsPanel>,
    /// Active index of the right-hand panel's tab strip (SFTP / History /
    /// Snippets). Driven by `Tabs::on_change` in `render_panel`.
    pub panel_active_tab: usize,
    pub hosts_view: Entity<crate::views::hosts::HostsView>,
    /// Snippets management sidebar view (right-click edit/delete).
    pub snippets_view: Entity<crate::views::snippets::SnippetsView>,
    /// Tunnels management sidebar view (create/start/stop/edit/delete).
    pub tunnels_view: Entity<crate::views::tunnels::TunnelsView>,
    /// Global alert dialog host. Rendered at the app root so it overlays the
    /// entire window regardless of which view is active. Triggered via
    /// `alert_controller.update(cx, |c, cx| c.show(state, cx))`.
    pub alert_controller: Entity<AlertController>,
    /// Global context menu host. Triggered via
    /// `context_menu.update(cx, |c, cx| c.show(state, cx))`.
    pub context_menu: Entity<ContextMenuController>,
    /// Global toast notification host. Rendered at the app root so toasts
    /// overlay the entire window regardless of which view is active.
    /// Triggered via
    /// `notification_controller.update(cx, |c, cx| c.show(notification, cx))`.
    pub notification_controller: Entity<NotificationController>,
    /// Central registry of all tunnels (stopped + running). Single source
    /// of truth for the Tunnels view; mutations fire `cx.notify()` via the
    /// `on_change` callback wired at construction.
    pub tunnels: Arc<crate::views::tunnels::TunnelRegistry>,
    /// Tunnel form window state (singleton dialog for creating/editing a
    /// tunnel config). `None` when the dialog is closed.
    pub tunnel_form: Option<crate::views::tunnels::TunnelFormState>,
    /// Snippet form window state (singleton dialog for creating/editing a
    /// snippet). `None` when the dialog is closed.
    pub snippet_form: Option<crate::views::snippets::SnippetFormState>,
    /// Shared context bundle (app handle + global controllers + tunnels).
    /// Cheap to clone; handed out to child views / layout functions / async
    /// tasks instead of passing each controller separately.
    pub app_ctx: AppCtx,
    wired: bool,
    /// Tab id that currently holds focus. Used to focus the terminal only on
    /// actual tab switches instead of every render (which would steal focus
    /// from overlays like SFTP/command palette/connection form).
    last_focused_tab_id: Option<u64>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SidebarItem {
    Sessions,
    Tunnels,
    Snippets,
    History,
}

impl SidebarItem {
    pub fn label(&self) -> SharedString {
        match self {
            SidebarItem::Sessions => t!("sidebar.sessions").into(),
            SidebarItem::Tunnels => t!("sidebar.tunnels").into(),
            SidebarItem::Snippets => t!("sidebar.snippets").into(),
            SidebarItem::History => t!("sidebar.history").into(),
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            SidebarItem::Sessions => "icons/monitor-cloud.svg",
            SidebarItem::Tunnels => "icons/waypoints.svg",
            SidebarItem::Snippets => "icons/braces.svg",
            SidebarItem::History => "icons/clock.svg",
        }
    }

    pub fn all() -> [SidebarItem; 4] {
        [
            SidebarItem::Sessions,
            SidebarItem::Tunnels,
            SidebarItem::Snippets,
            SidebarItem::History,
        ]
    }
}

impl CrabportApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let home_tab = Tab {
            id: 0,
            title: "Home".into(),
            kind: TabKind::Home,
            is_remote: false,
        };

        let command_palette = cx.new(|cx| CommandView::new(window, cx));
        let sftp_panel = cx.new(|_cx| crate::views::panel::sftp::SftpPanel::new());
        let snippets_panel =
            cx.new(|_cx| crate::views::panel::snippets_panel::SnippetsPanel::new());
        let history_panel =
            cx.new(|_cx| crate::views::panel::history_command_panel::HistoryCommandPanel::new());
        let tunnels_panel = cx.new(|_cx| crate::views::panel::tunnels_panel::TunnelsPanel::new());
        let app_entity = cx.entity();
        let hosts_view = cx.new(|_cx| crate::views::hosts::HostsView::new(app_entity.clone()));
        let snippets_view =
            cx.new(|_cx| crate::views::snippets::SnippetsView::new(app_entity.clone()));
        let tunnels_view =
            cx.new(|_cx| crate::views::tunnels::TunnelsView::new(app_entity.clone()));
        let alert_controller = cx.new(|_cx| AlertController::new());
        let context_menu = cx.new(|_cx| ContextMenuController::new());
        let notification_controller =
            cx.new(|_cx| NotificationController::new(NotificationPosition::BottomRight));

        // Tunnel registry: a plain mutex-guarded list of tunnel configs +
        // their optional runtime state. `CrabportApp` calls `cx.notify()`
        // after each mutation (start/stop/add/remove) since those run in
        // GPUI contexts. The registry itself is context-free.
        let tunnels = Arc::new(crate::views::tunnels::TunnelRegistry::new());

        // Shared context bundle: app handle + global controllers + tunnels.
        // Built once here and stored on the app; cloned freely by child views
        // and layout functions instead of threading each controller through
        // `render_content`'s parameter list.
        let app_ctx = AppCtx {
            app: app_entity,
            alert: alert_controller.clone(),
            context_menu: context_menu.clone(),
            notifications: notification_controller.clone(),
            tunnels: tunnels.clone(),
        };

        // Read persisted data through the shared global store. The global
        // is initialized in `main` before any window is opened.
        let store = AppState::store(cx);
        let hosts: Vec<ConnectionHost> = store
            .lock()
            .hosts()
            .unwrap_or_default()
            .into_iter()
            .map(|h| ConnectionHost {
                id: h.id,
                name: h.name,
                host: h.host,
                port: h.port,
                username: h.username,
                kind: match h.kind {
                    CoreHostKind::Ssh => crate::views::hosts::ConnectionKind::SSH,
                    CoreHostKind::Telnet => crate::views::hosts::ConnectionKind::Telnet,
                    CoreHostKind::Serial => crate::views::hosts::ConnectionKind::Serial,
                },
                credential_id: h.credential_id,
                last_login: h.last_login,
                favorite: h.favorite,
                proxy_id: h.proxy_id,
            })
            .collect();

        // Load persisted tunnel configs from the store. Tunnels start in the
        // stopped state — the user starts them explicitly from the Tunnels
        // view or a terminal panel.
        let tunnel_configs = store.lock().tunnels().unwrap_or_default();
        tunnels.set_configs(tunnel_configs);

        Self {
            sidebar_item: SidebarItem::Sessions,
            tabs: vec![home_tab],
            active_tab_id: 0,
            hovered_tab_id: None,
            next_tab_id: 1,
            terminal_views: HashMap::new(),
            hosts,
            connection_form: None,
            command_palette,
            sftp_panel,
            snippets_panel,
            history_panel,
            tunnels_panel,
            panel_active_tab: 0,
            hosts_view,
            snippets_view,
            tunnels_view,
            alert_controller,
            context_menu,
            notification_controller,
            tunnels,
            tunnel_form: None,
            snippet_form: None,
            app_ctx,
            wired: false,
            last_focused_tab_id: None,
        }
    }

    /// Wire cross-entity callbacks. Called once after construction.
    pub fn wire(&mut self, cx: &mut Context<Self>) {
        if self.wired {
            return;
        }
        self.wired = true;

        let cmd = self.command_palette.clone();
        let app = cx.entity().clone();

        // ---- Command palette callbacks ----
        let cmd_for_close = cmd.clone();
        let cmd_for_new = cmd.clone();
        let app_for_cmd = app.clone();
        self.command_palette.update(cx, move |cmd, _cx| {
            cmd.set_on_close({
                let c = cmd_for_close.clone();
                move |_, cx| {
                    c.update(cx, |cmd, cx| cmd.close(cx));
                }
            });
            cmd.set_on_new_connection({
                let c = cmd_for_new.clone();
                let a = app_for_cmd.clone();
                move |ct, w, cx| {
                    match ct {
                        ConnectionType::LocalTerminal => {
                            a.update(cx, |app, cx| {
                                app.add_tab(cx);
                            });
                        }
                        _ => {
                            a.update(cx, |app, _cx| {
                                app.activate_tab(0);
                                app.sidebar_item = SidebarItem::Sessions;
                            });
                            a.update(cx, |app, cx| {
                                app.open_connection_form(w, cx);
                            });
                        }
                    }
                    c.update(cx, |cmd, cx| cmd.close(cx));
                }
            });
            cmd.set_on_select_host({
                let c = cmd_for_new.clone();
                let a = app_for_cmd.clone();
                move |idx, _w, cx| {
                    a.update(cx, |app, cx| {
                        let host_id = app.hosts.get(idx).map(|h| h.id).unwrap_or(-1);
                        if host_id >= 0 {
                            app.connect_to_host(host_id, cx);
                        }
                    });
                    c.update(cx, |cmd, cx| cmd.close(cx));
                }
            });
        });
    }

    // -- Helpers --
}

impl Render for CrabportApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let handle = cx.entity().clone();
        let show_sidebar = self.is_home_active();

        // Host data for command palette (sorted by favorite desc, last_login desc, limited to 5)
        let mut sorted_hosts: Vec<ConnectionHost> = self.hosts.clone();
        sorted_hosts.sort_by(|a, b| {
            b.favorite
                .cmp(&a.favorite)
                .then_with(|| b.last_login.cmp(&a.last_login))
        });
        self.command_palette.update(cx, |cmd, _cx| {
            cmd.set_hosts(sorted_hosts);
        });

        // ---- Content view ----
        // Pre-read tunnel state here (in the render method, where `self` is
        // already borrowed) rather than via `handle.read_with` inside
        // `render_content` — that would be a nested read of `CrabportApp`
        // and panic ("cannot read while it is already being updated").
        // Same pattern as `panel_active_tab`.
        let tunnel_list = self.tunnels.list();
        let tunnel_form_state = self.tunnel_form.clone();
        let snippet_form_state = self.snippet_form.clone();

        let content = crate::layouts::content::render_content(
            self.sidebar_item,
            &handle,
            &self.tabs,
            self.active_tab_id,
            &self.terminal_views,
            &self.hosts,
            self.connection_form.as_ref(),
            &self.sftp_panel,
            &self.snippets_panel,
            &self.history_panel,
            &self.tunnels_panel,
            self.panel_active_tab,
            &self.hosts_view,
            &self.snippets_view,
            &self.tunnels_view,
            tunnel_list,
            tunnel_form_state,
            snippet_form_state,
            &self.alert_controller,
            &self.context_menu,
            _window,
            cx,
        );

        // Focus the active terminal tab only when the active tab actually
        // changes — not on every render. Otherwise we'd steal focus from the
        // SFTP panel, command palette, connection form, etc.
        if self.last_focused_tab_id != Some(self.active_tab_id) {
            let active = self.tabs.iter().find(|t| t.id == self.active_tab_id);
            if let Some(tab) = active
                && tab.kind == TabKind::Terminal
                && let Some(entity) = self.terminal_views.get(&tab.id)
            {
                let fh = entity.read_with(cx, |view, cx| view.focus_handle(cx));
                _window.focus(&fh);
            }
            self.last_focused_tab_id = Some(self.active_tab_id);
        }

        // ---- Root ----
        div()
            .size_full()
            .bg(rgb(BG_BASE))
            .flex()
            .flex_row()
            .key_context("App")
            .on_action(cx.listener(Self::toggle_command))
            // -- Sidebar --
            .child(render_sidebar(self.sidebar_item, show_sidebar, &handle))
            .child(content)
            // -- Command palette --
            .child(self.command_palette.clone())
            // -- Global alert dialog (host-key prompts, etc.) --
            .child(self.alert_controller.clone())
            // -- Global context menu --
            .child(self.context_menu.clone())
            // -- Global toast notifications --
            .child(self.notification_controller.clone())
    }
}

// ---------------------------------------------------------------------------
// Main window construction
// ---------------------------------------------------------------------------

/// Open the main terminal window.
///
/// This is the heavy window — owns the `CrabportApp` root view (tabs,
/// terminals, SFTP, command palette, etc.). Constructed directly here rather
/// than going through `crate::windows::focus_or_open`, because the main
/// window is neither singleton-managed nor lightweight.
///
/// Cross-window sharing still happens via `App`-level globals: `AppState`
/// for the persistent store, `WindowRegistry` for singleton auxiliary
/// windows.
pub fn open_main_window(cx: &mut App) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(1200.0), px(800.0)), cx)),
        #[cfg(target_os = "macos")]
        titlebar: Some(TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: Some(point(px(12.0), px(14.0))),
            ..Default::default()
        }),
        window_min_size: Some(Size {
            width: px(560.0),
            height: px(340.0),
        }),
        ..Default::default()
    };

    cx.open_window(options, |_window, cx| {
        cx.new(|cx| {
            let app = cx.new(|cx| CrabportApp::new(_window, cx));
            app.update(cx, |app, cx| app.wire(cx));
            gpui_component::Root::new(app, _window, cx)
        })
    })
    .expect("Failed to open main window");
}
