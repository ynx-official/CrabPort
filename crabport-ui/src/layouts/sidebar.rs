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
                .selected(is_selected)
                .on_click(move |_e, _w, cx| {
                    h.update(cx, |app, _| {
                        app.sidebar_item = item;
                    });
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            svg()
                                .path(item.icon())
                                .size_4()
                                .text_color(rgb(TEXT_PRIMARY)),
                        )
                        .child(item.label()),
                )
                .h_9()
                .border_0()
                .px_2()
                .text_sm()
        }))
}
