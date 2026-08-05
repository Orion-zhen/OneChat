use gpui::{AnyElement, Context, FontWeight, SharedString, div, prelude::*, px};

use crate::{
    app::{OneChat, SystemPromptMode},
    model::{Message, MessageRole, MessageStatus, RequestInfo, RequestStatus, SystemPromptSource},
    ui::{
        inspector::InspectorTab,
        markdown,
        shell::{Colors, button, compact_button},
    },
};

pub(crate) fn render(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
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
        .track_scroll(&app.message_scroll)
        .on_scroll_wheel(cx.listener(OneChat::on_message_scroll))
        .px_6()
        .pt_6()
        .pb_4()
        .children(
            (has_system_prompt || editing_system_prompt)
                .then(|| render_system_prompt_card(app, colors, cx)),
        );

    if app.current_messages().is_empty() {
        messages = messages.child(render_empty_conversation(app, colors, cx));
    } else {
        for message in app.current_messages() {
            messages = messages.child(render_message(app, message, colors, scale_factor, cx));
        }
    }

    div()
        .size_full()
        .flex()
        .flex_col()
        .child(messages)
        .children((!app.follow_latest).then(|| {
            div().flex_none().flex().justify_center().pb_2().child(
                button("jump-to-latest", "↓ Jump to latest", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.jump_to_latest(cx))),
            )
        }))
        .child(render_composer(
            app,
            has_system_prompt,
            editing_system_prompt,
            colors,
            cx,
        ))
        .into_any_element()
}

fn render_empty_conversation(
    app: &OneChat,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .min_h(px(240.0))
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
                        .child("Start the conversation"),
                )
                .child(
                    div().text_color(colors.muted).child(
                        "Messages, request metrics, and partial responses are saved locally.",
                    ),
                )
                .children(app.current_model().is_none().then(|| {
                    button("empty-choose-model", "Choose model", colors)
                        .on_click(cx.listener(|this, _, _, cx| this.open_model_picker(cx)))
                })),
        )
        .into_any_element()
}

fn render_message(
    app: &OneChat,
    message: &Message,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    match message.role {
        MessageRole::User => render_user_message(message, colors),
        MessageRole::Assistant => render_assistant_message(app, message, colors, scale_factor, cx),
    }
}

fn render_user_message(message: &Message, colors: Colors) -> AnyElement {
    div()
        .mb_6()
        .flex()
        .justify_end()
        .child(
            div()
                .max_w(px(680.0))
                .rounded_xl()
                .bg(colors.accent_soft)
                .px_4()
                .py_3()
                .whitespace_normal()
                .line_height(px(23.0))
                .child(message.content.clone()),
        )
        .into_any_element()
}

