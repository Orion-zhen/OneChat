mod pickers;
mod sidebar;

pub(crate) use pickers::{
    CommandPaletteDelegate, ModelPickerDelegate, PromptPickerDelegate, command_palette_dialog,
    model_picker_dialog, prompt_picker_dialog,
};
use sidebar::render_sidebar;

use gpui::{
    AnyElement, App, Context, ElementId, Focusable as _, FontWeight, KeyBinding, SharedString,
    Window, actions, div, prelude::*, px,
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
    domain::{AutoTitleState, Conversation},
};

actions!(
    onechat,
    [
        NewConversation,
        ShowCommandPalette,
        ShowModelPicker,
        ToggleSidebar,
        OpenSettings,
        DismissOverlay,
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
    component_theme::sync_component_theme(
        app.theme(),
        &mut app.applied_component_theme,
        window,
        cx,
    );
    settings::sync_controls(app, window, cx);
    inspector::sync_controls(app, window, cx);

    if let Some(message) = app.data.error.take() {
        window.push_notification(
            Notification::error(message)
                .title("OneChat")
                .id::<AppErrorNotification>()
                .autohide(false),
            cx,
        );
    }

    if let Some(pending) = app.navigation.pending_focus.take() {
        if pending == PendingFocus::Root {
            window.focus(&app.root_focus, cx);
        }
        let focus = match pending {
            PendingFocus::Root => None,
            PendingFocus::ConversationSearch => {
                Some(app.sidebar.search_input.read(cx).focus_handle(cx))
            }
            PendingFocus::SystemPrompt => app
                .chat
                .system_prompt_editor
                .as_ref()
                .map(|input| input.read(cx).focus_handle(cx)),
            PendingFocus::SettingsPrompt => {
                if let Some(editor) = &app.settings_ui.prompt_preset_editor {
                    Some(editor.focus_input().read(cx).focus_handle(cx))
                } else {
                    app.settings_ui
                        .title_prompt_editor
                        .as_ref()
                        .map(|input| input.read(cx).focus_handle(cx))
                }
            }
            PendingFocus::MessageEditor => app
                .active_message_editor()
                .map(|input| input.read(cx).focus_handle(cx)),
            PendingFocus::Composer if app.navigation.page == Page::Chat => {
                Some(app.chat.composer.read(cx).focus_handle(cx))
            }
            PendingFocus::Composer => None,
        };
        if let Some(focus) = focus {
            window.focus(&focus, cx);
        }
    }

    let animated_title = if app.navigation.page == Page::Chat {
        app.advance_message_scroll(window);
        app.current_animated_title(window)
    } else {
        None
    };

    let scale_factor = window.scale_factor();
    let inspector_progress = app
        .navigation
        .inspector_motion
        .progress(window, cx.reduce_motion());
    let sidebar_progress = app
        .navigation
        .sidebar_motion
        .progress(window, cx.reduce_motion());
    let jump_to_latest_visible = app.navigation.page == Page::Chat
        && app.current_conversation().is_some()
        && !app.chat.follow_latest
        && !app.chat.message_scroll_motion.is_active();
    app.chat
        .jump_to_latest_motion
        .set_visible(jump_to_latest_visible);
    let jump_to_latest_progress = app.chat.jump_to_latest_motion.progress(window);
    let sidebar_width = SIDEBAR_WIDTH * sidebar_progress;
    let chat_available_width = (f32::from(window.bounds().size.width) - sidebar_width).max(0.0);
    let sidebar = (app.navigation.page == Page::Chat && sidebar_width > 0.01).then(|| {
        div()
            .w(px(sidebar_width))
            .h_full()
            .flex_none()
            .overflow_hidden()
            .child(render_sidebar(app, animated_title.as_deref(), cx))
    });
    let top_bar = render_top_bar(app, animated_title.as_deref(), cx);
    let page = match app.navigation.page {
        Page::Chat => render_chat_page(
            app,
            chat_available_width,
            scale_factor,
            jump_to_latest_progress,
            cx,
        ),
        Page::Settings => settings::render(app, cx),
    };
    let inspector = (app.navigation.page == Page::Chat
        && (app.navigation.inspector_open || inspector_progress > 0.0))
        .then(|| {
            translated_x(
                inspector::render(app, cx),
                px(368.0 * (1.0 - inspector_progress)),
            )
        });

    div()
        .relative()
        .size_full()
        .flex()
        .track_focus(&app.root_focus)
        .key_context("OneChat")
        .on_action(cx.listener(|this, _: &NewConversation, _, cx| this.create_conversation(cx)))
        .on_action(cx.listener(|this, _: &ShowCommandPalette, window, cx| {
            this.open_command_palette(window, cx)
        }))
        .on_action(
            cx.listener(|this, _: &ShowModelPicker, window, cx| this.open_model_picker(window, cx)),
        )
        .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| this.toggle_sidebar(cx)))
        .on_action(cx.listener(|this, _: &OpenSettings, _, cx| this.set_page(Page::Settings, cx)))
        .on_action(
            cx.listener(|this, _: &DismissOverlay, window, cx| this.dismiss_overlay(window, cx)),
        )
        .bg(cx
            .theme()
            .background
            .alpha(app.settings().background_opacity()))
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
                .child(
                    div()
                        .relative()
                        .min_h_0()
                        .flex_1()
                        .flex()
                        .child(page)
                        .children(inspector),
                ),
        )
        .into_any_element()
}

