use super::*;

pub(in crate::desktop::ui::chat) fn render_assistant_turn(
    app: &OneChat,
    turn: &Turn,
    response: &AssistantResponse,
    message_max_width: f32,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    animated_message(
        render_assistant_message(
            app,
            turn,
            response,
            message_max_width,
            scale_factor,
            typography,
            cx,
        ),
        format!("assistant-{}", response.id),
    )
}

fn render_assistant_message(
    app: &OneChat,
    turn: &Turn,
    message: &AssistantResponse,
    message_max_width: f32,
    scale_factor: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let request = app.request_for_response(message);
    let action_group: SharedString = format!("assistant-actions-{}", message.id).into();
    let assistant_label = format!("{} · {}", message.model_name, message.provider_name);
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
        let save_on_mouse_down_id = save_id.clone();
        div()
            .rounded_xl()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .p_3()
            .child(
                div()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_move(|_, _, cx| cx.stop_propagation())
                    .on_mouse_up(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_up_out(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_action(
                        cx.listener(|this, _: &InputEscape, _, cx| this.cancel_message_edit(cx)),
                    )
                    .child(
                        Input::new(&editor)
                            .aria_label("Edit assistant response")
                            .bg(cx.theme().muted)
                            .text_size(px(typography.body_size))
                            .line_height(px(typography.body_line_height)),
                    ),
            )
            .child(
                div()
                    .pt_3()
                    .flex()
                    .items_center()
                    .justify_end()
                    .gap_2()
                    .child(
                        large_icon_button(
                            SharedString::from(format!("cancel-edit-message-{}", message.id)),
                            AppIcon::Close,
                            IconTone::Muted,
                            cx,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                this.cancel_message_edit(cx);
                                cx.stop_propagation();
                            }),
                        )
                        .on_click(cx.listener(|this, _, _, cx| this.cancel_message_edit(cx))),
                    )
                    .child(
                        primary_icon_button(
                            SharedString::from(format!("save-edit-message-{}", message.id)),
                            AppIcon::Save,
                            cx,
                        )
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(move |this, _, _, cx| {
                                this.save_assistant_edit(save_on_mouse_down_id.clone(), cx);
                                cx.stop_propagation();
                            }),
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
            .text_size(px(typography.metadata_size))
            .line_height(px(typography.metadata_line_height))
            .text_color(cx.theme().muted_foreground)
            .child(div().size(px(7.0)).rounded_full().bg(cx.theme().primary))
            .child(waiting_label(message))
            .into_any_element()
    } else if let Some(document) = app.markdown_for(&message.id, &message.content) {
        markdown::render(
            document,
            &message.id,
            &app.chat.text_selection,
            scale_factor,
            typography,
            cx,
        )
    } else {
        markdown::render_plain(
            &message.content,
            &message.id,
            &app.chat.text_selection,
            typography,
            cx,
        )
    };

    let latest = app.is_latest_turn(&turn.id);
    let generating = app.is_current_generating();
    let has_content = !message.content.is_empty();
    let can_copy = has_content;
    let can_edit = latest && !generating && (!editing_any || editing);
    let can_regenerate = latest
        && !generating
        && !editing
        && !matches!(
            message.status,
            MessageStatus::Failed | MessageStatus::Interrupted
        );
    let can_use_context = !generating
        && message.status == MessageStatus::Completed
        && has_content
        && turn.continuation_response_id.as_deref() != Some(&message.id);
    let can_fork = !editing_any && message.status == MessageStatus::Completed && has_content;
    let has_info = request.is_some();

    let content_actions = if can_copy || can_edit {
        let mut group = div().flex().items_center().gap_1();
        if can_copy {
            group = group.child(CopyButton::new(
                SharedString::from(format!("copy-message-{}", message.id)),
                message.content.clone(),
            ));
        }
        if can_edit {
            let edit_id = message.id.clone();
            group = group.child(
                icon_button(
                    SharedString::from(format!("edit-message-{}", message.id)),
                    AppIcon::Pencil,
                    if editing {
                        IconTone::Accent
                    } else {
                        IconTone::Muted
                    },
                    cx,
                )
                .selected(editing)
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.begin_edit_assistant(edit_id.clone(), window, cx)
                })),
            );
        }
        Some(group)
    } else {
        None
    };

    let response_actions = if can_regenerate || can_use_context {
        let mut group = div().flex().items_center().gap_1();
        if can_regenerate {
            let regenerate_id = message.id.clone();
            group = group.child(
                icon_button(
                    SharedString::from(format!("regenerate-message-{}", message.id)),
                    AppIcon::Regenerate,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.regenerate_assistant(regenerate_id.clone(), cx)
                })),
            );
        }
        if can_use_context {
            let context_turn_id = turn.id.clone();
            let context_response_id = message.id.clone();
            group = group.child(
                icon_button(
                    SharedString::from(format!("use-response-context-{}", message.id)),
                    AppIcon::ContextSelect,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.use_response_for_context(
                        context_turn_id.clone(),
                        context_response_id.clone(),
                        cx,
                    )
                })),
            );
        }
        Some(group)
    } else {
        None
    };

    let conversation_actions = if can_fork {
        let fork_id = message.id.clone();
        Some(
            div().flex().items_center().gap_1().child(
                icon_button(
                    SharedString::from(format!("fork-message-{}", message.id)),
                    AppIcon::Fork,
                    IconTone::Muted,
                    cx,
                )
                .on_click(
                    cx.listener(move |this, _, _, cx| this.fork_from_response(fork_id.clone(), cx)),
                ),
            ),
        )
    } else {
        None
    };

    let info_actions = if has_info {
        let info_id = message.id.clone();
        Some(
            div().flex().items_center().gap_1().child(
                icon_button(
                    SharedString::from(format!("info-message-{}", message.id)),
                    AppIcon::Info,
                    IconTone::Muted,
                    cx,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.inspect_message_request(info_id.clone(), cx)
                })),
            ),
        )
    } else {
        None
    };

    let actions = div()
        .invisible()
        .group_hover(action_group.clone(), |actions| actions.visible())
        .flex()
        .items_center()
        .gap_2()
        .children(content_actions)
        .children(response_actions)
        .children(conversation_actions)
        .children(info_actions);

    let multiple_responses = turn.responses.len() > 1;
    let header_content = if multiple_responses {
        let mut tabs = div()
            .id(SharedString::from(format!("response-tabs-{}", turn.id)))
            .min_w_0()
            .flex_1()
            .flex()
            .items_center()
            .gap_1()
            .overflow_x_scroll()
            .restrict_scroll_to_axis();
        for response in &turn.responses {
            let selected = response.id == message.id;
            let context = turn.continuation_response_id.as_deref() == Some(&response.id);
            let status = match response.status {
                MessageStatus::Pending | MessageStatus::Streaming => "  ·  …",
                MessageStatus::Failed | MessageStatus::Interrupted => "  ·  !",
                MessageStatus::Stopped => "  ·  ■",
                MessageStatus::Completed => "",
            };
            let label = format!(
                "{} · {}{}",
                response.model_name, response.provider_name, status
            );
            let tab_turn_id = turn.id.clone();
            let tab_response_id = response.id.clone();
            tabs = tabs.child(
                response_tab_button(
                    SharedString::from(format!("response-tab-{}", response.id)),
                    label,
                    typography,
                )
                .selected(selected)
                .flex()
                .items_center()
                .gap_1()
                .children(
                    context
                        .then(|| render_icon(AppIcon::ContextSelected, IconTone::Accent, 15.0, cx)),
                )
                .bg(if selected {
                    cx.theme().accent
                } else {
                    cx.theme().transparent
                })
                .text_color(if selected {
                    cx.theme().primary
                } else {
                    cx.theme().muted_foreground
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.show_response(tab_turn_id.clone(), tab_response_id.clone(), cx)
                })),
            );
        }
        tabs.into_any_element()
    } else {
        div()
            .text_size(px(typography.metadata_size))
            .line_height(px(typography.metadata_line_height))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(cx.theme().muted_foreground)
            .child(assistant_label)
            .into_any_element()
    };
    let header = div()
        .mb_3()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .size(px(24.0))
                .flex_none()
                .rounded_lg()
                .bg(cx.theme().accent)
                .flex()
                .items_center()
                .justify_center()
                .text_color(cx.theme().primary)
                .child(render_icon(AppIcon::Sparkles, IconTone::Accent, 13.0, cx)),
        )
        .child(header_content)
        .children(
            (!multiple_responses && !matches!(message.status, MessageStatus::Completed))
                .then(|| status_badge(message.status, typography, cx)),
        );
    let stats = request.map(format_message_stats).unwrap_or_default();
    div()
        .id(SharedString::from(format!(
            "assistant-message-{}",
            message.id
        )))
        .mx_auto()
        .group(action_group)
        .mb_8()
        .w_full()
        .max_w(px(message_max_width))
        .child(header)
        .children(render_reasoning(app, message, request, typography, cx))
        .children(render_tool_executions(app, message, typography, cx))
        .child(content)
        .children(render_error_card(
            app, message, request, latest, generating, typography, cx,
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
                        .text_size(px(typography.micro_size))
                        .line_height(px(typography.micro_line_height))
                        .text_color(cx.theme().muted_foreground)
                        .child(stats)
                })),
        )
        .into_any_element()
}

