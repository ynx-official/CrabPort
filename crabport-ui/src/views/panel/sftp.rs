use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_animation::{animation::TransitionExt, transition::general::Linear};
use gpui_component::input::InputState;
use gpui_component::scroll::ScrollableElement;
use rust_i18n::t;

use crate::color::*;
use crate::components::context_menu::{ContextMenuItem, ContextMenuState};
use crate::components::dialog::{AlertController, AlertSeverity, AlertState};
use crate::components::input::StyledInput;

/// SFTP panel view.
///
/// Holds its own `InputState` for the path input bar so the typed text
/// persists across renders. Entries / cwd / navigation callback are pushed
/// in from the active terminal's backend before each render via `set_state`.
pub struct SftpPanel {
    /// Path input state. Lazily initialized on the first `set_state` call
    /// (which receives a `&mut Window` required by `InputState::new`).
    path_input: Option<Entity<InputState>>,
    /// Current working directory, shown as the input's default value.
    cwd: Option<Arc<String>>,
    /// Current directory entries.
    entries: Arc<Vec<(String, bool)>>,
    /// Navigate callback — invoked with the typed path on Enter.
    on_navigate: Option<Rc<dyn Fn(String, &mut App)>>,
    /// The tab id whose state is currently reflected in the input.
    /// When the active tab changes we force-sync the input to the new
    /// backend's cwd instead of preserving the previous tab's text.
    active_tab_id: Option<u64>,
    /// Global context menu host. Held so the panel can open a right-click
    /// menu on entries ("Enter" for dirs, "Download" for files).
    context_menu: Option<Entity<crate::components::context_menu::ContextMenuController>>,
    /// Global alert dialog host, used for the delete-confirmation prompt.
    alert_controller: Option<Entity<AlertController>>,
    /// The entry name that triggered the currently-open context menu, if
    /// any. While set, that row stays highlighted in the hover color even
    /// though the mouse has moved to the overlay.
    context_menu_entry: Option<String>,
    /// The entry currently being hovered, if any. Used to drive the hover
    /// background transition (same pattern as HostsView).
    hovered_entry: Option<String>,
}

impl SftpPanel {
    pub fn new() -> Self {
        Self {
            path_input: None,
            cwd: None,
            entries: Arc::new(Vec::new()),
            on_navigate: None,
            active_tab_id: None,
            context_menu: None,
            alert_controller: None,
            context_menu_entry: None,
            hovered_entry: None,
        }
    }

    /// Update the SFTP state from the active terminal's backend.
    /// Called by the content layout each render.
    pub fn set_state(
        &mut self,
        entries: Arc<Vec<(String, bool)>>,
        cwd: Option<Arc<String>>,
        on_navigate: Option<Rc<dyn Fn(String, &mut App)>>,
        active_tab_id: u64,
        context_menu: Entity<crate::components::context_menu::ContextMenuController>,
        alert_controller: Entity<AlertController>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Did the active tab change since last render? This is the signal
        // for a context switch (e.g. user opened another SSH connection),
        // and we must force-sync the input to the new backend's cwd —
        // otherwise the input would keep showing the previous tab's path.
        let tab_changed = self.active_tab_id != Some(active_tab_id);

        // Lazily init the InputState now that we have a Window.
        if self.path_input.is_none() {
            let initial = cwd.as_ref().map(|s| s.as_str()).unwrap_or("/").to_string();
            let entity = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(initial)
                    .placeholder("/path/to/dir")
            });
            // Listen for Enter key to submit navigation.
            cx.subscribe(
                &entity,
                |this, input, event: &gpui_component::input::InputEvent, cx| {
                    if let gpui_component::input::InputEvent::PressEnter { .. } = event {
                        let path = input.read(cx).value().to_string();
                        if !path.is_empty() {
                            if let Some(ref cb) = this.on_navigate {
                                let cb = cb.clone();
                                cx.defer(move |cx| cb(path, cx));
                            }
                        }
                    }
                },
            )
            .detach();

            // On blur, discard any in-progress edit and snap the input back
            // to the backend's current cwd. This avoids stale text lingering
            // when the user clicks away mid-edit.
            let blur_handle = entity.read(cx).focus_handle(cx);
            cx.on_blur(&blur_handle, window, move |this, window, cx| {
                if let Some(ref input) = this.path_input {
                    let value = this
                        .cwd
                        .as_ref()
                        .map(|s| s.as_str().to_string())
                        .unwrap_or_else(|| "/".to_string());
                    input.update(cx, |state, cx| {
                        state.set_value(value, window, cx);
                    });
                }
            })
            .detach();

