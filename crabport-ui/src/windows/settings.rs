//! Settings window.
//!
//! Currently a skeleton — renders a placeholder layout. Real settings panels
//! (appearance, terminal defaults, SSH defaults, data export) will be added
//! incrementally in follow-up commits.

use gpui::*;
use rust_i18n::t;

use crate::color::*;

/// Root view for the Settings window.
pub struct SettingsWindow;

impl SettingsWindow {
    /// Open the Settings window (or no-op if one already exists — callers
    /// should normally go through [`crate::windows::focus_or_open`] for the
    /// singleton check).
    pub fn open(cx: &mut App) -> WindowHandle<gpui_component::Root> {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(720.0), px(560.0)), cx)),
            titlebar: Some(TitlebarOptions {
                title: Some(t!("window.settings.title").to_string().into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.0), px(14.0))),
                ..Default::default()
            }),
            window_min_size: Some(Size {
                width: px(480.0),
                height: px(400.0),
            }),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            cx.new(|cx| {
                let view = cx.new(|_cx| SettingsWindow);
                gpui_component::Root::new(view, window, cx)
            })
        })
        .expect("Failed to open Settings window")
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(BG_BASE))
            .flex()
            .flex_col()
            .p_6()
            .gap_4()
            .child(
                div()
                    .text_xl()
                    .text_color(rgb(TEXT_PRIMARY))
                    .child(t!("window.settings.title").to_string()),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(TEXT_MUTED))
                    .child(t!("window.settings.placeholder").to_string()),
            )
    }
}