fn render_assistant_message(
    app: &OneChat,
    message: &Message,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let request = app.request_for_message(message);
    let waiting = message.content.is_empty()
        && matches!(
            message.status,
            MessageStatus::Pending | MessageStatus::Streaming
        );
    let selectable = app.selectable_message(message);
    let selecting = selectable.is_some();
    let content = if let Some(selectable) = selectable {
        div()
            .rounded_lg()
            .border_1()
            .border_color(colors.border)
            .bg(colors.raised)
            .p_2()
            .child(
                div()
                    .pb_2()
                    .text_xs()
                    .text_color(colors.muted)
                    .child("Selection mode · drag to select, then press Cmd+C"),
            )
            .child(selectable)
            .into_any_element()
    } else if waiting {
        div()
            .text_color(colors.muted)
            .child("Waiting for provider…")
            .into_any_element()
    } else if let Some(document) = app.markdown_for(message) {
        markdown::render(document, colors, scale_factor)
    } else {
        markdown::render_plain(&message.content, colors)
    };

    let latest = app.is_latest_assistant(&message.id);
    let generating = app.is_current_generating();
    let copy_id = message.id.clone();
    let select_id = message.id.clone();
    let regenerate_id = message.id.clone();
    let info_id = message.id.clone();
    let mut actions = div().flex().items_center().gap_1();
    if !message.content.is_empty() {
        actions = actions
            .child(
                compact_button(
                    SharedString::from(format!("copy-message-{}", message.id)),
                    "Copy",
                    colors,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.copy_assistant(copy_id.clone(), cx)),
                ),
            )
            .children((!generating).then(|| {
                compact_button(
                    SharedString::from(format!("select-message-{}", message.id)),
                    if selecting { "Rendered" } else { "Select text" },
                    colors,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.toggle_message_selection(select_id.clone(), cx)
                }))
            }));
    }
    if latest
        && !generating
        && !matches!(
            message.status,
            MessageStatus::Failed | MessageStatus::Interrupted
        )
    {
        actions = actions.child(
            compact_button(
                SharedString::from(format!("regenerate-message-{}", message.id)),
                "Regenerate",
                colors,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.regenerate_assistant(regenerate_id.clone(), cx)
            })),
        );
    }
    if request.is_some() {
        actions =
            actions.child(
                compact_button(
                    SharedString::from(format!("info-message-{}", message.id)),
                    "Info",
                    colors,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.inspect_message_request(info_id.clone(), cx)
                })),
            );
    }

    div()
        .mb_7()
        .w_full()
        .child(
            div()
                .mb_3()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors.muted)
                        .child("Assistant")
                        .child(status_badge(message.status, colors)),
                )
                .child(actions),
        )
        .children((!message.thinking.is_empty()).then(|| {
            div()
                .mb_4()
                .rounded_lg()
                .border_1()
                .border_color(colors.border)
                .bg(colors.raised)
                .p_3()
                .text_sm()
                .text_color(colors.muted)
                .child(
                    div()
                        .pb_2()
                        .text_xs()
                        .font_weight(FontWeight::SEMIBOLD)
                        .child("Thinking"),
                )
                .child(message.thinking.clone())
        }))
        .child(content)
        .children(render_error_card(
            app, message, request, latest, generating, colors, cx,
        ))
        .children(request.map(|request| render_request_line(request, colors)))
        .into_any_element()
}

fn status_badge(status: MessageStatus, colors: Colors) -> AnyElement {
    let label = match status {
        MessageStatus::Pending => "Sending",
        MessageStatus::Streaming => "Streaming",
        MessageStatus::Completed => "Completed",
        MessageStatus::Stopped => "Stopped",
        MessageStatus::Failed => "Failed",
        MessageStatus::Interrupted => "Interrupted",
    };
    div()
        .rounded_md()
        .bg(colors.raised)
        .px_2()
        .py_1()
        .text_color(
            if matches!(status, MessageStatus::Failed | MessageStatus::Interrupted) {
                colors.danger
            } else {
                colors.muted
            },
        )
        .child(label)
        .into_any_element()
}

fn render_error_card(
    app: &OneChat,
    message: &Message,
    request: Option<&RequestInfo>,
    latest: bool,
    generating: bool,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> Option<AnyElement> {
    if !matches!(
        message.status,
        MessageStatus::Failed | MessageStatus::Interrupted
    ) {
        return None;
    }
    let error = request.and_then(|request| request.error.as_ref());
    let summary = error.map_or_else(
        || "Generation was interrupted before it completed.".to_string(),
        |error| error.message.clone(),
    );
    let detail = error
        .and_then(|error| error.detail.clone())
        .or_else(|| error.map(|error| format!("Error category: {}", error.kind)));
    let expanded = app.error_detail_expanded(&message.id);
    let retry_id = message.id.clone();
    let detail_id = message.id.clone();

    Some(
        div()
            .mt_4()
            .rounded_lg()
            .border_1()
            .border_color(colors.danger)
            .bg(colors.raised)
            .p_3()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(colors.danger)
                    .child(summary),
            )
            .children((expanded).then(|| {
                div().text_xs().text_color(colors.muted).child(
                    detail
                        .clone()
                        .unwrap_or_else(|| "No technical details returned.".into()),
                )
            }))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .children((latest && !generating).then(|| {
                        button(
                            SharedString::from(format!("retry-message-{}", message.id)),
                            "Retry",
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.regenerate_assistant(retry_id.clone(), cx)
                        }))
                    }))
                    .children(detail.map(|_| {
                        button(
                            SharedString::from(format!("error-detail-{}", message.id)),
                            if expanded { "Hide details" } else { "Details" },
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_error_detail(detail_id.clone(), cx)
                        }))
                    })),
            )
            .into_any_element(),
    )
}

