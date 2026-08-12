mod app;
mod audio_playback;
mod audio_recording;
mod branch_swipe;
mod html_snapshot;
mod pressure_touch;
mod ui;

use std::{sync::Arc, time::Duration};

use app::OneChat;
use gpui::{
    App, Bounds, Context, DisplayId, Entity, Pixels, Render, TitlebarOptions, Window,
    WindowBackgroundAppearance, WindowBounds, WindowOptions, div, point, prelude::*, px, size,
};
#[cfg(target_os = "macos")]
use gpui::{KeyBinding, Menu, MenuItem, OsAction, SystemMenuType, actions};
use gpui_component::Root;
use gpui_component_assets::Assets;
use tokio::runtime::Builder;

use crate::{
    mcp::McpManager,
    storage::{Storage, WindowMode, WindowState},
};

#[cfg(target_os = "macos")]
actions!(onechat, [Hide, HideOthers, OpenRepository, Quit, ShowAll]);

#[doc(hidden)]
pub fn run_snapshot_helper_if_requested() -> bool {
    html_snapshot::run_helper_if_requested()
}

struct WindowContent {
    one_chat: Entity<OneChat>,
    last_windowed_bounds: Bounds<Pixels>,
    window_state_save_revision: u64,
}

impl WindowContent {
    fn new(
        one_chat: Entity<OneChat>,
        storage: Arc<Storage>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let observed_storage = storage.clone();
        cx.observe_window_bounds(window, move |this, window, cx| {
            let state = stored_window_state(window, this.last_windowed_bounds, cx);
            let windowed_bounds = (state.mode == WindowMode::Windowed).then(|| state.bounds());
            this.window_state_save_revision = this.window_state_save_revision.wrapping_add(1);
            let revision = this.window_state_save_revision;
            let timer = cx.background_executor().timer(Duration::from_millis(200));
            let storage = observed_storage.clone();
            cx.spawn(async move |this, cx| {
                timer.await;
                let _ = this.update(cx, |this, _| {
                    if this.window_state_save_revision != revision {
                        return;
                    }
                    if let Some(bounds) = windowed_bounds {
                        this.last_windowed_bounds = bounds;
                    }
                    let _ = storage.save_window_state(&state);
                });
            })
            .detach();
        })
        .detach();

        let content = cx.weak_entity();
        window.on_window_should_close(cx, move |window, cx| {
            let _ = content.update(cx, |this, cx| {
                this.one_chat.update(cx, |one_chat, cx| {
                    one_chat.cancel_voice_recording(cx);
                    one_chat.stop_audio_playback();
                });
                let state = stored_window_state(window, this.last_windowed_bounds, cx);
                let _ = storage.save_window_state(&state);
            });
            true
        });

        Self {
            one_chat,
            last_windowed_bounds: window.window_bounds().get_bounds(),
            window_state_save_revision: 0,
        }
    }
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
    let (window_bounds, display_id) = restored_window(&storage, cx);
    cx.open_window(
        WindowOptions {
            window_bounds: Some(window_bounds),
            display_id,
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
            pressure_touch::configure(window);
            let one_chat = cx.new(|cx| OneChat::new(storage.clone(), runtime, mcp, window, cx));
            let initial_focus = one_chat.read(cx).initial_focus_handle(cx);
            window.focus(&initial_focus, cx);
            let content = cx.new(|cx| WindowContent::new(one_chat, storage, window, cx));
            cx.new(|cx| Root::new(content, window, cx).bg(gpui::transparent_black()))
        },
    )
    .expect("failed to open OneChat window");
}

fn restored_window(storage: &Storage, cx: &App) -> (WindowBounds, Option<DisplayId>) {
    let Some(state) = storage
        .load_window_state()
        .ok()
        .flatten()
        .filter(WindowState::is_valid)
    else {
        return (
            WindowBounds::Windowed(Bounds::centered(None, size(px(1240.0), px(820.0)), cx)),
            None,
        );
    };

    let saved_display = state.display.as_deref().and_then(|saved| {
        cx.displays()
            .into_iter()
            .find(|display| display.uuid().is_ok_and(|uuid| uuid.to_string() == saved))
    });
    let restore_position = state.display.is_none() || saved_display.is_some();
    let display = saved_display.or_else(|| cx.primary_display());
    let display_id = display.as_ref().map(|display| display.id());

    let mut width = state.width.max(900.0);
    let mut height = state.height.max(640.0);
    let bounds = if let Some(display) = display {
        let available = display.visible_bounds();
        width = width.min(f32::from(available.size.width));
        height = height.min(f32::from(available.size.height));
        let window_size = size(px(width), px(height));
        if restore_position {
            let min_x = f32::from(available.origin.x);
            let min_y = f32::from(available.origin.y);
            let max_x = min_x + f32::from(available.size.width) - width;
            let max_y = min_y + f32::from(available.size.height) - height;
            Bounds::new(
                point(
                    px(state.x.clamp(min_x, max_x)),
                    px(state.y.clamp(min_y, max_y)),
                ),
                window_size,
            )
        } else {
            Bounds::centered(display_id, window_size, cx)
        }
    } else {
        Bounds::new(point(px(state.x), px(state.y)), size(px(width), px(height)))
    };

    let bounds = match state.mode {
        WindowMode::Windowed => WindowBounds::Windowed(bounds),
        WindowMode::Maximized => WindowBounds::Maximized(bounds),
        WindowMode::Fullscreen => WindowBounds::Fullscreen(bounds),
    };
    (bounds, display_id)
}

fn stored_window_state(
    window: &Window,
    last_windowed_bounds: Bounds<Pixels>,
    cx: &App,
) -> WindowState {
    let (mode, bounds) = match window.window_bounds() {
        WindowBounds::Fullscreen(bounds) => (WindowMode::Fullscreen, bounds),
        WindowBounds::Maximized(bounds) => (WindowMode::Maximized, bounds),
        WindowBounds::Windowed(_) if window.is_maximized() => {
            (WindowMode::Maximized, last_windowed_bounds)
        }
        WindowBounds::Windowed(bounds) => (WindowMode::Windowed, bounds),
    };
    WindowState {
        mode,
        display: window
            .display(cx)
            .and_then(|display| display.uuid().ok())
            .map(|uuid| uuid.to_string()),
        x: f32::from(bounds.origin.x),
        y: f32::from(bounds.origin.y),
        width: f32::from(bounds.size.width),
        height: f32::from(bounds.size.height),
    }
}

impl WindowState {
    fn bounds(&self) -> Bounds<Pixels> {
        Bounds::new(
            point(px(self.x), px(self.y)),
            size(px(self.width), px(self.height)),
        )
    }

    fn is_valid(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .into_iter()
            .all(f32::is_finite)
            && self.width > 0.0
            && self.height > 0.0
    }
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
