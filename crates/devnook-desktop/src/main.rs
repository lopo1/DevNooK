mod app;
mod ui;

use app::DevNookApp;
use gpui_kit::{App, AppContext as _, Bounds, WindowBounds, WindowOptions, px, size};

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx: &mut App| {
            gpui_kit::init(cx);
            let bounds = Bounds::centered(None, size(px(1240.), px(820.)), cx);
            gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    ..Default::default()
                },
                cx,
                |window, cx| cx.new(|cx| DevNookApp::new(window, cx)),
            )
            .expect("Failed to open DevNook window");
            cx.activate(true);
        });
}
