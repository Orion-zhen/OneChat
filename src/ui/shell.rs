use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Context, Div, ElementId, FontWeight, KeyBinding,
    Rgba, SharedString, Stateful, Window, WindowAppearance, actions, div, ease_out_quint,
    prelude::*, px, rgb, rgba,
};

use crate::{
    app::{ConnectionTestStatus, OneChat, PaletteCommand, PendingFocus},
    model::{Conversation, Page, Theme},
    ui::{chat, inspector, settings},
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

#[derive(Clone, Copy)]
pub(crate) struct Colors {
    pub(crate) canvas: Rgba,
    pub(crate) panel: Rgba,
    pub(crate) raised: Rgba,
    pub(crate) text: Rgba,
    pub(crate) muted: Rgba,
    pub(crate) border: Rgba,
    pub(crate) accent: Rgba,
    pub(crate) accent_soft: Rgba,
    pub(crate) danger: Rgba,
    pub(crate) dark: bool,
}

impl Colors {
    fn for_theme(theme: Theme, appearance: WindowAppearance) -> Self {
        let dark = theme == Theme::Dark
            || (theme == Theme::System
                && matches!(
                    appearance,
                    WindowAppearance::Dark | WindowAppearance::VibrantDark
                ));
        if dark {
            Self {
                canvas: rgb(0x15171a),
                panel: rgb(0x1d2024),
                raised: rgb(0x25292e),
                text: rgb(0xf3f4f6),
                muted: rgb(0x9ca3af),
                border: rgb(0x343941),
                accent: rgb(0x7aa7ff),
                accent_soft: rgb(0x263754),
                danger: rgb(0xff8b8b),
                dark: true,
            }
        } else {
            Self {
                canvas: rgb(0xf5f6f8),
                panel: rgb(0xffffff),
                raised: rgb(0xf0f2f5),
                text: rgb(0x202124),
                muted: rgb(0x747b87),
                border: rgb(0xdfe2e7),
                accent: rgb(0x2563eb),
                accent_soft: rgb(0xe8f0ff),
                danger: rgb(0xb42318),
                dark: false,
            }
        }
    }
}

pub fn render(app: &mut OneChat, window: &mut Window, cx: &mut Context<OneChat>) -> AnyElement {
    if let Some(pending) = app.pending_focus.take() {
        let input = match pending {
            PendingFocus::CommandPalette => Some(app.command_input.clone()),
            PendingFocus::ModelPicker => Some(app.model_search_input.clone()),
            PendingFocus::ConversationSearch => Some(app.search_input.clone()),
            PendingFocus::SystemPrompt => app.system_prompt_editor.clone(),
            PendingFocus::DefaultSystemPrompt => app.default_system_prompt_editor.clone(),
            PendingFocus::Composer if app.page == Page::Chat => Some(app.composer.clone()),
            PendingFocus::Composer => None,
        };
        if let Some(input) = input {
            window.focus(&input.read(cx).focus_handle(cx));
        }
    }

    let colors = Colors::for_theme(app.theme(), window.appearance());
    let narrow = window.viewport_size().width < px(1180.0);
    let sidebar = render_sidebar(app, colors, cx);
    let top_bar = render_top_bar(app, colors, cx);
    let page = match app.page {
        Page::Chat => render_chat_page(app, colors, window.scale_factor(), cx),
        Page::Settings => settings::render(app, colors, cx),
    };

    let inspector = app
        .inspector_open
        .then(|| inspector::render(app, colors, narrow, cx));
    let command_palette = app
        .command_palette_open
        .then(|| render_command_palette(app, colors, cx));
    let model_picker = app
        .model_picker_open
        .then(|| render_model_picker(app, colors, cx));
    let error = app.error.clone().map(|message| {
        div()
            .flex_none()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px_4()
            .py_2()
            .bg(if colors.dark {
                rgb(0x512c2c)
            } else {
                rgb(0xffe9e7)
            })
            .text_color(colors.danger)
            .text_sm()
            .child(message)
            .child(
                button("dismiss-error", "Dismiss", colors)
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
            this.command_palette_open = false;
            this.model_picker_open = false;
            this.create_conversation(cx);
        }))
        .on_action(cx.listener(|this, _: &ShowCommandPalette, _, cx| this.open_command_palette(cx)))
        .on_action(cx.listener(|this, _: &ShowModelPicker, _, cx| this.open_model_picker(cx)))
        .on_action(cx.listener(|this, _: &ToggleSidebar, _, cx| this.toggle_sidebar(cx)))
        .on_action(cx.listener(|this, _: &OpenSettings, _, cx| this.set_page(Page::Settings, cx)))
        .on_action(cx.listener(|this, _: &DismissOverlay, _, cx| this.dismiss_overlay(cx)))
        .bg(colors.canvas)
        .text_color(colors.text)
        .child(sidebar)
        .child(
            div()
                .min_w_0()
                .flex_1()
                .h_full()
                .flex()
                .flex_col()
                .child(top_bar)
                .children(error)
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
        .children(command_palette)
        .children(model_picker)
        .into_any_element()
}

fn render_sidebar(app: &mut OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    if app.settings().sidebar_collapsed {
        let sidebar = div()
            .w(px(52.0))
            .h_full()
            .flex_none()
            .flex()
            .flex_col()
            .items_center()
            .gap_3()
            .py_3()
            .border_r_1()
            .border_color(colors.border)
            .bg(colors.panel)
            .child(
                icon_button("expand-sidebar", "☰", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
            )
            .child(
                icon_button("new-conversation-collapsed", "+", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.create_conversation(cx))),
            )
            .child(div().flex_1())
            .child(
                icon_button("settings-collapsed", "⚙", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Settings, cx))),
            );
        return animate_sidebar(sidebar, true, app.settings().reduce_motion);
    }

    let groups = app.conversation_groups();
    let current_id = app
        .settings()
        .current_conversation_id
        .as_deref()
        .map(str::to_owned);
    let mut list = div()
        .id("conversation-list")
        .min_h_0()
        .flex_1()
        .overflow_y_scroll()
        .px_3()
        .pb_3();
    if groups.is_empty() {
        list = list.child(div().p_3().text_sm().text_color(colors.muted).child(
            if app.search_query.trim().is_empty() {
                "No conversations yet"
            } else {
                "No matching conversations"
            },
        ));
    } else {
        for (group, conversations) in groups {
            list = list.child(
                div()
                    .pt_4()
                    .pb_2()
                    .px_2()
                    .text_xs()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.muted)
                    .child(group.label()),
            );
            for conversation in conversations {
                list = list.child(render_conversation_row(
                    app,
                    conversation,
                    current_id.as_deref(),
                    colors,
                    cx,
                ));
            }
        }
    }

    let sidebar = div()
        .w(px(248.0))
        .h_full()
        .flex_none()
        .flex()
        .flex_col()
        .border_r_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .child(
            div()
                .h(px(58.0))
                .flex_none()
                .flex()
                .items_center()
                .justify_between()
                .px_4()
                .child(div().font_weight(FontWeight::SEMIBOLD).child("OneChat"))
                .child(
                    icon_button("collapse-sidebar", "‹", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                ),
        )
        .child(
            div()
                .px_3()
                .pb_3()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    button("new-conversation", "+  New conversation", colors)
                        .w_full()
                        .on_click(cx.listener(|this, _, _, cx| this.create_conversation(cx))),
                )
                .child(app.search_input.clone()),
        )
        .child(list)
        .child(render_connection_footer(app, colors, cx));
    animate_sidebar(sidebar, false, app.settings().reduce_motion)
}