            self.path_input = Some(entity);
            self.cwd = cwd.clone();
            self.active_tab_id = Some(active_tab_id);
            self.context_menu = Some(context_menu);
            self.alert_controller = Some(alert_controller);
            return;
        }

        // If the backend's cwd changed (e.g. user double-clicked a folder),
        // sync the input to reflect it. Three cases:
        //   1. Tab switched → always overwrite (context switch).
        //   2. Same tab, cwd just arrived (None → Some) → overwrite. This
        //      happens right after connecting when the backend reports its
        //      initial cwd; the `cur == prev` guard below would fail because
        //      `prev` (the old None) serializes to "".
        //   3. Same tab, cwd changed (Some → Some) → only when the input
        //      still shows the previous cwd, so we don't clobber the user's
        //      in-progress edit.
        let cwd_changed = self.cwd.as_ref().map(|s| s.as_str()) != cwd.as_ref().map(|s| s.as_str());
        let cwd_just_arrived = self.cwd.is_none() && cwd.is_some();
        if tab_changed || cwd_changed {
            if let Some(ref input) = self.path_input {
                let should_overwrite = if tab_changed || cwd_just_arrived {
                    true
                } else {
                    let cur = input.read(cx).value().to_string();
                    let prev = self.cwd.as_ref().map(|s| s.as_str()).unwrap_or("");
                    cur == prev
                };
                if should_overwrite {
                    let value = cwd
                        .as_ref()
                        .map(|s| s.as_str().to_string())
                        .unwrap_or_else(|| "/".to_string());
                    input.update(cx, |state, cx| {
                        state.set_value(value, window, cx);
                    });
                }
            }
        }

        self.cwd = cwd;
        self.entries = entries;
        self.on_navigate = on_navigate;
        self.active_tab_id = Some(active_tab_id);
        self.context_menu = Some(context_menu);
        self.alert_controller = Some(alert_controller);
    }
}

