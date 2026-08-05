mod app;
pub mod db;
pub mod generation;
pub mod model;
pub mod providers;
pub mod ui;

use std::sync::Arc;

use app::OneChat;
use db::Database;
use gpui::{App, Application, Bounds, WindowBounds, WindowOptions, prelude::*, px, size};

fn main() {
    let database = Arc::new(Database::open_default().expect("failed to open OneChat database"));
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to start network runtime"),
    );
    Application::new().run(move |cx: &mut App| {
        ui::init(cx);

        let bounds = Bounds::centered(None, size(px(1100.0), px(760.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(900.0), px(640.0))),
                    ..Default::default()
                },
                {
                    let database = database.clone();
                    let runtime = runtime.clone();
                    move |_, cx| cx.new(|cx| OneChat::new(database, runtime, cx))
                },
            )
            .expect("failed to open OneChat window");

        window
            .update(cx, |app, window, cx| {
                window.focus(&app.initial_focus_handle(cx));
                cx.activate(true);
            })
            .expect("failed to initialize OneChat window");
    });
}