fn animate_sidebar(sidebar: Div, collapsed: bool, reduce_motion: bool) -> AnyElement {
    let id = if collapsed {
        "sidebar-collapsed"
    } else {
        "sidebar-expanded"
    };
    let duration = if reduce_motion { 160 } else { 200 };
    sidebar
        .overflow_hidden()
        .with_animation(
            id,
            Animation::new(Duration::from_millis(duration)).with_easing(ease_out_quint()),
            move |sidebar, delta| {
                let sidebar = sidebar.opacity(0.82 + delta * 0.18);
                if reduce_motion {
                    sidebar
                } else {
                    let width = if collapsed {
                        248.0 - 196.0 * delta
                    } else {
                        52.0 + 196.0 * delta
                    };
                    sidebar.w(px(width))
                }
            },
        )
        .into_any_element()
}

fn render_conversation_row(
    app: &OneChat,
    conversation: Conversation,
    current_id: Option<&str>,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    if let Some(input) = app.rename_input(&conversation.id) {
        return div()
            .mb_1()
            .rounded_lg()
            .bg(colors.raised)
            .p_2()
            .child(input)
            .into_any_element();
    }

    let selected = current_id == Some(conversation.id.as_str());
    let select_id = conversation.id.clone();
    let pin_id = conversation.id.clone();
    let rename_id = conversation.id.clone();
    let delete_id = conversation.id.clone();
    let row_id: SharedString = format!("conversation-{}", conversation.id).into();

    div()
        .id(row_id)
        .mb_1()
        .rounded_lg()
        .border_1()
        .border_color(if selected {
            colors.accent
        } else {
            colors.panel
        })
        .bg(if selected {
            colors.accent_soft
        } else {
            colors.panel
        })
        .p_2()
        .child(
            div()
                .id(SharedString::from(format!("select-{}", conversation.id)))
                .w_full()
                .cursor_pointer()
                .text_sm()
                .font_weight(if selected {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_conversation(select_id.clone(), cx)
                }))
                .child(conversation.title),
        )
        .child(
            div()
                .pt_2()
                .flex()
                .items_center()
                .gap_2()
                .text_xs()
                .text_color(colors.muted)
                .child(
                    compact_button(
                        SharedString::from(format!("pin-{}", pin_id)),
                        if conversation.pinned { "Unpin" } else { "Pin" },
                        colors,
                    )
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.toggle_pin(pin_id.clone(), cx)),
                    ),
                )
                .child(
                    compact_button(
                        SharedString::from(format!("rename-{}", rename_id)),
                        "Rename",
                        colors,
                    )
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.start_rename(rename_id.clone(), window, cx)
                    })),
                )
                .child(
                    compact_button(
                        SharedString::from(format!("delete-{}", delete_id)),
                        "Delete",
                        colors,
                    )
                    .text_color(colors.danger)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.delete_conversation(delete_id.clone(), cx)
                    })),
                ),
        )
        .into_any_element()
}

