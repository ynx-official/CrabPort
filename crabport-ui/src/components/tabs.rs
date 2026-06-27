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
    /// Pre-built base string for deriving child ids once, not per frame.
    id_str: String,
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
        let id: ElementId = id.into();
        let id_str = format!("{id:?}");
        Self {
            id,
            id_str,
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
        let id_str = self.id_str;

        // -------------------------------------------------------------------
        // Header: SegmentedControl
        // -------------------------------------------------------------------
        let on_change_rc = self.on_change;

        let mut ctrl =
            SegmentedControl::new(ElementId::Name(format!("{id_str}-ctrl").into())).active(active);

        for (i, pane) in self.panes.iter().enumerate() {
            let cb = on_change_rc.clone();
            let seg = Segment::new(pane.label.clone()).on_select(move |w, cx| {
                if let Some(f) = &cb {
                    f(i, w, cx);
                }
            });
            ctrl = ctrl.segment(seg);
        }

        // -------------------------------------------------------------------
        // Panels: ONLY the active pane builds its (expensive) content.
        // Inactive panes are empty placeholders, so heavyweight children like
        // `Input` are never laid out while hidden. A short opacity fade keeps
        // switching smooth without the old full-width sliding track.
        // -------------------------------------------------------------------
        let panels: Vec<AnyElement> = self
            .panes
            .into_iter()
            .enumerate()
            .map(|(i, pane)| {
                let is_active = i == active;
                let panel_id = ElementId::Name(format!("{id_str}-panel-{i}").into());

                let mut panel = div()
                    .id(panel_id.clone())
                    .absolute()
                    .inset_0()
                    .overflow_hidden()
                    .opacity(0.)
                    .with_transition(panel_id)
                    .transition_when_else(
                        is_active,
                        Duration::from_millis(200),
                        EaseInOutQuad,
                        |state| state.opacity(1.),
                        |state| state.opacity(0.),
                    );

                if is_active {
                    panel = panel.child(pane.content);
                }
                panel.into_any_element()
            })
            .collect();

        let content_area = div()
            .relative()
            .w_full()
            .flex_1()
            .overflow_hidden()
            .children(panels);

        // -------------------------------------------------------------------
        // Root
        // -------------------------------------------------------------------
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
