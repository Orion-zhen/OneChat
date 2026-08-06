mod app;
mod ui;

use std::sync::Arc;

use app::OneChat;
use gpui::{
    App, Application, Bounds, TitlebarOptions, WindowBackgroundAppearance, WindowBounds,
    WindowOptions, prelude::*, px, size,
};
use tokio::runtime::Builder;

use crate::storage::Storage;

pub fn run() {
    let storage = Arc::new(Storage::open_default().expect("failed to open OneChat storage"));
    let runtime = Arc::new(
        Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to start network runtime"),
    );
    Application::new().run(move |cx: &mut App| {
        ui::init(cx);

        let bounds = Bounds::centered(None, size(px(1180.0), px(800.0)), cx);
        let window = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(860.0), px(620.0))),
                    titlebar: Some(TitlebarOptions {
                        title: Some("OneChat".into()),
                        appears_transparent: false,
                        traffic_light_position: None,
                    }),
                    window_background: WindowBackgroundAppearance::Blurred,
                    ..Default::default()
                },
                {
                    let storage = storage.clone();
                    let runtime = runtime.clone();
                    move |_, cx| cx.new(|cx| OneChat::new(storage, runtime, cx))
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
