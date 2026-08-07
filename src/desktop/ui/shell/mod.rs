mod dialogs;
mod pickers;
mod sidebar;

use dialogs::{animated_overlay, render_destructive_confirmation};
use pickers::{render_command_palette, render_model_picker, render_prompt_picker};
use sidebar::{COLLAPSED_SIDEBAR_WIDTH, EXPANDED_SIDEBAR_WIDTH, render_sidebar};

use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Context, Div, FontWeight, KeyBinding,
    SharedString, Window, actions, div, ease_out_quint, prelude::*, px, rgba,
};

use super::{
    components::{
        IconTone, UiIcon, button_base, compact_button, icon_button, large_svg_icon_button,
        primary_button, primary_button_base, primary_svg_icon_button, svg_icon, svg_icon_button,
    },
    motion::{translated_x, waiting_title},
    theme::Colors,
};

use crate::{
    desktop::app::{
        ConnectionTestStatus, DestructiveAction, OneChat, Page, PaletteCommand, PendingFocus,
    },
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

fn shortcut_label(key: &str) -> String {
    if cfg!(target_os = "macos") {
        format!("⌘{key}")
    } else {
        format!("Ctrl+{key}")
    }
}

pub fn render(app: &mut OneChat, window: &mut Window, cx: &mut Context<OneChat>) -> AnyElement {
    if let Some(pending) = app.navigation.pending_focus.take() {
        if pending == PendingFocus::Root {
            window.focus(&app.root_focus);
        }
        let input = match pending {
            PendingFocus::Root => None,
            PendingFocus::CommandPalette => Some(app.overlays.command_input.clone()),
            PendingFocus::ModelPicker => Some(app.overlays.model_search_input.clone()),
            PendingFocus::ConversationSearch => Some(app.sidebar.search_input.clone()),
            PendingFocus::SystemPrompt => app.chat.system_prompt_editor.clone(),
            PendingFocus::SettingsPrompt => app
                .settings_ui
                .prompt_preset_editor
                .as_ref()
                .map(|editor| editor.focus_input())
                .or_else(|| app.settings_ui.title_prompt_editor.clone()),
            PendingFocus::MessageEditor => app.active_message_editor(),
            PendingFocus::Composer if app.navigation.page == Page::Chat => {
                Some(app.chat.composer.clone())
            }
            PendingFocus::Composer => None,
        };
        if let Some(input) = input {
            window.focus(&input.read(cx).focus_handle(cx));
        }
    }

    let animated_title = if app.navigation.page == Page::Chat {
        app.advance_message_scroll(window);
        app.current_animated_title(window)
    } else {
        None
    };

    let colors = Colors::for_theme(app.theme(), window.appearance());
    let scale_factor = window.scale_factor();
    let inspector_progress = app.navigation.inspector_motion.progress(window);
    let sidebar_width = if app.settings().sidebar_collapsed {
        COLLAPSED_SIDEBAR_WIDTH
    } else {
        EXPANDED_SIDEBAR_WIDTH
    };
    let chat_available_width = (f32::from(window.bounds().size.width) - sidebar_width).max(0.0);
    let sidebar = (app.navigation.page == Page::Chat)
        .then(|| render_sidebar(app, animated_title.as_deref(), colors, scale_factor, cx));
    let top_bar = render_top_bar(app, animated_title.as_deref(), colors, scale_factor, cx);
    let page = match app.navigation.page {
        Page::Chat => render_chat_page(app, chat_available_width, colors, scale_factor, cx),
        Page::Settings => settings::render(app, colors, scale_factor, cx),
    };
    let inspector = (app.navigation.page == Page::Chat
        && (app.navigation.inspector_open || inspector_progress > 0.0))
        .then(|| {
            translated_x(
                inspector::render(app, colors, scale_factor, cx),
                px(340.0 * (1.0 - inspector_progress)),
            )
        });
    let command_palette = app
        .overlays
        .command_palette_open
        .then(|| render_command_palette(app, colors, cx));
    let model_picker = app
        .overlays
        .model_picker_open
        .then(|| render_model_picker(app, colors, cx));
    let prompt_picker = app
        .overlays
        .prompt_picker_open
        .then(|| render_prompt_picker(app, colors, cx));
    let prompt_preset_panel = (app.settings_ui.prompt_preset_editor.is_some()
        || app.settings_ui.viewed_prompt_preset.is_some())
    .then(|| {
        animated_overlay(
            settings::prompt_preset_panel(app, colors, scale_factor, cx),
            colors,
            "prompt-preset-backdrop",
            "prompt-preset-panel",
        )
    });
    let destructive_confirmation = app
        .overlays
        .destructive_action
        .as_ref()
        .map(|action| render_destructive_confirmation(action, colors, scale_factor, cx));
    let error = app.data.error.clone().map(|message| {
        div()
            .absolute()
            .top(px(66.0))
            .left(px(16.0))
            .right(px(16.0))
            .rounded_xl()
            .border_1()
            .border_color(if colors.dark {
                rgba(0xff453a66)
            } else {
                rgba(0xd7001538)
            })
            .bg(if colors.dark {
                rgba(0x3a2020f5)
            } else {
                rgba(0xfff2f2f5)
            })
            .shadow_lg()
            .px_4()
            .py_3()
            .flex()
            .items_center()
            .justify_between()
            .gap_4()
            .text_sm()
            .text_color(colors.danger)
            .child(message)
            .child(
                compact_button("dismiss-error", "Dismiss", colors)
                    .text_color(colors.danger)
                    .on_click(cx.listener(|this, _, _, cx| this.dismiss_error(cx))),
            )
    });

    div()
        .relative()
        .size_full()
        .flex()
        .track_focus(&app.root_focus)
        .key_context("OneChat")
        .on_action(cx.listener(|this, _: &NewConversation, _, cx| {
            this.overlays.command_palette_open = false;
            this.overlays.model_picker_open = false;
            this.overlays.prompt_picker_open = false;
            this.create_conversation(cx);
        }))
        .on_action(cx.listener(|this, _: &ShowCommandPalette, _, cx| this.open_command_palette(cx)))
        .on_action(cx.listener(|this, _: &ShowModelPicker, _, cx| this.open_model_picker(cx)))
        .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| this.toggle_sidebar(cx)))
        .on_action(cx.listener(|this, _: &OpenSettings, _, cx| this.set_page(Page::Settings, cx)))
        .on_action(cx.listener(|this, _: &DismissOverlay, _, cx| this.dismiss_overlay(cx)))
        .bg(colors.canvas)
        .text_color(colors.text)
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
                )
                .children(error),
        )
        .children(command_palette)
        .children(model_picker)
        .children(prompt_picker)
        .children(prompt_preset_panel)
        .children(destructive_confirmation)
        .into_any_element()
}