fn render_connection_footer(
    app: &OneChat,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let enabled = app
        .snapshot
        .providers
        .iter()
        .filter(|provider| provider.enabled)
        .count();
    div()
        .flex_none()
        .border_t_1()
        .border_color(colors.border)
        .p_3()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child(format!("{enabled} providers configured")),
        )
        .child(
            icon_button("open-settings", "⚙", colors)
                .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Settings, cx))),
        )
        .into_any_element()
}

fn render_top_bar(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let title = match app.page {
        Page::Settings => "Settings".to_string(),
        Page::Chat => app
            .current_conversation()
            .map(|conversation| conversation.title.clone())
            .unwrap_or_else(|| "OneChat".into()),
    };
    let model = app
        .current_model()
        .map(|model| model.display_name.clone())
        .unwrap_or_else(|| "No model".into());
    let provider = app.current_provider();
    let provider_name = provider
        .map(|provider| provider.name.clone())
        .unwrap_or_else(|| "No provider".into());
    let (connection, connection_color) =
        provider.map_or(("Not configured", colors.muted), |provider| {
            match app.connection_tests.get(&provider.id) {
                Some(ConnectionTestStatus::Testing) => ("Testing", colors.accent),
                Some(ConnectionTestStatus::Connected) => ("Connected", rgb(0x22a06b)),
                Some(ConnectionTestStatus::Failed(_)) => ("Connection failed", colors.danger),
                None if provider.enabled => ("Configured", colors.muted),
                None => ("Disabled", colors.danger),
            }
        });
    let has_system_prompt = app
        .current_conversation()
        .is_some_and(|conversation| !conversation.system_prompt.content.trim().is_empty());

    div()
        .h(px(58.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .px_4()
        .border_b_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    icon_button("top-toggle-sidebar", "☰", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_sidebar(cx))),
                )
                .child(
                    div()
                        .min_w_0()
                        .max_w(px(190.0))
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .children(has_system_prompt.then(|| {
                    div()
                        .flex_none()
                        .rounded_md()
                        .bg(colors.accent_soft)
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(colors.accent)
                        .child("System")
                }))
                .child(
                    button(
                        "open-model-picker",
                        format!("{model} · {provider_name}"),
                        colors,
                    )
                    .min_w_0()
                    .max_w(px(250.0))
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .on_click(cx.listener(|this, _, _, cx| this.open_model_picker(cx))),
                )
                .child(
                    div()
                        .flex_none()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_xs()
                        .text_color(connection_color)
                        .child(div().size_2().rounded_full().bg(connection_color))
                        .child(connection),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .children((app.page == Page::Settings).then(|| {
                    button("chat-page", "Chat", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Chat, cx)))
                }))
                .child(
                    button("open-command-palette", shortcut_label("K"), colors)
                        .on_click(cx.listener(|this, _, _, cx| this.open_command_palette(cx))),
                )
                .child(
                    button("toggle-inspector", "Inspector", colors)
                        .when(app.inspector_open, |element| {
                            element.bg(colors.accent_soft).border_color(colors.accent)
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_inspector(cx))),
                )
                .child(
                    icon_button("top-settings", "⚙", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Settings, cx))),
                ),
        )
        .into_any_element()
}

