use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::InteractiveElementExt;
use gpui_component::input::InputState;
use gpui_component::scroll::ScrollableElement;

use crate::color::*;
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
}

impl SftpPanel {
    pub fn new() -> Self {
        Self {
            path_input: None,
            cwd: None,
            entries: Arc::new(Vec::new()),
            on_navigate: None,
            active_tab_id: None,
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

                        div()
                            .id(ElementId::Name(format!("sftp-{}", name).into()))
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_1p5()
                            .px_2()
                            .py_1()
                            .rounded(px(4.0))
                            .hover(|s| s.bg(rgb(SURFACE_HOVER)))
                            .when(*is_dir, |el| {
                                el.on_double_click({
                                    let on_navigate = on_navigate.clone();
                                    let target = target_path.clone();
                                    move |_, _w, cx| {
                                        if let Some(ref cb) = on_navigate {
                                            cb(target.clone(), cx);
                                        }
                                    }
                                })
                            })
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
