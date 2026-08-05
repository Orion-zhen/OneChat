use gpui::{
    AnyElement, Context, Div, ElementId, FontWeight, Rgba, SharedString, Stateful, Window,
    WindowAppearance, div, prelude::*, px, rgb, rgba,
};

use crate::{
    app::{OneChat, SystemPromptMode},
    model::{Conversation, MessageRole, MessageStatus, Page, RequestStatus, Theme},
    ui::{inspector, settings},
};

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
    dark: bool,
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
    let colors = Colors::for_theme(app.theme(), window.appearance());
    let narrow = window.viewport_size().width < px(1180.0);
    let sidebar = render_sidebar(app, colors, cx);
    let top_bar = render_top_bar(app, colors, cx);
    let page = match app.page {
        Page::Chat => render_chat_page(app, colors, cx),
        Page::Settings => settings::render(app, colors, cx),
    };

    let inspector = app
        .inspector_open
        .then(|| inspector::render(app, colors, narrow, cx));
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
        .children(model_picker)
        .into_any_element()
}

fn render_sidebar(app: &mut OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    if app.settings().sidebar_collapsed {
        return div()
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
            )
            .into_any_element();
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

    div()
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
        .child(render_connection_footer(app, colors, cx))
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
    let provider = app
        .current_provider()
        .map(|provider| provider.name.clone())
        .unwrap_or_else(|| "No provider".into());
    let has_system_prompt = app
        .current_conversation()
        .is_some_and(|conversation| !conversation.system_prompt.content.trim().is_empty());

    div()
        .h(px(58.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .px_5()
        .border_b_1()
        .border_color(colors.border)
        .bg(colors.panel)
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_3()
                .child(div().font_weight(FontWeight::SEMIBOLD).child(title))
                .children(has_system_prompt.then(|| {
                    div()
                        .rounded_md()
                        .bg(colors.accent_soft)
                        .px_2()
                        .py_1()
                        .text_xs()
                        .text_color(colors.accent)
                        .child("System")
                }))
                .child(
                    button("open-model-picker", format!("{model} · {provider}"), colors)
                        .on_click(cx.listener(|this, _, _, cx| this.open_model_picker(cx))),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    button("chat-page", "Chat", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Chat, cx))),
                )
                .child(
                    button("toggle-inspector", "Inspector", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.toggle_inspector(cx))),
                )
                .child(
                    button("top-settings", "Settings", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.set_page(Page::Settings, cx))),
                ),
        )
        .into_any_element()
}