fn render_chat_page(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let content = if app.loading {
        empty_state(
            "Loading local data…",
            "Opening OneChat's SQLite database.",
            None,
            colors,
            cx,
        )
    } else if app.snapshot.providers.is_empty() {
        empty_state(
            "No provider configured",
            "Add OpenAI, Anthropic, Gemini, or an OpenAI-compatible provider to begin.",
            Some(("Open settings", Page::Settings)),
            colors,
            cx,
        )
    } else if !app
        .snapshot
        .models
        .iter()
        .any(|model| app.model_availability(model).is_ok())
    {
        empty_state(
            "Your provider has no models",
            "Add at least one remote model ID before creating a conversation.",
            Some(("Manage models", Page::Settings)),
            colors,
            cx,
        )
    } else if app.snapshot.conversations.is_empty() {
        return div()
            .min_w_0()
            .flex_1()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .max_w(px(480.0))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_3()
                    .text_center()
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Ready for a new conversation"),
                    )
                    .child(
                        div()
                            .text_color(colors.muted)
                            .child("Provider and model data are stored locally."),
                    )
                    .child(
                        button("empty-new-conversation", "New conversation", colors)
                            .on_click(cx.listener(|this, _, _, cx| this.create_conversation(cx))),
                    ),
            )
            .into_any_element();
    } else if app.current_conversation().is_none() {
        empty_state(
            "Choose a conversation",
            "Select an item in the sidebar or create a new conversation.",
            None,
            colors,
            cx,
        )
    } else {
        chat::render(app, colors, scale_factor, cx)
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
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .max_w(px(520.0))
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .text_center()
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(div().text_color(colors.muted).child(detail))
                .children(action.map(|(label, page)| {
                    button("empty-state-action", label, colors)
                        .on_click(cx.listener(move |this, _, _, cx| this.set_page(page, cx)))
                })),
        )
        .into_any_element()
}

