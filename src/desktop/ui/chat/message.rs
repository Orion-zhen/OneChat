use super::*;

pub(super) fn render_message(
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
        .mx_auto()
        .mb_7()
        .w_full()
        .max_w(px(780.0))
        .flex()
        .justify_end()
        .child(
            div()
                .max_w(px(590.0))
                .rounded_xl()
                .bg(colors.accent)
                .px_4()
                .py_3()
                .text_color(colors.on_accent)
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
    let editor = app.assistant_message_editor(message);
    let editing = editor.is_some();
    let editing_any = app.active_message_editor().is_some();
    let content = if let Some(editor) = editor {
        let save_id = message.id.clone();
        div()
            .rounded_xl()
            .border_1()
            .border_color(colors.border)
            .bg(colors.panel)
            .p_3()
            .child(editor)
            .child(
                div()
                    .pt_3()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .child(
                        button(
                            SharedString::from(format!("cancel-edit-message-{}", message.id)),
                            "Cancel",
                            colors,
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_assistant_edit(cx))),
                    )
                    .child(
                        primary_button(
                            SharedString::from(format!("save-edit-message-{}", message.id)),
                            "Save",
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.save_assistant_edit(save_id.clone(), cx)
                        })),
                    ),
            )
            .into_any_element()
    } else if waiting {
        div()
            .flex()
            .items_center()
            .gap_2()
            .text_color(colors.muted)
            .child(div().size(px(7.0)).rounded_full().bg(colors.accent))
            .child(if message.thinking.is_empty() {
                "Contacting provider…"
            } else {
                "Thinking…"
            })
            .into_any_element()
    } else if let Some(document) = app.markdown_for(message) {
        markdown::render(document, colors, scale_factor)
    } else {
        markdown::render_plain(&message.content, colors)
    };

    let latest = app.is_latest_assistant(&message.id);
    let generating = app.is_current_generating();
    let copy_id = message.id.clone();
    let edit_id = message.id.clone();
    let regenerate_id = message.id.clone();
    let info_id = message.id.clone();
    let thinking_expanded = app.thinking_expanded(&message.id);
    let thinking_id = message.id.clone();
    let thinking_preview = thinking_preview(&message.thinking);
    let mut actions = div().flex().items_center().gap_1();
    if !message.content.is_empty() {
        actions = actions.child(
            svg_icon_button(
                SharedString::from(format!("copy-message-{}", message.id)),
                UiIcon::Copy,
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .on_click(cx.listener(move |this, _, _, cx| this.copy_assistant(copy_id.clone(), cx))),
        );
    }
    if !generating && (!editing_any || editing) {
        actions = actions.child(
            svg_icon_button(
                SharedString::from(format!("edit-message-{}", message.id)),
                UiIcon::Pencil,
                if editing {
                    IconTone::Accent
                } else {
                    IconTone::Muted
                },
                colors,
                scale_factor,
            )
            .on_click(
                cx.listener(move |this, _, _, cx| this.begin_edit_assistant(edit_id.clone(), cx)),
            ),
        );
    }
    if latest
        && !generating
        && !editing
        && !matches!(
            message.status,
            MessageStatus::Failed | MessageStatus::Interrupted
        )
    {
        actions = actions.child(
            svg_icon_button(
                SharedString::from(format!("regenerate-message-{}", message.id)),
                UiIcon::Regenerate,
                IconTone::Muted,
                colors,
                scale_factor,
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.regenerate_assistant(regenerate_id.clone(), cx)
            })),
        );
    }
    if request.is_some() {
        actions =
            actions.child(
                svg_icon_button(
                    SharedString::from(format!("info-message-{}", message.id)),
                    UiIcon::Info,
                    IconTone::Muted,
                    colors,
                    scale_factor,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.inspect_message_request(info_id.clone(), cx)
                })),
            );
    }

    let stats = request.map(format_message_stats).unwrap_or_default();
    let show_status = !matches!(message.status, MessageStatus::Completed);
    div()
        .id(SharedString::from(format!(
            "assistant-message-{}",
            message.id
        )))
        .mx_auto()
        .mb_8()
        .w_full()
        .max_w(px(780.0))
        .child(
            div()
                .mb_3()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .size(px(24.0))
                        .rounded_lg()
                        .bg(colors.accent_soft)
                        .flex()
                        .items_center()
                        .justify_center()
                        .text_size(px(11.0))
                        .text_color(colors.accent)
                        .child("✦"),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(colors.muted)
                        .child("OneChat"),
                )
                .children(show_status.then(|| status_badge(message.status, colors))),
        )
        .children((!message.thinking.is_empty()).then(|| {
            div()
                .mb_4()
                .rounded_xl()
                .bg(colors.raised)
                .p_4()
                .text_sm()
                .line_height(px(22.0))
                .text_color(colors.muted)
                .child(
                    div()
                        .pb_2()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(px(11.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("REASONING"),
                        )
                        .child(
                            compact_button(
                                SharedString::from(format!("thinking-{}", message.id)),
                                if thinking_expanded { "Hide" } else { "Show" },
                                colors,
                            )
                            .text_color(colors.accent)
                            .on_click(cx.listener(
                                move |this, _, _, cx| this.toggle_thinking(thinking_id.clone(), cx),
                            )),
                        ),
                )
                .child(if thinking_expanded {
                    message.thinking.clone()
                } else {
                    thinking_preview
                })
        }))
        .child(content)
        .children(render_error_card(
            app, message, request, latest, generating, colors, cx,
        ))
        .child(
            div()
                .mt_3()
                .min_h(px(24.0))
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(actions)
                .children((!stats.is_empty()).then(|| {
                    div()
                        .min_w_0()
                        .flex_1()
                        .text_right()
                        .text_size(px(11.0))
                        .text_color(colors.muted)
                        .child(stats)
                })),
        )
        .into_any_element()
}

