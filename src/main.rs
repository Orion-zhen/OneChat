mod app;
mod ui;

use app::OneChat;
use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};

fn main() {
    Application::new().run(|cx: &mut App| {
        ui::init(cx);

        let bounds = Bounds::centered(None, size(px(1100.0), px(760.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(900.0), px(640.0))),
                    ..Default::default()
                },
                |_, cx| cx.new(OneChat::new),
            )
            .expect("failed to open OneChat window");

        window
            .update(cx, |app, window, cx| {
                window.focus(&app.composer_focus_handle(cx));
                cx.activate(true);
            })
            .expect("failed to initialize OneChat window");
    });
}
