mod chat_page;
mod pickers;
mod runtime;
mod sidebar;
mod top_bar;

use std::cell::Cell;

use chat_page::render_chat_page;
pub(crate) use pickers::{
    CommandPaletteDelegate, ModelPickerDelegate, PromptPickerDelegate, ReasoningPickerDelegate,
    command_palette_dialog,
};
use runtime::prepare_render;
use sidebar::render_sidebar;
use top_bar::render_top_bar;

use gpui::{
    AnyElement, App, ClickEvent, Context, DragMoveEvent, ElementId, Empty, Focusable as _,
    FontWeight, KeyBinding, MouseButton, SharedString, Window, actions, div, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _, Selectable as _, Sizable as _, WindowExt as _,
    button::{Button, ButtonVariants as _},
    input::{Escape as InputEscape, Input},
    notification::Notification,
};

use super::{
    SIDEBAR_WIDTH,
    icons::{AppIcon, IconTone, render_icon},
    motion::{translated_x, waiting_title},
    theme as component_theme,
};

use crate::{
    desktop::app::{ConnectionTestStatus, OneChat, Page, PendingFocus},
    desktop::ui::{chat, inspector, settings},
    domain::{AutoTitleState, Conversation, ModelCapabilities},
    mcp::McpServerStatus,
};

const CHAT_SIDEBAR_MIN_WIDTH: f32 = 220.0;
const CHAT_SIDEBAR_MAX_WIDTH: f32 = 380.0;
const SIDEBAR_RESIZE_HANDLE_WIDTH: f32 = 6.0;
const ATTACHMENT_DROP_GROUP: &str = "attachment-drop-zone";

#[derive(Default)]
struct SidebarResizeDrag {
    pointer_offset: Cell<f32>,
}

actions!(
    onechat,
    [
        #[cfg(target_os = "macos")]
        AboutOneChat,
        #[cfg(target_os = "macos")]
        CloseWindow,
        DismissOverlay,
        #[cfg(target_os = "macos")]
        MinimizeWindow,
        NewConversation,
        OpenSettings,
        SaveSettingsEdit,
        ShowCommandPalette,
        ShowModelPicker,
        #[cfg(target_os = "macos")]
        ToggleFullScreen,
        ToggleSidebar,
        #[cfg(target_os = "macos")]
        ZoomWindow,
    ]
);

fn shortcut_label(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{key}")
    } else {
        format!("Ctrl+{key}")
    }
}

pub fn init(cx: &mut App) {
    let primary = if cfg!(target_os = "macos") {
        "cmd"
    } else {
        "ctrl"
    };
    let shortcut = |key: &str| format!("{primary}-{key}");
    cx.bind_keys([
        KeyBinding::new(&shortcut("n"), NewConversation, Some("OneChat")),
        KeyBinding::new(&shortcut("k"), ShowCommandPalette, Some("OneChat")),
        KeyBinding::new(&shortcut("l"), ShowModelPicker, Some("OneChat")),
        KeyBinding::new(&shortcut("shift-s"), ToggleSidebar, Some("OneChat")),
        KeyBinding::new(&shortcut(","), OpenSettings, Some("OneChat")),
        KeyBinding::new(&shortcut("s"), SaveSettingsEdit, Some("OneChat")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-w", CloseWindow, Some("OneChat")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-m", MinimizeWindow, Some("OneChat")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("ctrl-cmd-f", ToggleFullScreen, Some("OneChat")),
        KeyBinding::new("escape", DismissOverlay, Some("OneChat")),
    ]);
}

struct AppErrorNotification;

fn icon_tooltip(icon: AppIcon) -> &'static str {
    match icon {
        AppIcon::ChevronLeft => "Collapse sidebar",
        AppIcon::Close => "Close",
        AppIcon::Compose => "New conversation",
        AppIcon::Info => "Toggle Inspector",
        AppIcon::Layers => "Manage models",
        AppIcon::Pencil => "Rename",
        AppIcon::Pin => "Pin or unpin",
        AppIcon::Plus => "New conversation",
        AppIcon::Settings => "Open settings",
        AppIcon::Sidebar => "Toggle sidebar",
        AppIcon::Trash => "Delete",
        _ => "Action",
    }
}