fn render_chat_page(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
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
        render_conversation(app, colors, cx)
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

fn render_conversation(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let conversation = app
        .current_conversation()
        .expect("conversation page requires a current conversation");
    let has_system_prompt = !conversation.system_prompt.content.trim().is_empty();
    let editing_system_prompt = app.system_prompt_mode == SystemPromptMode::Editing;
    let mut messages = div()
        .id("message-list")
        .min_h_0()
        .flex_1()
        .overflow_y_scroll()
        .px_6()
        .py_5()
        .children(
            (has_system_prompt || editing_system_prompt)
                .then(|| render_system_prompt_card(app, colors, cx)),
        );
    if app.current_messages().is_empty() {
        messages = messages.child(
            div().h_full().flex().items_center().justify_center().child(
                div()
                    .text_center()
                    .text_color(colors.muted)
                    .child("Send a message to start this conversation."),
            ),
        );
    } else {
        for message in app.current_messages() {
            let assistant = message.role == MessageRole::Assistant;
            let status = match message.status {
                MessageStatus::Pending => "Sending",
                MessageStatus::Streaming => "Streaming",
                MessageStatus::Completed => "Completed",
                MessageStatus::Stopped => "Stopped",
                MessageStatus::Failed => "Failed",
                MessageStatus::Interrupted => "Interrupted",
            };
            let content =
                if message.content.is_empty() && message.status == MessageStatus::Streaming {
                    "Waiting for provider…".to_string()
                } else {
                    message.content.clone()
                };
            messages = messages.child(
                div()
                    .mb_4()
                    .flex()
                    .justify_end()
                    .when(assistant, |element| element.justify_start())
                    .child(
                        div()
                            .max_w(px(720.0))
                            .rounded_xl()
                            .border_1()
                            .border_color(colors.border)
                            .bg(if assistant {
                                colors.panel
                            } else {
                                colors.accent_soft
                            })
                            .px_4()
                            .py_3()
                            .child(
                                div()
                                    .pb_2()
                                    .flex()
                                    .gap_2()
                                    .text_xs()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(colors.muted)
                                    .child(if assistant { "Assistant" } else { "You" })
                                    .child(status),
                            )
                            .when(!message.thinking.is_empty(), |element| {
                                element.child(
                                    div()
                                        .mb_3()
                                        .rounded_lg()
                                        .bg(colors.raised)
                                        .p_3()
                                        .text_sm()
                                        .text_color(colors.muted)
                                        .child(message.thinking.clone()),
                                )
                            })
                            .child(content),
                    ),
            );
        }
    }

    let generating = app.is_current_generating();
    let action = if generating {
        button("stop-generation", "Stop", colors)
            .text_color(colors.danger)
            .on_click(cx.listener(|this, _, _, cx| this.stop_current_generation(cx)))
    } else {
        button("send-message", "Send", colors)
            .on_click(cx.listener(|this, _, _, cx| this.send_composer(cx)))
    };
    let request_summary = app.current_request().map(|request| {
        let status = match request.status {
            RequestStatus::Sending => "Sending",
            RequestStatus::Streaming => "Streaming",
            RequestStatus::Stopped => "Stopped",
            RequestStatus::Failed => "Failed",
            RequestStatus::Completed => "Completed",
            RequestStatus::Interrupted => "Interrupted",
        };
        let mut summary = format!(
            "{status} · input {} · output {} · TTFT {} · total {}",
            format_token_count(request.usage.input_tokens, request.usage.estimated),
            format_token_count(request.usage.output_tokens, request.usage.estimated),
            request
                .ttft_ms
                .map_or_else(|| "—".into(), |value| format!("{value} ms")),
            request
                .duration_ms
                .map_or_else(|| "—".into(), |value| format!("{value} ms")),
        );
        if let Some(error) = &request.error {
            summary.push_str(" · ");
            summary.push_str(&error.message);
        }
        summary
    });

    div()
        .size_full()
        .flex()
        .flex_col()
        .child(messages)
        .child(
            div()
                .flex_none()
                .mx_auto()
                .w_full()
                .max_w(px(820.0))
                .px_5()
                .pb_5()
                .children(
                    (!has_system_prompt
                        && !editing_system_prompt
                        && app.current_messages().is_empty())
                    .then(|| {
                        div().pb_2().child(
                            button("add-system-prompt", "+ Add System Prompt", colors).on_click(
                                cx.listener(|this, _, _, cx| this.begin_edit_system_prompt(cx)),
                            ),
                        )
                    }),
                )
                .child(
                    div()
                        .flex()
                        .items_end()
                        .gap_3()
                        .child(div().min_w_0().flex_1().child(app.composer.clone()))
                        .child(action),
                )
                .children(request_summary.map(|summary| {
                    div()
                        .pt_2()
                        .px_2()
                        .text_xs()
                        .text_color(colors.muted)
                        .child(summary)
                })),
        )
        .into_any_element()
}

fn render_system_prompt_card(
    app: &OneChat,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let conversation = app
        .current_conversation()
        .expect("system prompt card requires a conversation");
    let source = match conversation.system_prompt.source {
        crate::model::SystemPromptSource::None => "None",
        crate::model::SystemPromptSource::FromDefault => "Default snapshot",
        crate::model::SystemPromptSource::Custom => "Custom",
    };
    let actions = match app.system_prompt_mode {
        SystemPromptMode::Compact => div()
            .flex()
            .gap_2()
            .child(
                compact_button("expand-system-prompt", "Expand", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.expand_system_prompt(cx))),
            )
            .child(
                compact_button("copy-system-prompt", "Copy", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.copy_system_prompt(cx))),
            )
            .child(
                compact_button("edit-system-prompt", "Edit", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.begin_edit_system_prompt(cx))),
            )
            .into_any_element(),
        SystemPromptMode::Expanded => div()
            .flex()
            .gap_2()
            .child(
                compact_button("collapse-system-prompt", "Collapse", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.collapse_system_prompt(cx))),
            )
            .child(
                compact_button("copy-system-prompt-expanded", "Copy", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.copy_system_prompt(cx))),
            )
            .child(
                compact_button("edit-system-prompt-expanded", "Edit", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.begin_edit_system_prompt(cx))),
            )
            .into_any_element(),
        SystemPromptMode::Editing => div()
            .flex()
            .gap_2()
            .child(
                button("save-system-prompt", "Save", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.save_system_prompt(cx))),
            )
            .child(
                button("cancel-system-prompt", "Cancel", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.cancel_system_prompt_edit(cx))),
            )
            .into_any_element(),
    };
    let content = match app.system_prompt_mode {
        SystemPromptMode::Compact => div()
            .text_sm()
            .text_color(colors.muted)
            .child(prompt_preview(&conversation.system_prompt.content))
            .into_any_element(),
        SystemPromptMode::Expanded => div()
            .text_sm()
            .child(conversation.system_prompt.content.clone())
            .into_any_element(),
        SystemPromptMode::Editing => app
            .system_prompt_editor
            .as_ref()
            .map(|editor| editor.clone().into_any_element())
            .unwrap_or_else(|| {
                div()
                    .text_sm()
                    .text_color(colors.muted)
                    .child("Opening editor…")
                    .into_any_element()
            }),
    };

    div()
        .mb_4()
        .mx_auto()
        .w_full()
        .max_w(px(820.0))
        .rounded_xl()
        .border_1()
        .border_color(colors.border)
        .bg(colors.panel)
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
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("System Prompt"),
                )
                .child(div().text_xs().text_color(colors.muted).child(source)),
        )
        .child(content)
        .child(actions)
        .into_any_element()
}

