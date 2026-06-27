use crate::components::segmented_control::{Segment, SegmentedControl};
use gpui::*;
use gpui_animation::animation::TransitionExt;
use gpui_animation::transition::general::EaseInOutQuad;
use std::time::Duration;

// Tabs::new("my-tabs")
//     .active(self.active_tab)
//     .on_change(cx.listener(|this, idx, _, cx| {
//         this.active_tab = *idx;
//         cx.notify();
//     }))
//     .pane(TabPane::new("Overview", my_overview_component()))
//     .pane(TabPane::new("Settings", my_settings_component()))
//     .pane(TabPane::new("History",  my_history_component()))

// ---------------------------------------------------------------------------
// TabPane
// ---------------------------------------------------------------------------
pub struct TabPane {
    pub label: SharedString,
    pub content: AnyElement,
}

impl TabPane {
    pub fn new(label: impl Into<SharedString>, content: impl IntoElement + 'static) -> Self {
        Self {
            label: label.into(),
            content: content.into_any_element(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------
#[derive(IntoElement)]
pub struct Tabs {
    id: ElementId,
    style: StyleRefinement,
    panes: Vec<TabPane>,
    active: usize,
    on_change: Option<std::rc::Rc<dyn Fn(usize, &mut Window, &mut App) + 'static>>,
}

impl Styled for Tabs {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Tabs {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            style: Default::default(),
            panes: Vec::new(),
            active: 0,
            on_change: None,
        }
    }

    pub fn pane(mut self, pane: TabPane) -> Self {
        self.panes.push(pane);
        self
    }

    pub fn active(mut self, index: usize) -> Self {
        self.active = index;
        self
    }

    pub fn on_change(mut self, f: impl Fn(usize, &mut Window, &mut App) + 'static) -> Self {
        self.on_change = Some(std::rc::Rc::new(f));
        self
    }
}

impl RenderOnce for Tabs {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        let count = self.panes.len().max(1);
        let active = self.active.min(count - 1);
        let id_str = format!("{:?}", self.id);

        // -----------------------------------------------------------------------
        // SegmentedControl
        // -----------------------------------------------------------------------
        let on_change_rc: Option<std::rc::Rc<dyn Fn(usize, &mut Window, &mut App)>> =
            self.on_change;

        let mut ctrl = SegmentedControl::new(ElementId::Name(format!("{}-ctrl", id_str).into()))
            .active(active);

        for (i, pane) in self.panes.iter().enumerate() {
            let cb = on_change_rc.clone();
            let seg = Segment::new(pane.label.clone()).on_select(move |w, cx| {
                if let Some(f) = &cb {
                    f(i, w, cx);
                }
            });
            ctrl = ctrl.segment(seg);
        }

        let track_id = ElementId::Name(format!("{}-slide-track", id_str).into());

        let panel_w = DefiniteLength::Fraction(1.0_f32 / count as f32);

        let panels: Vec<AnyElement> = self
            .panes
            .into_iter()
            .enumerate()
            .map(|(i, pane)| {
                let is_active = i == active;
                let panel_id = ElementId::Name(format!("{}-panel-{}", id_str, i).into());

                div()
                    .id(panel_id.clone())
                    .flex_none()
                    .w(panel_w)
                    .h_full()
                    .overflow_hidden()
                    .opacity(0.)
                    .with_transition(panel_id)
                    .transition_when_else(
                        is_active,
                        Duration::from_millis(280),
                        EaseInOutQuad,
                        |state| state.opacity(1.),
                        |state| state.opacity(0.),
                    )
                    .child(pane.content)
                    .into_any_element()
            })
            .collect();

        let mut track = div()
            .id(track_id.clone())
            .absolute()
            .flex()
            .flex_row()
            .h_full()
            .w(DefiniteLength::Fraction(count as f32))
            .with_transition(track_id)
            .children(panels);

        for i in 0..count {
            // left 是相对于 clip（containing block）的 Fraction
            // active=i 时 left = -(i as f32) × 100% of clip
            let target = DefiniteLength::Fraction(-(i as f32));
            track = track.transition_when_else(
                active == i,
                Duration::from_millis(320),
                EaseInOutQuad,
                move |state| state.left(target),
                |state| state,
            );
        }

        let content_area = div()
            .relative()
            .w_full()
            .flex_1()
            .overflow_hidden()
            .child(track);

        // -----------------------------------------------------------------------
        // Root
        // -----------------------------------------------------------------------
        let mut root = div()
            .id(self.id)
            .flex()
            .flex_col()
            .w_full()
            .h_full()
            .gap_2()
            .child(ctrl)
            .child(content_area);

        root.style().refine(&self.style);
        root
    }
}
