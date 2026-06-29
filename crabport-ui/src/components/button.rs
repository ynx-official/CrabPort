use crate::color::*;
use gpui::{prelude::FluentBuilder, *};
use gpui_animation::{animation::TransitionExt, transition::general::Linear};
use std::{rc::Rc, time::Duration};

// ---------------------------------------------------------------------------
// Button
// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct Button {
    id: ElementId,
    style: StyleRefinement,
    children: Vec<AnyElement>,
    icon: Option<SharedString>,
    on_hover: Option<Rc<dyn Fn(&bool, &mut Window, &mut App) + 'static>>,
    on_click: Option<Rc<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>>,
    on_close: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
    selected: Option<bool>,
    disabled: Option<bool>,
    centered: bool,
    /// When true, the content area allows text to wrap across multiple
    /// lines instead of being clamped to a single line with an ellipsis.
    /// Useful for buttons whose label is longer than the button width
    /// (e.g. the host-key confirmation buttons in the connection overlay).
    multiline: bool,
    // Colors
    bg: u32,
    bg_hover: u32,
    bg_selected: u32,
    bg_disabled: u32,
    border: u32,
    text_disabled: u32,
}

impl Styled for Button {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for Button {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

impl Button {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: Default::default(),
            children: Default::default(),
            icon: None,
            on_hover: None,
            on_click: None,
            on_close: None,
            selected: None,
            disabled: None,
            centered: false,
            multiline: false,
            bg: BTN_BG,
            bg_hover: BTN_BG_HOVER,
            bg_selected: BTN_BG_SELECTED,
            bg_disabled: BTN_BG_DISABLED,
            border: BTN_BORDER,
            text_disabled: BTN_TEXT_DISABLED,
        }
    }

    // -- Color presets --

    pub fn tab(self) -> Self {
        Self {
            bg: TAB_BTN_BG,
            bg_hover: TAB_BTN_BG_HOVER,
            bg_selected: TAB_BTN_BG_SELECTED,
            bg_disabled: TAB_BTN_BG_DISABLED,
            border: TAB_BTN_BORDER,
            text_disabled: TAB_BTN_TEXT_DISABLED,
            ..self
        }
    }

    pub fn primary(self) -> Self {
        Self {
            bg: BTN_PRIMARY_BG,
            bg_hover: BTN_PRIMARY_BG_HOVER,
            bg_selected: BTN_PRIMARY_BG_SELECTED,
            bg_disabled: BTN_PRIMARY_BG_DISABLED,
            border: BTN_PRIMARY_BORDER,
            text_disabled: BTN_PRIMARY_TEXT_DISABLED,
            ..self
        }
    }

    /// Ghost button: transparent background, no visible border. Hover reveals
    /// a subtle surface background. Good for icon-only action buttons nested
    /// inside rows (edit / delete / etc.).
    pub fn ghost(mut self) -> Self {
        self.bg = BTN_GHOST_BG;
        self.bg_hover = BTN_GHOST_BG_HOVER;
        self.bg_selected = BTN_GHOST_BG_SELECTED;
        self.bg_disabled = BTN_GHOST_BG_DISABLED;
        self.border = BTN_GHOST_BORDER;
        self.text_disabled = BTN_GHOST_TEXT_DISABLED;
        self
    }

    // -- Color overrides --

    pub fn bg(mut self, color: u32) -> Self {
        self.bg = color;
        self
    }

    pub fn bg_hover(mut self, color: u32) -> Self {
        self.bg_hover = color;
        self
    }

    pub fn bg_selected(mut self, color: u32) -> Self {
        self.bg_selected = color;
        self
    }

    pub fn bg_disabled(mut self, color: u32) -> Self {
        self.bg_disabled = color;
        self
    }

    pub fn border_color(mut self, color: u32) -> Self {
        self.border = color;
        self
    }

    pub fn text_disabled_color(mut self, color: u32) -> Self {
        self.text_disabled = color;
        self
    }

    // -- Content & behavior --

    pub fn icon(mut self, path: impl Into<SharedString>) -> Self {
        self.icon = Some(path.into());
        self
    }

    pub fn on_hover(mut self, f: impl Fn(&bool, &mut Window, &mut App) + 'static) -> Self {
        self.on_hover = Some(Rc::new(f));
        self
    }

    pub fn on_click(mut self, f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(f));
        self
    }

    pub fn on_close(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_close = Some(Rc::new(f));
        self
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = Some(disabled);
        self
    }

    pub fn centered(mut self, centered: bool) -> Self {
        self.centered = centered;
        self
    }

    /// Allow the button's text content to wrap across multiple lines.
    ///
    /// By default the content area is `whitespace_nowrap` + `text_ellipsis`,
    /// so a long label is truncated to a single line. Enabling multiline
    /// swaps that for `whitespace_normal` and drops the ellipsis so the
    /// label wraps inside the button's width. Pair with an explicit height
    /// (e.g. `.h_10()`) tall enough to fit two lines.
    pub fn multiline(mut self, multiline: bool) -> Self {
        self.multiline = multiline;
        self
    }

    /// Clean up all gpui-animation state associated with this Button.
    /// Call this when the Button is removed from the render tree
    /// (e.g. when closing a tab) to prevent stale hover/transition state
    /// from persisting in the global DashMap.
    pub fn cleanup_animation(id: &ElementId, has_close: bool) {
        gpui_animation::reset_transition(id);
        if has_close {
            let close_bg_id = ElementId::Name(format!("{}-close-bg", id).into());
            let close_opacity_id = ElementId::Name(format!("{}-close-opacity", id).into());
            gpui_animation::reset_transition(&close_bg_id);
            gpui_animation::reset_transition(&close_opacity_id);
        }
    }
}

