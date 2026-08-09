mod app;
mod ui;

use std::sync::Arc;

use app::OneChat;
use gpui::{
    App, Bounds, Context, Entity, Render, TitlebarOptions, Window, WindowBackgroundAppearance,
    WindowBounds, WindowOptions, div, prelude::*, px, size,
};
#[cfg(target_os = "macos")]
use gpui::{KeyBinding, Menu, MenuItem, OsAction, SystemMenuType, actions};
use gpui_component::Root;
use gpui_component_assets::Assets;
use tokio::runtime::Builder;

use crate::{mcp::McpManager, storage::Storage};

#[cfg(target_os = "macos")]
actions!(onechat, [Hide, HideOthers, OpenRepository, Quit, ShowAll]);

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

fn open_main_window(
    storage: Arc<Storage>,
    runtime: Arc<tokio::runtime::Runtime>,
    mcp: Arc<McpManager>,
    cx: &mut App,
) {
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
        move |window, cx| {
            let one_chat = cx.new(|cx| OneChat::new(storage, runtime, mcp, window, cx));
            let initial_focus = one_chat.read(cx).initial_focus_handle(cx);
            window.focus(&initial_focus, cx);
            let content = cx.new(|_| WindowContent { one_chat });
            cx.new(|cx| Root::new(content, window, cx).bg(gpui::transparent_black()))
        },
    )
    .expect("failed to open OneChat window");
}

#[cfg(target_os = "macos")]
fn init_macos(cx: &mut App) {
    use gpui_component::input::{Copy, Cut, Paste, Redo, SelectAll, Undo};

    cx.on_action(|_: &Quit, cx| cx.quit());
    cx.on_action(|_: &Hide, cx| cx.hide());
    cx.on_action(|_: &HideOthers, cx| cx.hide_other_apps());
    cx.on_action(|_: &ShowAll, cx| cx.unhide_other_apps());
    cx.on_action(|_: &OpenRepository, cx| cx.open_url("https://github.com/Orion-zhen/OneChat"));
    cx.bind_keys([
        KeyBinding::new("cmd-q", Quit, None),
        KeyBinding::new("cmd-h", Hide, None),
        KeyBinding::new("alt-cmd-h", HideOthers, None),
    ]);
    cx.set_menus([
        Menu::new("OneChat").items([
            MenuItem::action("About OneChat", ui::shell::AboutOneChat),
            MenuItem::separator(),
            MenuItem::action("Settings…", ui::shell::OpenSettings),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Hide OneChat", Hide),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAll),
            MenuItem::separator(),
            MenuItem::action("Quit OneChat", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Conversation", ui::shell::NewConversation),
            MenuItem::separator(),
            MenuItem::action("Close Window", ui::shell::CloseWindow),
        ]),
        Menu::new("Edit").items([
            MenuItem::os_action("Undo", Undo, OsAction::Undo),
            MenuItem::os_action("Redo", Redo, OsAction::Redo),
            MenuItem::separator(),
            MenuItem::os_action("Cut", Cut, OsAction::Cut),
            MenuItem::os_action("Copy", Copy, OsAction::Copy),
            MenuItem::os_action("Paste", Paste, OsAction::Paste),
            MenuItem::separator(),
            MenuItem::os_action("Select All", SelectAll, OsAction::SelectAll),
        ]),
        Menu::new("View").items([
            MenuItem::action("Command Palette…", ui::shell::ShowCommandPalette),
            MenuItem::action("Choose Model…", ui::shell::ShowModelPicker),
            MenuItem::separator(),
            MenuItem::action("Toggle Sidebar", ui::shell::ToggleSidebar),
            MenuItem::separator(),
            MenuItem::action("Enter Full Screen", ui::shell::ToggleFullScreen),
        ]),
        Menu::new("Window").items([
            MenuItem::action("Minimize", ui::shell::MinimizeWindow),
            MenuItem::action("Zoom", ui::shell::ZoomWindow),
        ]),
        Menu::new("Help").items([MenuItem::action("OneChat Repository", OpenRepository)]),
    ]);
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
    let application = gpui_platform::application().with_assets(Assets);

    #[cfg(target_os = "macos")]
    application.on_reopen({
        let storage = storage.clone();
        let runtime = runtime.clone();
        let mcp = mcp.clone();
        move |cx| {
            if cx.windows().is_empty() {
                open_main_window(storage.clone(), runtime.clone(), mcp.clone(), cx);
            }
            cx.activate(true);
        }
    });

    application.run(move |cx: &mut App| {
        ui::init(cx);
        #[cfg(target_os = "macos")]
        init_macos(cx);
        open_main_window(storage, runtime, mcp, cx);
        cx.activate(true);
    });
}