fn render_command_palette(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let commands = app.filtered_commands();
    let mut rows = div()
        .id("command-palette-list")
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(&app.command_scroll)
        .flex()
        .flex_col()
        .gap_1();
    if commands.is_empty() {
        rows = rows.child(
            div()
                .p_4()
                .text_sm()
                .text_color(colors.muted)
                .child("No matching commands."),
        );
    } else {
        for (index, command) in commands.into_iter().enumerate() {
            let shortcut = command_shortcut(command);
            rows = rows.child(
                div()
                    .id(SharedString::from(format!("command-{command:?}")))
                    .rounded_lg()
                    .border_1()
                    .border_color(if index == app.command_selection {
                        colors.accent
                    } else {
                        colors.panel
                    })
                    .bg(if index == app.command_selection {
                        colors.accent_soft
                    } else {
                        colors.panel
                    })
                    .p_3()
                    .cursor_pointer()
                    .hover(move |style| style.bg(colors.raised))
                    .on_click(cx.listener(move |this, _, _, cx| this.execute_command(command, cx)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_4()
                            .child(
                                div()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(command.label()),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(colors.muted)
                                            .child(command.detail()),
                                    ),
                            )
                            .children(shortcut.map(|shortcut| {
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(colors.muted)
                                    .child(shortcut)
                            })),
                    ),
            );
        }
    }

    let panel = div()
        .w_full()
        .max_w(px(560.0))
        .max_h(px(560.0))
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .shadow_lg()
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Command Palette"),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(colors.muted)
                                .child("↑↓ · Enter · Esc"),
                        )
                        .child(button("close-command-palette", "Close", colors).on_click(
                            cx.listener(|this, _, _, cx| this.close_command_palette(cx)),
                        )),
                ),
        )
        .child(app.command_input.clone())
        .child(rows);
    animated_overlay(
        panel,
        "command-palette-backdrop",
        "command-palette-panel",
        app.settings().reduce_motion,
    )
}

fn command_shortcut(command: PaletteCommand) -> Option<String> {
    match command {
        PaletteCommand::NewConversation => Some(shortcut_label("N")),
        PaletteCommand::ChooseModel => Some(shortcut_label("L")),
        PaletteCommand::ToggleSidebar => Some(if cfg!(target_os = "macos") {
            "⇧⌘S".into()
        } else {
            "Ctrl+Shift+S".into()
        }),
        PaletteCommand::OpenSettings => Some(shortcut_label(",")),
        _ => None,
    }
}