fn button_base(id: impl Into<ElementId>) -> Button {
    Button::new(id)
}

fn icon_button(id: impl Into<ElementId>, icon: AppIcon, tone: IconTone, cx: &App) -> Button {
    Button::new(id)
        .ghost()
        .tooltip(icon_tooltip(icon))
        .size(px(28.0))
        .p_0()
        .child(render_icon(icon, tone, 17.0, cx))
}

fn large_icon_button(id: impl Into<ElementId>, icon: AppIcon, tone: IconTone, cx: &App) -> Button {
    Button::new(id)
        .ghost()
        .tooltip(icon_tooltip(icon))
        .size(px(36.0))
        .p_0()
        .child(render_icon(icon, tone, 20.0, cx))
}

fn primary_icon_button(id: impl Into<ElementId>, icon: AppIcon, cx: &App) -> Button {
    Button::new(id)
        .primary()
        .rounded(px(18.0))
        .tooltip(icon_tooltip(icon))
        .size(px(36.0))
        .p_0()
        .child(render_icon(icon, IconTone::OnAccent, 20.0, cx))
}

pub fn render(app: &mut OneChat, window: &mut Window, cx: &mut Context<OneChat>) -> AnyElement {
    let animated_title = prepare_render(app, window, cx);
    let scale_factor = window.scale_factor();
    let reduce_motion = cx.reduce_motion();
    let inspector_progress = app
        .navigation
        .inspector_motion
        .progress(window, reduce_motion);
    let picker_progress = app.overlays.picker_motion.progress(window, false);
    if app.overlays.picker.is_some() && app.overlays.picker_motion.is_hidden() {
        app.overlays.picker = None;
        if let Some(focus) = app.overlays.picker_previous_focus.take() {
            window.focus(&focus, cx);
        }
    }
    let picker_overlay = app.overlays.picker.map(|picker| {
        pickers::render_picker_overlay(app, picker, picker_progress, reduce_motion, cx)
    });
    let jump_to_latest_visible = app.navigation.page == Page::Chat
        && app.current_conversation().is_some()
        && !app.chat.follow_latest
        && !app.chat.message_scroll_motion.is_active();
    app.chat
        .jump_to_latest_motion
        .set_visible(jump_to_latest_visible);
    let jump_to_latest_progress = app
        .chat
        .jump_to_latest_motion
        .progress(window, reduce_motion);
    let timeline_expansion = app
        .chat
        .timeline
        .expansion_motion
        .progress(window, reduce_motion);
    let timeline_focused = app.chat.timeline.focus.is_focused(window);
    let sidebar_width = app
        .navigation
        .sidebar_width_motion
        .progress(window, reduce_motion);
    let chat_available_width = (f32::from(window.bounds().size.width) - sidebar_width).max(0.0);
    let sidebar = (app.navigation.page == Page::Chat && sidebar_width > 0.01).then(|| {
        div()
            .w(px(sidebar_width))
            .h_full()
            .flex_none()
            .overflow_hidden()
            .child(render_sidebar(
                app,
                sidebar_width,
                animated_title.as_deref(),
                cx,
            ))
    });
    let sidebar_resize_handle =
        (app.navigation.page == Page::Chat && !app.settings().sidebar_collapsed).then(|| {
            div()
                .id("sidebar-resize-handle")
                .absolute()
                .top_0()
                .bottom_0()
                .left(px(sidebar_width - SIDEBAR_RESIZE_HANDLE_WIDTH / 2.0))
                .w(px(SIDEBAR_RESIZE_HANDLE_WIDTH))
                .cursor_col_resize()
                .on_drag(
                    SidebarResizeDrag::default(),
                    |drag, pointer_offset, _, cx| {
                        drag.pointer_offset.set(f32::from(pointer_offset.x));
                        cx.new(|_| Empty)
                    },
                )
                .on_click(cx.listener(|this, event: &ClickEvent, _, cx| {
                    if event.click_count() >= 2 && this.sidebar.width != SIDEBAR_WIDTH {
                        this.sidebar.width = SIDEBAR_WIDTH;
                        this.navigation
                            .sidebar_width_motion
                            .set_target(SIDEBAR_WIDTH, true);
                        cx.notify();
                    }
                }))
        });
    let top_bar = render_top_bar(app, animated_title.as_deref(), cx);
    let page = match app.navigation.page {
        Page::Chat => render_chat_page(
            app,
            chat_available_width,
            scale_factor,
            jump_to_latest_progress,
            timeline_expansion,
            timeline_focused,
            cx,
        ),
        Page::Settings => settings::render(app, sidebar_width, cx),
    };
    let inspector = (app.navigation.page == Page::Chat
        && (app.navigation.inspector_open || inspector_progress > 0.0))
        .then(|| {
            div()
                .id("inspector-overlay")
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .left_0()
                .occlude()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, window, cx| {
                        if !window.has_active_prompt() {
                            this.close_inspector(cx);
                        }
                    }),
                )
                .child(
                    div()
                        .absolute()
                        .top(px(60.0))
                        .right_0()
                        .bottom_0()
                        .left_0()
                        .child(translated_x(
                            inspector::render(app, cx),
                            px(368.0 * (1.0 - inspector_progress)),
                        )),
                )
        });

    let root = div()
        .relative()
        .size_full()
        .flex()
        .track_focus(&app.root_focus)
        .key_context("OneChat")
        .on_drag_move(
            cx.listener(|this, event: &DragMoveEvent<SidebarResizeDrag>, _, cx| {
                let pointer_offset = event.drag(cx).pointer_offset.get();
                let width = (f32::from(event.event.position.x) - pointer_offset
                    + SIDEBAR_RESIZE_HANDLE_WIDTH / 2.0)
                    .clamp(CHAT_SIDEBAR_MIN_WIDTH, CHAT_SIDEBAR_MAX_WIDTH);
                if this.sidebar.width != width {
                    this.sidebar.width = width;
                    this.navigation.sidebar_width_motion.snap(width);
                    cx.notify();
                }
            }),
        );
    #[cfg(target_os = "macos")]
    let root = root
        .on_action(cx.listener(|_, _: &AboutOneChat, window, cx| {
            window.open_dialog(cx, |dialog, _, _| {
                dialog.title("About OneChat").child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child("OneChat")
                        .child(format!("Version {}", env!("CARGO_PKG_VERSION")))
                        .child("Your last one chatbox app."),
                )
            });
        }))
        .on_action(|_: &CloseWindow, window, _| window.remove_window())
        .on_action(|_: &MinimizeWindow, window, _| window.minimize_window())
        .on_action(|_: &ZoomWindow, window, _| window.zoom_window())
        .on_action(|_: &ToggleFullScreen, window, _| window.toggle_fullscreen());

    root.on_action(cx.listener(|this, _: &NewConversation, _, cx| this.create_conversation(cx)))
        .on_action(cx.listener(|this, _: &ShowCommandPalette, window, cx| {
            this.open_command_palette(window, cx)
        }))
        .on_action(cx.listener(|this, _: &ShowModelPicker, window, cx| {
            this.open_model_picker_immediate(window, cx)
        }))
        .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| this.toggle_sidebar(cx)))
        .on_action(cx.listener(|this, _: &OpenSettings, _, cx| this.set_page(Page::Settings, cx)))
        .on_action(cx.listener(|this, _: &SaveSettingsEdit, window, cx| {
            if this.navigation.page == Page::Settings && this.settings_ui.provider_editor.is_some()
            {
                this.save_provider(window, cx);
            }
        }))
        .on_action(
            cx.listener(|this, _: &DismissOverlay, window, cx| this.dismiss_overlay(window, cx)),
        )
        .bg(component_theme::window_background(
            app.theme(),
            window.appearance(),
            app.settings().background_opacity(),
            cx,
        ))
        .text_color(cx.theme().foreground)
        .text_size(px(15.0))
        .children(sidebar)
        .child(
            div()
                .relative()
                .min_w_0()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .child(top_bar)
                .child(div().relative().min_h_0().flex_1().flex().child(page)),
        )
        .children(sidebar_resize_handle)
        .children(inspector)
        .children(picker_overlay)
        .into_any_element()
}