fn render_top_bar(
    app: &OneChat,
    animated_title: Option<&str>,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if app.navigation.page == Page::Settings {
        return div()
            .h(px(56.0))
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .px_5()
            .border_b_1()
            .border_color(colors.border)
            .bg(colors.toolbar)
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
                large_svg_icon_button(
                    "chat-page",
                    UiIcon::Close,
                    IconTone::Muted,
                    colors,
                    scale_factor,
                )
                .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Chat, cx))),
            )
            .into_any_element();
    }

    let current_conversation = app.current_conversation();
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
        .map(|model| format!("{} · {provider_name}", model.display_name))
        .unwrap_or_else(|| "Choose Model".into());
    let prompt_label = current_conversation
        .map(|conversation| app.system_prompt_label(&conversation.system_prompt))
        .unwrap_or_else(|| "None".into());
    let can_choose_prompt = current_conversation.is_some() && !app.is_current_generating();
    let (connection, connection_color) =
        provider.map_or(("Not configured", colors.muted), |provider| {
            match app.settings_ui.connection_tests.get(&provider.id) {
                Some(ConnectionTestStatus::Testing) => ("Testing", colors.accent),
                Some(ConnectionTestStatus::Connected) => ("Connected", colors.success),
                Some(ConnectionTestStatus::Failed(_)) => ("Connection failed", colors.danger),
                None if provider.enabled => ("Ready", colors.success),
                None => ("Disabled", colors.danger),
            }
        });

    div()
        .h(px(56.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .px_5()
        .border_b_1()
        .border_color(colors.border)
        .bg(colors.toolbar)
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
                        .max_w(px(340.0))
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
                        .text_color(colors.muted)
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
                    button_base("open-model-picker", colors)
                        .max_w(px(300.0))
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
                        .child(svg_icon(
                            UiIcon::ChevronDown,
                            IconTone::Muted,
                            colors,
                            scale_factor,
                            14.0,
                        ))
                        .on_click(cx.listener(|this, _, _, cx| this.open_model_picker(cx))),
                )
                .child(
                    button_base("open-prompt-picker", colors)
                        .max_w(px(220.0))
                        .flex()
                        .items_center()
                        .gap_2()
                        .when(app.overlays.prompt_picker_open, |element| {
                            element.bg(colors.accent_soft)
                        })
                        .when(can_choose_prompt, |element| {
                            element
                                .on_click(cx.listener(|this, _, _, cx| this.open_prompt_picker(cx)))
                        })
                        .when(!can_choose_prompt, |element| element.opacity(0.5))
                        .child(
                            div()
                                .min_w_0()
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(format!("Prompt · {prompt_label}")),
                        )
                        .child(svg_icon(
                            UiIcon::ChevronDown,
                            IconTone::Muted,
                            colors,
                            scale_factor,
                            14.0,
                        )),
                )
                .child(
                    compact_button("open-command-palette", shortcut_label("K"), colors)
                        .text_color(colors.muted)
                        .on_click(cx.listener(|this, _, _, cx| this.open_command_palette(cx))),
                )
                .child(
                    icon_button("toggle-inspector", "ⓘ", colors)
                        .when(app.navigation.inspector_open, |element| {
                            element.bg(colors.accent_soft).text_color(colors.accent)
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_inspector(cx))),
                ),
        )
        .into_any_element()
}