fn render_top_bar(
    app: &OneChat,
    animated_title: Option<&str>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if app.navigation.page == Page::Settings {
        return div()
            .h(px(60.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px_5()
            .bg(cx.theme().title_bar)
            .shadow_xs()
            .child(
                div()
                    .min_w_0()
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .text_size(px(14.0))
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("Settings"),
            )
            .child(
                large_icon_button("chat-page", AppIcon::Close, IconTone::Muted, cx)
                    .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Chat, cx))),
            )
            .into_any_element();
    }

    let current_conversation = app.current_conversation();
    let inspector_open = app.navigation.inspector_open;
    let title = animated_title.map(str::to_string).unwrap_or_else(|| {
        current_conversation
            .map(|conversation| conversation.title.clone())
            .unwrap_or_else(|| "OneChat".into())
    });
    let title_waiting = current_conversation
        .is_some_and(|conversation| conversation.auto_title_state == AutoTitleState::Running);
    let title_animation_id: SharedString = current_conversation.map_or_else(
        || "waiting-top-bar-title".into(),
        |conversation| format!("waiting-top-bar-title-{}", conversation.id).into(),
    );
    let selected_model = app.selected_model();
    let provider = selected_model.and_then(|model| app.provider_for_model(model));
    let provider_name = provider
        .map(|provider| provider.name.clone())
        .unwrap_or_else(|| "No provider".into());
    let model_label = selected_model
        .map(|model| model.display_name.clone())
        .unwrap_or_else(|| "Choose Model".into());
    let prompt_label = current_conversation
        .map(|conversation| app.system_prompt_label(&conversation.system_prompt))
        .unwrap_or_else(|| "None".into());
    let can_choose_prompt = current_conversation.is_some() && !app.is_current_generating();
    let (connection, connection_color) = provider.map_or(
        ("Not configured", cx.theme().muted_foreground),
        |provider| match app.settings_ui.connection_tests.get(&provider.id) {
            Some(ConnectionTestStatus::Testing) => ("Testing", cx.theme().primary),
            Some(ConnectionTestStatus::Connected) => ("Connected", cx.theme().success),
            Some(ConnectionTestStatus::Failed(_)) => ("Connection failed", cx.theme().danger),
            None if provider.enabled => ("Ready", cx.theme().success),
            None => ("Disabled", cx.theme().danger),
        },
    );

    div()
        .h(px(60.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .px_4()
        .bg(cx.theme().title_bar)
        .shadow_xs()
        .when(app.settings().sidebar_collapsed, |this| {
            this.child(
                large_icon_button("expand-sidebar", AppIcon::Sidebar, IconTone::Muted, cx)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
            )
        })
        .child(
            div()
                .min_w_0()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .justify_center()
                .child(waiting_title(
                    div()
                        .max_w(px(400.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                    title_animation_id,
                    title_waiting,
                ))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(div().size(px(6.0)).rounded_full().bg(connection_color))
                        .child(format!("{provider_name} · {connection}")),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    button_base("open-model-picker")
                        .large()
                        .h(px(40.0))
                        .px(px(14.0))
                        .rounded(px(12.0))
                        .tooltip("Choose model")
                        .max_w(px(240.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(model_label),
                        )
                        .child(render_icon(AppIcon::ChevronDown, IconTone::Muted, 14.0, cx))
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_model_picker(window, cx)),
                        ),
                )
                .child(
                    button_base("open-prompt-picker")
                        .large()
                        .h(px(40.0))
                        .px(px(14.0))
                        .rounded(px(12.0))
                        .tooltip("Choose system prompt")
                        .disabled(!can_choose_prompt)
                        .max_w(px(190.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .on_click(
                            cx.listener(|this, _, window, cx| this.open_prompt_picker(window, cx)),
                        )
                        .child(render_icon(AppIcon::Command, IconTone::Muted, 14.0, cx))
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(prompt_label),
                        )
                        .child(render_icon(AppIcon::ChevronDown, IconTone::Muted, 14.0, cx)),
                )
                .child(
                    large_icon_button(
                        "toggle-inspector",
                        AppIcon::Info,
                        if inspector_open {
                            IconTone::Accent
                        } else {
                            IconTone::Muted
                        },
                        cx,
                    )
                    .selected(inspector_open)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_inspector_visible(!inspector_open, cx)
                    })),
                ),
        )
        .into_any_element()
}

