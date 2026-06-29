use std::rc::Rc;

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::InteractiveElementExt;
use gpui_component::scroll::ScrollableElement;

use crate::color::*;

pub fn render_sftp_panel(
    entries: Vec<(String, bool)>,
    cwd: Option<String>,
    on_navigate: Option<Rc<dyn Fn(String, &mut Window, &mut App)>>,
) -> impl IntoElement {
    let cwd_display = cwd.clone().unwrap_or_else(|| "/".into());

    // Sort entries alphabetically, directories first
    let mut sorted = entries;
    sorted.sort_by(|a, b| {
        // . and .. always first
        match (a.0.as_str(), b.0.as_str()) {
            (".", _) => std::cmp::Ordering::Less,
            (_, ".") => std::cmp::Ordering::Greater,
            ("..", _) => std::cmp::Ordering::Less,
            (_, "..") => std::cmp::Ordering::Greater,
            _ => a.0.to_lowercase().cmp(&b.0.to_lowercase()),
        }
    });

    // Prepend . and .. directory entries
    let mut all_entries: Vec<(String, bool)> = vec![("..".into(), true)];
    all_entries.extend(sorted);

    div()
        .h_full()
        .flex()
        .flex_col()
        .pt_1()
        .px_2()
        .child(
            // Path bar
            div()
                .px_2()
                .py_1()
                .mb_1()
                .rounded(px(4.0))
                .bg(rgb(SURFACE_ACTIVE))
                .border_1()
                .border_color(rgb(BORDER))
                .text_xs()
                .text_color(rgb(TEXT_MUTED))
                .whitespace_nowrap()
                .overflow_hidden()
                .child(cwd_display),
        )
        .child(
            div()
                .flex_1()
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
                    let cwd_ref = cwd.as_deref().unwrap_or("/");
                    let target_path = if name == "." {
                        cwd_ref.to_string()
                    } else if name == ".." {
                        // Go up one level
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
                                move |_, w, cx| {
                                    if let Some(ref cb) = on_navigate {
                                        cb(target.clone(), w, cx);
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