fn prompt_preview(prompt: &str) -> String {
    const MAX_CHARACTERS: usize = 160;
    let prompt = prompt.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = prompt.chars();
    let preview = characters.by_ref().take(MAX_CHARACTERS).collect::<String>();
    if characters.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

fn render_model_picker(app: &OneChat, colors: Colors, cx: &mut Context<OneChat>) -> AnyElement {
    let current_model_id = app
        .current_conversation()
        .and_then(|conversation| conversation.model_id.as_deref());
    let mut models = div()
        .id("model-picker-list")
        .min_h_0()
        .flex_1()
        .overflow_y_scroll()
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
    } else {
        for model in &app.snapshot.models {
            let provider = app
                .provider_for_model(model)
                .map(|provider| provider.name.as_str())
                .unwrap_or("Missing provider");
            let availability = app.model_availability(model);
            let available = availability.is_ok();
            let status = availability.map_or_else(|reason| reason, |_| "Available");
            let selected = current_model_id == Some(model.id.as_str());
            let status = match (selected, available) {
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
                    .border_color(if selected {
                        colors.accent
                    } else {
                        colors.border
                    })
                    .bg(if selected {
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
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(model.display_name.clone()),
                                    )
                                    .child(
                                        div()
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

    div()
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .left_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(rgba(0x00000066))
        .child(
            div()
                .w(px(520.0))
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
                        .child(
                            div()
                                .text_lg()
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Choose model"),
                        )
                        .child(
                            button("close-model-picker", "Close", colors).on_click(
                                cx.listener(|this, _, _, cx| this.close_model_picker(cx)),
                            ),
                        ),
                )
                .child(models),
        )
        .into_any_element()
}

fn format_token_count(value: Option<u64>, estimated: bool) -> String {
    value.map_or_else(
        || "—".into(),
        |value| {
            if estimated {
                format!("~{value}")
            } else {
                value.to_string()
            }
        },
    )
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

fn compact_button(
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
