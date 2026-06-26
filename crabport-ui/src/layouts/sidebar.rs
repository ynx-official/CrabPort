use gpui::*;

use crate::app::SidebarItem;
use crate::color::*;
use crate::components::button::Button;

pub fn render_sidebar(
    selected: SidebarItem,
    handle: &Entity<crate::app::CrabportApp>,
) -> impl IntoElement {
    div()
        .w(px(200.0))
        .h_full()
        .bg(rgb(BG_SIDEBAR))
        .border_r_1()
        .border_color(rgb(BORDER))
        .flex()
        .flex_col()
        .pt_11()
        .px_2()
        .gap_1()
        .children(SidebarItem::all().map(|item| {
            let is_selected = item == selected;
            let h = handle.clone();
            Button::new(ElementId::Name(format!("sidebar-{item:?}").into()))
                .tab()
                .selected(is_selected)
                .icon(item.icon())
                .child(item.label())
                .on_click(move |_e, _w, cx| {
                    h.update(cx, |app, _| {
                        app.sidebar_item = item;
                    });
                })
                .h_9()
                .border_0()
                .px_2()
                .text_sm()
        }))
}