impl RenderOnce for Button {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let has_close = self.on_close.is_some();

        // Move colors out of self so closures can capture by value
        let bg = self.bg;
        let bg_hover = self.bg_hover;
        let bg_selected = self.bg_selected;
        let bg_disabled = self.bg_disabled;
        let border = self.border;
        let text_disabled = self.text_disabled;
        // Interpret each color as rgba() so the alpha channel is respected.
        // Constants ≤ 0xFFFFFF (3-byte RGB) get an implicit opaque alpha
        // (0xff) appended; 4-byte 0xRRGGBBAA constants use their own alpha.
        let to_color = |c: u32| rgba(if c <= 0xFFFFFF { (c << 8) | 0xFF } else { c });

        let mut root = div()
            .id(self.id.clone())
            .flex()
            .items_center()
            .when_else(
                self.centered,
                |this| this.justify_center(),
                |this| this.justify_start(),
            )
            .w_full()
            .border_1()
            .border_color(to_color(border))
            .rounded_md()
            .h_8()
            .overflow_hidden()
            .bg(to_color(bg));
        root.style().refine(&self.style);

        let multiline = self.multiline;

        // Icon
        let icon_el = self.icon.map(|path| {
            svg()
                .path(path)
                .size_4()
                .flex_shrink_0()
                .text_color(rgb(TEXT_PRIMARY))
                .into_any_element()
        });

        // Content area
        let mut content = div()
            .items_center()
            .gap_2()
            .min_w_0()
            .overflow_hidden()
            .when_else(
                multiline,
                |this| {
                    // Multiline: use a column layout with full width so the
                    // text node is constrained to the button's content box
                    // and actually wraps. `whitespace_normal` only takes
                    // effect when the text element has a bounded width.
                    this.flex_col().w_full().whitespace_normal().text_center()
                },
                |this| this.flex().text_ellipsis().whitespace_nowrap(),
            );

        if has_close {
            content = content.flex_1();
        }

        if let Some(icon) = icon_el {
            content = content.child(icon);
        }

        let content = content.children(self.children);

        // Close area
        // The close container is sized to the icon plus a little padding so
        // the hover region is a comfortable click target without spanning
        // the whole right half of the button.
        let close_el = if has_close {
            let close_bg_id = ElementId::Name(format!("{}-close-bg", self.id).into());
            let close_opacity_id = ElementId::Name(format!("{}-close-opacity", self.id).into());

            Some(
                div()
                    .id(close_opacity_id.clone())
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_end()
                    .mr_1()
                    .opacity(0.)
                    .child(
                        div()
                            .id(close_bg_id)
                            .flex()
                            .items_center()
                            .justify_center()
                            .h_5()
                            .w_5()
                            .rounded_sm()
                            .cursor_pointer()
                            .child(
                                svg()
                                    .path("icons/close.svg")
                                    .size_3p5()
                                    .text_color(rgb(TEXT_PRIMARY)),
                            )
                            .on_click({
                                let on_close = self.on_close.clone();
                                move |_e, w, cx| {
                                    if let Some(ref cb) = on_close {
                                        cb(w, cx);
                                    }
                                    cx.stop_propagation();
                                }
                            })
                            .bg(rgb(SURFACE_ACTIVE)),
                    )
                    .with_transition(close_opacity_id)
                    .transition_on_hover(Duration::from_millis(100), Linear, |hovered, el| {
                        if *hovered {
                            el.opacity(1.)
                        } else {
                            el.opacity(0.)
                        }
                    })
                    .into_any_element(),
            )
        } else {
            None
        };

        let mut root = root.child(content);
        if let Some(close) = close_el {
            root = root.child(close);
        }

        // State transitions — use `move` closures so u32 values are captured by value ('static)
        root.with_transition(self.id).when_else(
            self.disabled.unwrap_or_default(),
            move |this| {
                this.bg(to_color(bg_disabled))
                    .text_color(to_color(text_disabled))
                    .cursor_not_allowed()
            },
            move |this| {
                this.text_color(rgb(TEXT_PRIMARY))
                    .when_some(self.on_hover, |this, on_hover| {
                        this.on_hover(move |h, w, a| (on_hover)(h, w, a))
                    })
                    .when_some(self.on_click, |this, on_click| {
                        this.on_click(move |e, w, a| {
                            (on_click)(e, w, a);
                            // Prevent the click from bubbling to parent
                            // elements (e.g. row double-click handlers).
                            a.stop_propagation();
                        })
                    })
                    .transition_when_else(
                        self.selected.unwrap_or_default(),
                        Duration::from_millis(250),
                        Linear,
                        move |this| this.bg(to_color(bg_selected)),
                        move |this| this.bg(to_color(bg)),
                    )
                    .transition_on_hover(
                        Duration::from_millis(250),
                        Linear,
                        move |hovered, this| {
                            if *hovered {
                                this.bg(to_color(bg_hover))
                            } else {
                                this
                            }
                        },
                    )
            },
        )
    }
}