impl Render for SftpPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        // Sort entries alphabetically, directories first
        let mut sorted: Vec<(String, bool)> = self.entries.iter().cloned().collect();
        sorted.sort_by(|a, b| match (a.0.as_str(), b.0.as_str()) {
            (".", _) => std::cmp::Ordering::Less,
            (_, ".") => std::cmp::Ordering::Greater,
            ("..", _) => std::cmp::Ordering::Less,
            (_, "..") => std::cmp::Ordering::Greater,
            _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
        });

        // Prepend .. entry
        let mut all_entries: Vec<(String, bool)> = vec![("..".into(), true)];
        all_entries.extend(sorted);

        let cwd = self.cwd.clone();
        let on_navigate = self.on_navigate.clone();
        let path_input = self.path_input.clone();
        let context_menu = self.context_menu.clone();
        let alert_controller = self.alert_controller.clone();

        // If the global context menu is no longer active, clear the
        // "menu-triggering entry" highlight.
        let menu_active = self
            .context_menu
            .as_ref()
            .map(|cm| cm.read_with(_cx, |c, _| c.is_active()))
            .unwrap_or(false);
        if !menu_active {
            self.context_menu_entry = None;
        }
        let context_menu_entry = self.context_menu_entry.clone();
        let hovered_entry = self.hovered_entry.clone();

        div()
            .h_full()
            .min_h_0()
            .overflow_hidden()
            .flex()
            .flex_col()
            .pt_1()
            .px_1()
            .when_some(path_input, |el, input| {
                el.child(
                    div().mb_1().child(
                        StyledInput::new("sftp-path", input).xsmall().prefix(
                            svg()
                                .path("icons/folder.svg")
                                .size(px(12.0))
                                .text_color(rgb(TEXT_MUTED)),
                        ),
                    ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .overflow_y_scrollbar()
                    .children(all_entries.iter().map(|(name, is_dir)| {
                        let icon_path = if *is_dir {
                            "icons/folder.svg"
                        } else {
                            "icons/file.svg"
                        };

                        // Build target path for navigation
                        let cwd_ref = cwd.as_ref().map(|s| s.as_str()).unwrap_or("/");
                        let target_path = if name == "." {
                            cwd_ref.to_string()
                        } else if name == ".." {
                            let mut parts: Vec<&str> =
                                cwd_ref.split('/').filter(|s| !s.is_empty()).collect();
                            parts.pop();
                            if parts.is_empty() {
                                "/".to_string()
                            } else {
                                format!("/{}", parts.join("/"))
                            }
                        } else if cwd_ref.ends_with('/') {
                            format!("{}{}", cwd_ref, name)
                        } else {
                            format!("{}/{}", cwd_ref, name)
                        };

                        let on_navigate = on_navigate.clone();
                        let context_menu = context_menu.clone();
                        let entity = _cx.entity().downgrade();
                        let is_hovered = hovered_entry.as_deref() == Some(name.as_str());
                        let force_highlight = context_menu_entry.as_deref() == Some(name.as_str());
                        let is_highlighted = is_hovered || force_highlight;
                        let row_id = ElementId::Name(format!("sftp-{}", name).into());
                        let row_id_for_transition = row_id.clone();

                        div()
                            .id(row_id.clone())
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_1()
                            .rounded(px(4.0))
                            .when(*is_dir, |el| {
                                el.on_mouse_down(MouseButton::Left, {
                                    let on_navigate = on_navigate.clone();
                                    let target = target_path.clone();
                                    move |event, _w, cx| {
                                        if event.click_count == 2 {
                                            if let Some(ref cb) = on_navigate {
                                                cb(target.clone(), cx);
                                            }
                                        }
                                    }
                                })
                            })
                            .on_mouse_down(MouseButton::Right, {
                                let name = name.clone();
                                let is_dir = *is_dir;
                                let target_path = target_path.clone();
                                let on_navigate = on_navigate.clone();
                                let entity = entity.clone();
                                let alert_controller = alert_controller.clone();
                                move |event, _w, cx| {
                                    let Some(ref cm) = context_menu else {
                                        return;
                                    };
                                    // Mark this entry as the menu-triggering
                                    // entry so it keeps the hover background.
                                    let _ = entity.update(cx, |view, cx| {
                                        view.context_menu_entry = Some(name.clone());
                                        cx.notify();
                                    });
                                    let pos = event.position;
                                    let mut items = if is_dir {
                                        vec![ContextMenuItem::new(t!("sftp.enter").to_string(), {
                                            let target = target_path.clone();
                                            let on_navigate = on_navigate.clone();
                                            move |_w, cx| {
                                                if let Some(ref cb) = on_navigate {
                                                    cb(target.clone(), cx);
                                                }
                                            }
                                        })]
                                    } else {
                                        vec![ContextMenuItem::new(
                                            t!("sftp.download").to_string(),
                                            {
                                                let name = name.clone();
                                                move |_w, _cx| {
                                                    // TODO: wire actual SFTP
                                                    // download once the backend
                                                    // exposes a read_file API
                                                    // to the UI.
                                                    eprintln!("SFTP download requested: {}", name);
                                                }
                                            },
                                        )]
                                    };
                                    // Add a "Delete" item for everything
                                    // except the ".." parent-navigation entry.
                                    if name != ".." {
                                        items.push(
                                            ContextMenuItem::new(t!("sftp.delete").to_string(), {
                                                let alert_controller = alert_controller.clone();
                                                let name = name.clone();
                                                let target_path = target_path.clone();
                                                move |_w, cx| {
                                                    let Some(ref ac) = alert_controller else {
                                                        return;
                                                    };
                                                    let target_path = target_path.clone();
                                                    ac.update(cx, |c, cx| {
                                                        c.show(
                                                            AlertState {
                                                                severity: AlertSeverity::Danger,
                                                                title: t!("sftp.delete_title").to_string().into(),
                                                                description: Some(
                                                                    t!("sftp.delete_prompt", name = name.as_str())
                                                                        .to_string()
                                                                        .into(),
                                                                ),
                                                                confirm_label: t!("sftp.delete_confirm").to_string().into(),
                                                                cancel_label: t!("terminal.host_key_cancel").to_string().into(),
                                                                on_confirm: Some(Rc::new(move |_w, _cx| {
                                                                    // TODO: wire actual SFTP delete
                                                                    // (remove_file / remove_dir) once
                                                                    // the backend exposes it to the UI.
                                                                    eprintln!(
                                                                        "SFTP delete confirmed: {}",
                                                                        target_path
                                                                    );
                                                                })),
                                                                ..AlertState::default()
                                                            },
                                                            cx,
                                                        );
                                                    });
                                                }
                                            })
                                            .danger(true),
                                        );
                                    }
                                    cm.update(cx, |c, cx| {
                                        c.show(
                                            ContextMenuState {
                                                position: pos,
                                                items,
                                                ..ContextMenuState::default()
                                            },
                                            cx,
                                        );
                                    });
                                }
                            })
                            // Smooth hover color transition. Uses
                            // `transition_when_else` (not `transition_on_hover`)
                            // so we can also force the highlight on when a
                            // context menu triggered by this row is open.
                            .with_transition(row_id_for_transition)
                            .on_hover({
                                let name = name.clone();
                                move |hovered, _w, cx| {
                                    let _ = entity.update(cx, |view, cx| {
                                        if *hovered {
                                            view.hovered_entry = Some(name.clone());
                                        } else if view.hovered_entry.as_deref()
                                            == Some(name.as_str())
                                        {
                                            view.hovered_entry = None;
                                        }
                                        cx.notify();
                                    });
                                }
                            })
                            .transition_when_else(
                                is_highlighted,
                                std::time::Duration::from_millis(120),
                                Linear,
                                |el| el.bg(rgba((SURFACE_HOVER << 8) | 0xFF)),
                                |el| el.bg(rgba((SURFACE_HOVER << 8) | 0x00)),
                            )
                            .child(
                                svg()
                                    .path(icon_path)
                                    .size(px(14.0))
                                    .flex_shrink_0()
                                    .text_color(rgb(TEXT_MUTED)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(TEXT_PRIMARY))
                                    .whitespace_nowrap()
                                    .overflow_hidden()
                                    .child(name.clone()),
                            )
                    })),
            )
    }
}