fn render_model_picker(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let current_model_id = app
        .current_conversation()
        .and_then(|conversation| conversation.model_id.as_deref());
    let filtered_models = app.filtered_models();
    let mut models = div()
        .id("model-picker-list")
        .min_h_0()
        .flex_1()
        .overflow_y_scroll()
        .track_scroll(&app.model_scroll)
        .flex()
        .flex_col()
        .gap_2();

    if app.current_conversation().is_none() {
        models = models.child(
            div()
                .rounded_lg()
                .bg(colors.raised)
                .p_3()
                .text_sm()
                .text_color(colors.muted)
                .child("Create or select a conversation before choosing a model."),
        );
    } else if app.snapshot.models.is_empty() {
        models = models.child(
            div()
                .rounded_lg()
                .bg(colors.raised)
                .p_3()
                .text_sm()
                .text_color(colors.muted)
                .child("No models configured."),
        );
    } else if filtered_models.is_empty() {
        models = models.child(
            div()
                .rounded_lg()
                .bg(colors.raised)
                .p_3()
                .text_sm()
                .text_color(colors.muted)
                .child("No models match this search."),
        );
    } else {
        for (index, model) in filtered_models.into_iter().enumerate() {
            let provider = app
                .provider_for_model(model)
                .map(|provider| provider.name.as_str())
                .unwrap_or("Missing provider");
            let availability = app.model_availability(model);
            let available = availability.is_ok();
            let status = availability.map_or_else(|reason| reason, |_| "Available");
            let current = current_model_id == Some(model.id.as_str());
            let highlighted = index == app.model_selection;
            let status = match (current, available) {
                (true, true) => "Selected".to_string(),
                (true, false) => format!("Selected · {status}"),
                (false, _) => status.to_string(),
            };
            let model_id = model.id.clone();
            models = models.child(
                div()
                    .id(SharedString::from(format!("pick-model-{}", model.id)))
                    .rounded_lg()
                    .border_1()
                    .border_color(if highlighted || current {
                        colors.accent
                    } else {
                        colors.border
                    })
                    .bg(if highlighted || current {
                        colors.accent_soft
                    } else {
                        colors.panel
                    })
                    .p_3()
                    .when(available, |element| {
                        element
                            .cursor_pointer()
                            .hover(move |style| style.bg(colors.raised))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.select_model(model_id.clone(), cx)
                            }))
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(
                                div()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(model.display_name.clone()),
                                    )
                                    .child(
                                        div()
                                            .overflow_hidden()
                                            .whitespace_nowrap()
                                            .text_ellipsis()
                                            .text_xs()
                                            .text_color(colors.muted)
                                            .child(format!("{} · {provider}", model.remote_id)),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(colors.muted)
                                            .child(inspector::capability_summary(model)),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_xs()
                                    .text_color(if available {
                                        colors.accent
                                    } else {
                                        colors.danger
                                    })
                                    .child(status),
                            ),
                    ),
            );
        }
    }

    let panel = div()
        .w_full()
        .max_w(px(540.0))
        .max_h(px(620.0))
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .shadow_lg()
        .p_5()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .text_lg()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Choose model"),
                )
                .child(
                    button("close-model-picker", "Close", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.close_model_picker(cx))),
                ),
        )
        .child(app.model_search_input.clone())
        .child(models)
        .child(
            div()
                .text_xs()
                .text_color(colors.muted)
                .child("↑↓ Navigate · Enter Select · Esc Close"),
        );
    animated_overlay(
        panel,
        "model-picker-backdrop",
        "model-picker-panel",
        app.settings().reduce_motion,
    )
}

fn animated_overlay(
    panel: Div,
    backdrop_id: &'static str,
    panel_id: &'static str,
    reduce_motion: bool,
) -> AnyElement {
    let duration = if reduce_motion { 160 } else { 200 };
    let panel = panel
        .with_animation(
            panel_id,
            Animation::new(Duration::from_millis(duration)).with_easing(ease_out_quint()),
            move |panel, delta| {
                let panel = panel.opacity(0.72 + delta * 0.28);
                if reduce_motion {
                    panel
                } else {
                    panel.mt(px(12.0 * (1.0 - delta)))
                }
            },
        )
        .into_any_element();

    div()
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .left_0()
        .flex()
        .items_center()
        .justify_center()
        .p_5()
        .bg(rgba(0x00000066))
        .child(panel)
        .with_animation(
            backdrop_id,
            Animation::new(Duration::from_millis(duration)).with_easing(ease_out_quint()),
            |backdrop, delta| backdrop.opacity(delta),
        )
        .into_any_element()
}

pub(crate) fn button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .px_3()
        .py_2()
        .rounded_lg()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .text_sm()
        .cursor_pointer()
        .hover(move |style| style.bg(colors.raised))
        .child(label.into())
}

pub(crate) fn compact_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .px_1()
        .py_1()
        .rounded_md()
        .cursor_pointer()
        .hover(move |style| style.bg(colors.raised))
        .child(label.into())
}

fn icon_button(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    colors: Colors,
) -> Stateful<Div> {
    div()
        .id(id)
        .w(px(32.0))
        .h(px(32.0))
        .flex()
        .items_center()
        .justify_center()
        .rounded_lg()
        .cursor_pointer()
        .hover(move |style| style.bg(colors.raised))
        .child(label.into())
}