fn render_chat_page(
    app: &OneChat,
    available_width: f32,
    scale_factor: f32,
    jump_to_latest_progress: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let content = if app.data.loading {
        empty_state(
            "Loading your conversations",
            "Opening the local OneChat library…",
            None,
            cx,
        )
    } else if app.data.snapshot.providers.is_empty() {
        empty_state(
            "Connect a provider",
            "Add OpenAI, Anthropic, Gemini, or an OpenAI-compatible provider to get started.",
            Some(("Open Settings", Page::Settings)),
            cx,
        )
    } else if !app
        .data
        .snapshot
        .models
        .iter()
        .any(|model| app.model_availability(model).is_ok())
    {
        empty_state(
            "Add your first model",
            "Choose a remote model ID for one of your configured providers.",
            Some(("Manage Models", Page::Settings)),
            cx,
        )
    } else if app.data.snapshot.conversations.is_empty() {
        return empty_state(
            "What would you like to explore?",
            "Conversations and credentials stay on this Mac.",
            Some(("New Conversation", Page::Chat)),
            cx,
        );
    } else if app.current_conversation().is_none() {
        empty_state(
            "Choose a conversation",
            "Select one from the sidebar or start a new conversation.",
            None,
            cx,
        )
    } else {
        chat::render(
            app,
            available_width,
            scale_factor,
            jump_to_latest_progress,
            cx,
        )
    };

    div()
        .min_w_0()
        .flex_1()
        .h_full()
        .child(content)
        .into_any_element()
}

fn empty_state(
    title: &'static str,
    detail: &'static str,
    action: Option<(&'static str, Page)>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let action = action.map(|(label, page)| {
        if label == "New Conversation" {
            primary_icon_button("empty-new-conversation", AppIcon::Plus, cx)
                .on_click(cx.listener(|this, _, _, cx| this.create_conversation(cx)))
        } else {
            primary_icon_button(
                "empty-state-action",
                if label == "Manage Models" {
                    AppIcon::Layers
                } else {
                    AppIcon::Settings
                },
                cx,
            )
            .on_click(cx.listener(move |this, _, _, cx| this.set_page(page, cx)))
        }
    });
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .p_8()
        .child(
            div()
                .max_w(px(500.0))
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .text_center()
                .child(
                    div()
                        .size(px(52.0))
                        .rounded_full()
                        .bg(cx.theme().accent)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xl()
                        .text_color(cx.theme().primary)
                        .child("✦"),
                )
                .child(
                    div()
                        .pt_2()
                        .text_size(px(24.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .max_w(px(440.0))
                        .line_height(px(22.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                )
                .children(action),
        )
        .into_any_element()
}
