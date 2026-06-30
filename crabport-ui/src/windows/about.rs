//! "About CrabPort" window.
//!
//! Shows the application name, version, and build metadata. Kept deliberately
//! simple — no settings, no persistence. Opened via [`AboutWindow::open`] or
//! the global [`focus_or_open`] helper.

use gpui::*;
use rust_i18n::t;

use crate::color::*;

/// Root view for the About window.
pub struct AboutWindow {
    /// Cached app version string. Read once at construction time so renders
    /// are pure and don't re-query cargo metadata.
    version: SharedString,
}

impl AboutWindow {
    /// Open the About window (or no-op if one already exists — callers
    /// should normally go through [`crate::windows::focus_or_open`] for the
    /// singleton check).
    pub fn open(cx: &mut App) -> WindowHandle<gpui_component::Root> {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(420.0), px(320.0)), cx)),
            titlebar: Some(TitlebarOptions {
                title: Some(t!("window.about.title").to_string().into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.0), px(14.0))),
                ..Default::default()
            }),
            window_min_size: Some(Size {
                width: px(360.0),
                height: px(280.0),
            }),
            ..Default::default()
        };

        let version = env!("CARGO_PKG_VERSION").into();

        cx.open_window(options, |window, cx| {
            cx.new(|cx| {
                let view = cx.new(|_cx| AboutWindow { version });
                gpui_component::Root::new(view, window, cx)
            })
        })
        .expect("Failed to open About window")
    }
}

impl Render for AboutWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(BG_BASE))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_2xl()
                    .text_color(rgb(TEXT_PRIMARY))
                    .child("CrabPort"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(TEXT_MUTED))
                    .child(format!("v{}", self.version)),
            )
    }
}
