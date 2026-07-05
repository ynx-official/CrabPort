//! Shared app context for cross-module access.
//!
//! [`AppCtx`] bundles the global controllers (alert / context-menu /
//! notifications / tunnels) and the owning `CrabportApp` entity handle into a
//! single clonable value. Child views and layout functions read whatever they
//! need from the context instead of receiving a long parameter list from
//! `CrabportApp`, which keeps `app.rs` thin.
//!
//! Construction happens once in [`CrabportApp::new`]; the context is stored on
//! the app as `pub app_ctx: AppCtx` and cloned freely (every field is
//! either an `Entity` or `Arc`, so clones are cheap).

use std::sync::Arc;

use gpui::*;

use crate::components::context_menu::ContextMenuController;
use crate::components::dialog::AlertController;
use crate::components::notification::NotificationController;
use crate::views::tunnels::TunnelRegistry;

/// Shared, cheaply-clonable bundle of the global controllers + app handle.
///
/// Every field is a GPUI `Entity` (refcounted handle) or an `Arc`, so cloning
/// this struct is essentially free and safe to hand out to child views /
/// layout functions / async tasks.
///
/// Named `AppCtx` (not `AppContext`) to avoid colliding with GPUI's
/// `AppContext` trait, which provides `cx.new(...)`.
#[derive(Clone)]
pub struct AppCtx {
    /// Owning `CrabportApp` entity. Use this to drive app-level mutations
    /// (`app.update(cx, |app, cx| ... )`) from outside the app module.
    pub app: Entity<crate::app::CrabportApp>,
    /// Global alert dialog host (host-key prompts, delete confirmations).
    pub alert: Entity<AlertController>,
    /// Global right-click context-menu host.
    pub context_menu: Entity<ContextMenuController>,
    /// Global toast notification host.
    pub notifications: Entity<NotificationController>,
    /// Central tunnel registry (stopped + running). Single source of truth
    /// for the Tunnels view and panel.
    pub tunnels: Arc<TunnelRegistry>,
}

impl AppCtx {
    /// Convenience accessor for the notification host, mirroring the helper
    /// methods on `CrabportApp` so call sites can stay terse.
    pub fn notify(
        &self,
        notification: crate::components::notification::Notification,
        cx: &mut App,
    ) {
        self.notifications
            .update(cx, |c, cx| c.show(notification, cx));
    }
}
