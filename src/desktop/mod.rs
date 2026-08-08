mod app;
mod ui;

use std::sync::Arc;

use app::OneChat;
use gpui::{
    App, Bounds, Context, Entity, Render, TitlebarOptions, Window, WindowBackgroundAppearance,
    WindowBounds, WindowOptions, div, prelude::*, px, size,
};
use gpui_component::Root;
use gpui_component_assets::Assets;
use tokio::runtime::Builder;

use crate::{mcp::McpManager, storage::Storage};

struct WindowContent {
    one_chat: Entity<OneChat>,
}

impl Render for WindowContent {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .relative()
            .size_full()
            .font(ui::theme::ui_font(cx))
            .child(self.one_chat.clone())
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

pub fn run() {
    let storage = Arc::new(Storage::open_default().expect("failed to open OneChat storage"));
    let runtime = Arc::new(
        Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("failed to start network runtime"),
    );
    let mcp = Arc::new(McpManager::new(storage.mcp_path()));
    gpui_platform::application()
        .with_assets(Assets)
        .run(move |cx: &mut App| {
            ui::init(cx);

            let bounds = Bounds::centered(None, size(px(1240.0), px(820.0)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(900.0), px(640.0))),
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
                    let mcp = mcp.clone();
                    move |window, cx| {
                        let one_chat = cx.new(|cx| OneChat::new(storage, runtime, mcp, window, cx));
                        let initial_focus = one_chat.read(cx).initial_focus_handle(cx);
                        window.focus(&initial_focus, cx);
                        let content = cx.new(|_| WindowContent { one_chat });
                        cx.new(|cx| Root::new(content, window, cx).bg(gpui::transparent_black()))
                    }
                },
            )
            .expect("failed to open OneChat window");

            cx.activate(true);
        });
}