fn render_chat_page(
    app: &OneChat,
    available_width: f32,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let content = if app.data.loading {
        empty_state(
            "Loading your conversations",
            "Opening the local OneChat library…",
            None,
            colors,
            cx,
        )
    } else if app.data.snapshot.providers.is_empty() {
        empty_state(
            "Connect a provider",
            "Add OpenAI, Anthropic, Gemini, or an OpenAI-compatible provider to get started.",
            Some(("Open Settings", Page::Settings)),
            colors,
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
            colors,
            cx,
        )
    } else if app.data.snapshot.conversations.is_empty() {
        return empty_state(
            "What would you like to explore?",
            "Conversations and credentials stay on this Mac.",
            Some(("New Conversation", Page::Chat)),
            colors,
            cx,
        );
    } else if app.current_conversation().is_none() {
        empty_state(
            "Choose a conversation",
            "Select one from the sidebar or start a new conversation.",
            None,
            colors,
            cx,
        )
    } else {
        chat::render(app, available_width, colors, scale_factor, cx)
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
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let action = action.map(|(label, page)| {
        if label == "New Conversation" {
            primary_button("empty-new-conversation", label, colors)
                .on_click(cx.listener(|this, _, _, cx| this.create_conversation(cx)))
        } else {
            primary_button("empty-state-action", label, colors)
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
                        .bg(colors.accent_soft)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_xl()
                        .text_color(colors.accent)
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
                        .text_color(colors.muted)
                        .child(detail),
                )
                .children(action),
        )
        .into_any_element()
}