pub(super) fn format_message_stats(request: &RequestInfo) -> String {
    let mut stats = Vec::new();
    if let Some(tokens) = request.usage.output_tokens {
        stats.push(format!(
            "{}{tokens} tokens",
            if request.usage.estimated { "~" } else { "" }
        ));
        if let (Some(duration_ms), Some(ttft_ms)) = (request.duration_ms, request.ttft_ms) {
            let generation_ms = duration_ms.saturating_sub(ttft_ms);
            if generation_ms > 0 {
                stats.push(format!(
                    "{:.1} tok/s",
                    tokens as f64 * 1000.0 / generation_ms as f64
                ));
            }
        }
    }
    if let Some(ttft_ms) = request.ttft_ms {
        stats.push(format!("TTFT {ttft_ms} ms"));
    }
    stats.join("  ·  ")
}

fn status_badge(status: MessageStatus, colors: Colors) -> AnyElement {
    let label = match status {
        MessageStatus::Pending => "Sending",
        MessageStatus::Streaming => "Writing",
        MessageStatus::Completed => "Completed",
        MessageStatus::Stopped => "Stopped",
        MessageStatus::Failed => "Failed",
        MessageStatus::Interrupted => "Interrupted",
    };
    let danger = matches!(status, MessageStatus::Failed | MessageStatus::Interrupted);
    div()
        .rounded_full()
        .bg(if danger {
            if colors.dark {
                rgba(0xff453a24)
            } else {
                rgba(0xd7001518)
            }
        } else {
            colors.raised
        })
        .px_2()
        .py_1()
        .text_size(px(11.0))
        .text_color(if danger { colors.danger } else { colors.muted })
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
        || "Generation stopped before it completed.".to_string(),
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
            .rounded_xl()
            .bg(if colors.dark {
                rgba(0xff453a16)
            } else {
                rgba(0xd700150d)
            })
            .p_4()
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
            .children(expanded.then(|| {
                div().text_sm().text_color(colors.muted).child(
                    detail
                        .clone()
                        .unwrap_or_else(|| "No technical details were returned.".into()),
                )
            }))
            .child(
                div()
                    .flex()
                    .gap_2()
                    .children((latest && !generating).then(|| {
                        primary_button(
                            SharedString::from(format!("retry-message-{}", message.id)),
                            "Try Again",
                            colors,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.regenerate_assistant(retry_id.clone(), cx)
                        }))
                    }))
                    .children(detail.map(|_| {
                        button(
                            SharedString::from(format!("error-detail-{}", message.id)),
                            if expanded {
                                "Hide Details"
                            } else {
                                "Show Details"
                            },
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

fn thinking_preview(thinking: &str) -> String {
    const MAX_CHARACTERS: usize = 220;
    let thinking = thinking.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = thinking.chars();
    let preview = characters.by_ref().take(MAX_CHARACTERS).collect::<String>();
    if characters.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}
