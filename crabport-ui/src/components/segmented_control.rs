use crate::color::*;
use gpui::*;
use gpui_animation::animation::TransitionExt;
use gpui_animation::transition::general::EaseInOutQuad;
use std::rc::Rc;
use std::time::Duration;

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

        let seg_width = 1.0_f32 / count as f32;
        let id_str = format!("{:?}", self.id);

        // ------------------------------------------------------------------
        // Sliding indicator
        //
        // A spacer is animated to width = active * seg_width, pushing the
        // indicator body to the active position. ONE transition rule with a
        // single concrete target — so when `active` is unchanged the animation
        // settles immediately instead of being re-evaluated every frame.
        // ------------------------------------------------------------------
        let spacer_id: ElementId = ElementId::Name(format!("{id_str}-spacer").into());
        let spacer_target = DefiniteLength::Fraction(active as f32 * seg_width);

        let spacer = div()
            .id(spacer_id.clone())
            .flex_none()
            .h_full()
            .w(spacer_target)
            .with_transition(spacer_id)
            .transition_when_else(
                true,
                Duration::from_millis(250),
                EaseInOutQuad,
                move |state| state.w(spacer_target),
                move |state| state.w(spacer_target),
            );

        let indicator_id: ElementId = ElementId::Name(format!("{id_str}-indicator").into());
        let indicator = div()
            .id(indicator_id)
            .flex_none()
            .w(DefiniteLength::Fraction(seg_width))
            .h_full()
            .rounded_sm()
            .bg(rgb(BG_BASE));

        let track = div()
            .absolute()
            .inset_0()
            .flex()
            .flex_row()
            .child(spacer)
            .child(indicator);

        // ------------------------------------------------------------------
        // Tab labels
        // ------------------------------------------------------------------
        let tabs: Vec<AnyElement> = self
            .segments
            .into_iter()
            .enumerate()
            .map(|(i, seg)| {
                let is_active = i == active;
                let on_select = seg.on_select.clone();
                let tab_id: ElementId = ElementId::Name(format!("{id_str}-tab-{i}").into());

                let tab = div()
                    .id(tab_id.clone())
                    .flex_1()
                    .px_3()
                    .py_1()
                    .rounded_sm()
                    .text_sm()
                    .text_color(rgb(TEXT_MUTED))
                    .text_center()
                    .with_transition(tab_id)
                    .transition_when_else(
                        is_active,
                        Duration::from_millis(200),
                        EaseInOutQuad,
                        |state| state.text_color(rgb(TEXT_PRIMARY)),
                        |state| state.text_color(rgb(TEXT_MUTED)),
                    )
                    .transition_on_hover(
                        Duration::from_millis(150),
                        EaseInOutQuad,
                        move |hovered, state| {
                            if *hovered && !is_active {
                                state.text_color(rgb(TEXT_PRIMARY))
                            } else {
                                state
                            }
                        },
                    )
                    .child(seg.label);

                if let Some(cb) = on_select {
                    tab.on_click(move |_e, w, cx| {
                        cb(w, cx);
                    })
                    .into_any_element()
                } else {
                    tab.into_any_element()
                }
            })
            .collect();

        // ------------------------------------------------------------------
        // Root
        // ------------------------------------------------------------------
        let inner = div()
            .relative()
            .flex()
            .flex_row()
            .w_full()
            .child(track)
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
