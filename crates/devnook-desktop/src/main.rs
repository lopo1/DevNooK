mod app;
mod ui;

use app::DevNookApp;
use gpui_kit::{App, AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};

fn open_main_window(cx: &mut App) {
    let bounds = Bounds::centered(None, size(px(1240.), px(820.)), cx);
    gpui_kit::open_window(
        WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(860.), px(560.))),
            is_resizable: true,
            ..Default::default()
        },
        cx,
        |window, cx| cx.new(|cx| DevNookApp::new(window, cx)),
    )
    .expect("Failed to open DevNook window");
    cx.activate(true);
}

fn main() {
    let app = gpui_kit::application().with_assets(gpui_kit::assets::Assets);
    app.on_reopen(|cx| {
        if cx.windows().is_empty() {
            open_main_window(cx);
        } else {
            cx.activate(true);
        }
    });
    app.run(|cx: &mut App| {
        gpui_kit::init(cx);
        open_main_window(cx);
    });
}
