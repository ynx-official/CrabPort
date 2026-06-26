use gpui::{prelude::FluentBuilder, *};
use std::rc::Rc;

use crate::color::*;

// ---------------------------------------------------------------------------
// SegmentedControl — segments with a sliding background indicator
// ---------------------------------------------------------------------------

/// A single segment in a [SegmentedControl].
pub struct Segment {
    pub label: SharedString,
    pub on_select: Option<Rc<dyn Fn(&mut Window, &mut App) + 'static>>,
}

impl Segment {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            on_select: None,
        }
    }

    pub fn on_select(mut self, f: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_select = Some(Rc::new(f));
        self
    }
}

// ---------------------------------------------------------------------------

#[derive(IntoElement)]
pub struct SegmentedControl {
    id: ElementId,
    style: StyleRefinement,
    segments: Vec<Segment>,
    active: usize,
}

impl Styled for SegmentedControl {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl SegmentedControl {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: Default::default(),
            segments: Vec::new(),
            active: 0,
        }
    }

    pub fn segment(mut self, seg: Segment) -> Self {
        self.segments.push(seg);
        self
    }

    pub fn active(mut self, index: usize) -> Self {
        self.active = index;
        self
    }
}

impl RenderOnce for SegmentedControl {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let count = self.segments.len().max(1);
        let active = self.active.min(count - 1);
        let indicator_id = ElementId::Name(format!("{}-indicator", self.id).into());

        // Each segment occupies 100%/count. The indicator slides to the active one.
        let seg_width = 100.0 / count as f32;
        let left_pct = active as f32 * seg_width;

        // Sliding background indicator
        let indicator = div()
            .id(indicator_id.clone())
            .absolute()
            .top_0()
            .bottom_0()
            .left(DefiniteLength::Fraction(left_pct / 100.0))
            .w(DefiniteLength::Fraction(seg_width / 100.0))
            .rounded_sm()
            .bg(rgb(BG_BASE));

        // Build tab elements
        let tabs: Vec<AnyElement> = self
            .segments
            .into_iter()
            .enumerate()
            .map(|(i, seg)| {
                let is_active = i == active;
                let on_select = seg.on_select.clone();

                let mut tab = div()
                    .flex_1()
                    .px_3()
                    .py_1()
                    .rounded_sm()
                    .text_sm()
                    .text_center()
                    .when(is_active, |el| el.text_color(rgb(TEXT_PRIMARY)))
                    .when(!is_active, |el| el.text_color(rgb(TEXT_MUTED)))
                    .child(seg.label);

                if let Some(cb) = on_select {
                    tab = tab.on_mouse_down(MouseButton::Left, move |_e, w, cx| {
                        cb(w, cx);
                    });
                }

                tab.into_any_element()
            })
            .collect();

        // Inner wrapper so indicator aligns with tabs; outer padding creates margin from background
        let inner = div()
            .relative()
            .flex()
            .flex_row()
            .w_full()
            .child(indicator)
            .children(tabs);

        let mut root = div()
            .id(self.id)
            .bg(rgb(SURFACE_ACTIVE))
            .rounded_md()
            .p_0p5()
            .child(inner);

        root.style().refine(&self.style);

        root
    }
}
