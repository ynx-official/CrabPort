//! Settings window.
//!
//! Renders a sidebar (General / Appearance) on the left and a scrollable
//! content pane on the right. Every control reads from and writes to the
//! process-wide [`crabport_core::config::CONFIG`] `LazyLock`, so changes are
//! persisted to `config.toml` immediately and visible to every other window
//! in the process.

use gpui::*;
use gpui_component::label::Label;
use rust_i18n::t;

use crabport_core::config;

use crate::color::*;
use crate::components::button::Button;
use crate::components::dropdown::Dropdown;

// ---------------------------------------------------------------------------
// Tab enum
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsTab {
    General,
    Appearance,
}

impl SettingsTab {
    fn all() -> [SettingsTab; 2] {
        [SettingsTab::General, SettingsTab::Appearance]
    }

    fn label(self) -> SharedString {
        match self {
            SettingsTab::General => t!("window.settings.tab.general").into(),
            SettingsTab::Appearance => t!("window.settings.tab.appearance").into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Root view
// ---------------------------------------------------------------------------

/// Root view for the Settings window.
pub struct SettingsWindow {
    tab: SettingsTab,
    // Dropdown open states (Dropdown is uncontrolled — caller manages it).
    locale_dropdown_open: bool,
}

impl SettingsWindow {
    /// Open the Settings window (or no-op if one already exists — callers
    /// should normally go through [`crate::windows::focus_or_open`] for the
    /// singleton check).
    pub fn open(cx: &mut App) -> WindowHandle<gpui_component::Root> {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::centered(size(px(820.0), px(600.0)), cx)),
            titlebar: Some(TitlebarOptions {
                title: Some(t!("window.settings.title").to_string().into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.0), px(14.0))),
                ..Default::default()
            }),
            window_min_size: Some(Size {
                width: px(560.0),
                height: px(440.0),
            }),
            ..Default::default()
        };

        cx.open_window(options, |window, cx| {
            cx.new(|cx| {
                let view = cx.new(|cx| SettingsWindow::new(window, cx));
                gpui_component::Root::new(view, window, cx)
            })
        })
        .expect("Failed to open Settings window")
    }

    fn new(_window: &mut Window, _cx: &mut Context<Self>) -> Self {
        Self {
            tab: SettingsTab::General,
            locale_dropdown_open: false,
        }
    }

    // -------------------------------------------------------------------
    // Sidebar
    // -------------------------------------------------------------------

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let handle = cx.entity().clone();
        div()
            .h_full()
            .w(px(180.0))
            .flex_shrink_0()
            .border_r_1()
            .border_color(rgb(BORDER))
            .bg(rgb(BG_SIDEBAR))
            .flex()
            .flex_col()
            .pt_11()
            .px_2()
            .gap_2()
            .children(SettingsTab::all().map(|item| {
                let is_selected = item == self.tab;
                let h = handle.clone();
                Button::new(ElementId::Name(format!("settings-tab-{:?}", item).into()))
                    .tab()
                    .selected(is_selected)
                    .child(item.label())
                    .on_click(move |_e, _w, cx| {
                        h.update(cx, |view, _| {
                            view.tab = item;
                        });
                    })
                    .h_9()
                    .border_0()
                    .px_2()
                    .text_sm()
                    .justify_start()
            }))
    }

    // -------------------------------------------------------------------
    // Section helpers
    // -------------------------------------------------------------------

    fn section_header(text: impl Into<SharedString>) -> impl IntoElement {
        div()
            .text_sm()
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(TEXT_PRIMARY))
            .child(text.into())
    }

    fn section_desc(text: impl Into<SharedString>) -> impl IntoElement {
        div()
            .text_xs()
            .text_color(rgb(TEXT_MUTED))
            .child(text.into())
    }

    // -------------------------------------------------------------------
    // General pane
    // -------------------------------------------------------------------

    fn render_general_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let store_path = crabport_core::store::default_data_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "(unknown)".to_string());
        let handle = cx.entity().clone();

        div()
            .size_full()
            .flex()
            .flex_col()
            .p_6()
            .gap_6()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Self::section_header(t!(
                        "window.settings.general.section_data"
                    )))
                    .child(Self::section_desc(t!(
                        "window.settings.general.open_data_dir_desc"
                    )))
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(TEXT_MUTED))
                            .child(Label::new(store_path)),
                    )
                    .child(
                        Button::new("settings-open-data-dir")
                            .child(t!("window.settings.general.open_data_dir").to_string())
                            .w_auto()
                            .centered(true)
                            .on_click(move |_e, _w, cx| {
                                let _ = crabport_core::store::default_data_dir().map(|p| {
                                    let _ = open_path(&p, cx);
                                });
                            }),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Self::section_header(t!(
                        "window.settings.general.reset_config"
                    )))
                    .child(Self::section_desc(t!(
                        "window.settings.general.reset_config_desc"
                    )))
                    .child({
                        let h = handle.clone();
                        Button::new("settings-reset-config")
                            .child(t!("window.settings.general.reset_config").to_string())
                            .w_auto()
                            .centered(true)
                            .on_click(move |_e, _w, cx| {
                                let _ = config::update(|cfg| {
                                    cfg.appearance = Default::default();
                                });
                                h.update(cx, |_, cx| {
                                    cx.notify();
                                });
                            })
                    }),
            )
    }

    // -------------------------------------------------------------------
    // Appearance pane
    // -------------------------------------------------------------------

    fn render_appearance_pane(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let handle = cx.entity().clone();
        let locale_idx = if config::snapshot().appearance.locale == "zh-CN" {
            1
        } else {
            0
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .p_6()
            .gap_6()
            // --- Language ---
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(Self::section_header(t!(
                        "window.settings.appearance.section_language"
                    )))
                    .child(
                        div().w(px(240.0)).child(
                            Dropdown::new("settings-locale")
                                .item(t!("window.settings.appearance.language_en"))
                                .item(t!("window.settings.appearance.language_zh_cn"))
                                .selected(locale_idx)
                                .is_open(self.locale_dropdown_open)
                                .on_toggle({
                                    let h = handle.clone();
                                    move |_w, cx| {
                                        h.update(cx, |view, cx| {
                                            view.locale_dropdown_open = !view.locale_dropdown_open;
                                            cx.notify();
                                        });
                                    }
                                })
                                .on_change(move |idx, _w, cx| {
                                    let locale = if idx == 1 { "zh-CN" } else { "en" };
                                    let _ = config::update(|cfg| {
                                        cfg.appearance.locale = locale.to_string();
                                    });
                                    crate::set_locale(locale);
                                    handle.update(cx, |view, cx| {
                                        view.locale_dropdown_open = false;
                                        cx.notify();
                                    });
                                }),
                        ),
                    ),
            )
    }
}

// ---------------------------------------------------------------------------
// Render
// ---------------------------------------------------------------------------

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content: AnyElement = match self.tab {
            SettingsTab::General => self.render_general_pane(cx).into_any_element(),
            SettingsTab::Appearance => self.render_appearance_pane(cx).into_any_element(),
        };

        div()
            .size_full()
            .bg(rgb(BG_BASE))
            .flex()
            .flex_row()
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .overflow_hidden()
                    .child(content),
            )
    }
}

// ---------------------------------------------------------------------------
// open_path helper — best-effort cross-platform "reveal in Finder/Explorer"
// ---------------------------------------------------------------------------

fn open_path(path: &std::path::Path, _cx: &mut App) -> Result<(), ()> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(path)
            .spawn()
            .map_err(|_| ())?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(path)
            .spawn()
            .map_err(|_| ())?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map_err(|_| ())?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err(())
}
