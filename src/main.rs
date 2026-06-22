use crabport_ui::CrabportApp;
use crabport_ui::assets::CrabportAssets;
use gpui::*;

fn main() {
    Application::new()
        .with_assets(CrabportAssets::new())
        .run(|cx| {
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::centered(size(px(1200.0), px(800.0)), cx)),
                    titlebar: Some(TitlebarOptions {
                        title: None,
                        appears_transparent: true,
                        traffic_light_position: Some(point(px(12.0), px(11.0))),
                    }),
                    ..Default::default()
                },
                |_window, cx| cx.new(|_cx| CrabportApp::new()),
            )
            .expect("Failed to open window");
        });
}