fn render_request_line(request: &RequestInfo, colors: Colors) -> AnyElement {
    let status = request_status(request.status);
    div()
        .pt_3()
        .text_xs()
        .text_color(colors.muted)
        .child(format!(
            "{status} · input {} · output {} · TTFT {} · total {}",
            format_token_count(request.usage.input_tokens, request.usage.estimated),
            format_token_count(request.usage.output_tokens, request.usage.estimated),
            request
                .ttft_ms
                .map_or_else(|| "—".into(), |value| format!("{value} ms")),
            request
                .duration_ms
                .map_or_else(|| "—".into(), |value| format!("{value} ms")),
        ))
        .into_any_element()
}

fn render_composer(
    app: &OneChat,
    has_system_prompt: bool,
    editing_system_prompt: bool,
    colors: Colors,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let generating = app.is_current_generating();
    let action = if generating {
        button("composer-stop", "■ Stop", colors)
            .text_color(colors.danger)
            .on_click(cx.listener(|this, _, _, cx| this.stop_current_generation(cx)))
    } else {
        button("composer-send", "Send ↑", colors)
            .on_click(cx.listener(|this, _, _, cx| this.send_composer(cx)))
    };

    div()
        .flex_none()
        .w_full()
        .px_5()
        .pb_5()
        .children((!has_system_prompt && !editing_system_prompt).then(|| {
            div().pb_2().child(
                compact_button("composer-add-system-prompt", "+ Add System Prompt", colors)
                    .on_click(cx.listener(|this, _, _, cx| this.begin_edit_system_prompt(cx))),
            )
        }))
        .child(
            div()
                .rounded_xl()
                .border_1()
                .border_color(colors.border)
                .bg(colors.panel)
                .shadow_md()
                .p_3()
                .child(
                    div()
                        .pb_2()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_1()
                        .child(
                            compact_button("composer-system", "System Prompt", colors).on_click(
                                cx.listener(|this, _, _, cx| this.begin_edit_system_prompt(cx)),
                            ),
                        )
                        .child(
                            compact_button("composer-context", "Context", colors).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.open_inspector(InspectorTab::Context, cx)
                                }),
                            ),
                        )
                        .child(
                            compact_button("composer-parameters", "Parameters", colors).on_click(
                                cx.listener(|this, _, _, cx| {
                                    this.open_inspector(InspectorTab::Model, cx)
                                }),
                            ),
                        )
                        .child(
                            compact_button("composer-model", "Model", colors)
                                .on_click(cx.listener(|this, _, _, cx| this.open_model_picker(cx))),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_end()
                        .gap_3()
                        .child(div().min_w_0().flex_1().child(app.composer.clone()))
                        .child(action),
                ),
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
        SystemPromptSource::None => "None",
        SystemPromptSource::FromDefault => "Default snapshot",
        SystemPromptSource::Custom => "Custom",
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
            .whitespace_normal()
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
        .mb_6()
        .w_full()
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

fn request_status(status: RequestStatus) -> &'static str {
    match status {
        RequestStatus::Sending => "Sending",
        RequestStatus::Streaming => "Streaming",
        RequestStatus::Stopped => "Stopped",
        RequestStatus::Failed => "Failed",
        RequestStatus::Completed => "Completed",
        RequestStatus::Interrupted => "Interrupted",
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_preview_is_short_and_single_line() {
        let preview = prompt_preview(&format!("first\n{}", "x".repeat(200)));
        assert!(!preview.contains('\n'));
        assert!(preview.ends_with('…'));
        assert!(preview.chars().count() <= 161);
    }

    #[test]
    fn estimated_token_counts_are_explicit() {
        assert_eq!(format_token_count(Some(12), true), "~12");
        assert_eq!(format_token_count(Some(12), false), "12");
        assert_eq!(format_token_count(None, false), "—");
    }
}