pub(super) fn waiting_label(message: &AssistantResponse) -> String {
    if let Some(execution) = message
        .tool_executions
        .iter()
        .rev()
        .find(|execution| execution.status.is_active())
    {
        let action = match execution.status {
            ToolExecutionStatus::Queued => "Preparing",
            ToolExecutionStatus::Running => "Using",
            _ => unreachable!(),
        };
        return format!(
            "{action} {} · {}…",
            execution.server_id, execution.tool_name
        );
    }
    if !message.tool_executions.is_empty() {
        "Waiting for model…".into()
    } else if message.thinking.is_empty() {
        "Contacting provider…".into()
    } else {
        "Thinking…".into()
    }
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

fn status_badge(status: MessageStatus, typography: MessageTypography, cx: &App) -> AnyElement {
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
            if cx.theme().is_dark() {
                rgba(0xff453a24).into()
            } else {
                rgba(0xd7001518).into()
            }
        } else {
            cx.theme().muted
        })
        .px_2()
        .py_1()
        .text_size(px(typography.micro_size))
        .line_height(px(typography.micro_line_height))
        .text_color(if danger {
            cx.theme().danger
        } else {
            cx.theme().muted_foreground
        })
        .child(label)
        .into_any_element()
}

fn render_error_card(
    app: &OneChat,
    message: &AssistantResponse,
    request: Option<&RequestInfo>,
    latest: bool,
    generating: bool,
    typography: MessageTypography,
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
            .bg(if cx.theme().is_dark() {
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
                    .text_size(px(typography.metadata_size))
                    .line_height(px(typography.metadata_line_height))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(cx.theme().danger)
                    .child(summary),
            )
            .children(expanded.then(|| {
                div()
                    .text_size(px(typography.metadata_size))
                    .line_height(px(typography.metadata_line_height))
                    .text_color(cx.theme().muted_foreground)
                    .child(
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
                        primary_icon_button(
                            SharedString::from(format!("retry-message-{}", message.id)),
                            AppIcon::Regenerate,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.regenerate_assistant(retry_id.clone(), cx)
                        }))
                    }))
                    .children(detail.map(|_| {
                        large_icon_button(
                            SharedString::from(format!("error-detail-{}", message.id)),
                            if expanded {
                                AppIcon::ChevronUp
                            } else {
                                AppIcon::ChevronDown
                            },
                            IconTone::Muted,
                            cx,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_error_detail(detail_id.clone(), cx)
                        }))
                    })),
            )
            .into_any_element(),
    )
}
