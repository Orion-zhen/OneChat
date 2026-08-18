use super::*;

pub(super) fn render_tool_executions(
    app: &OneChat,
    message: &AssistantResponse,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> Option<AnyElement> {
    if message.tool_executions.is_empty() {
        return None;
    }

    Some(
        div()
            .mb_4()
            .flex()
            .flex_col()
            .gap_2()
            .children(
                message
                    .tool_executions
                    .iter()
                    .map(|execution| render_tool_execution(app, execution, typography, cx)),
            )
            .into_any_element(),
    )
}

pub(super) fn render_tool_placeholder(
    block_id: &str,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    div()
        .id(SharedString::from(format!("tool-call-{block_id}")))
        .rounded_xl()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .shadow_xs()
        .px_3()
        .py_2()
        .flex()
        .items_center()
        .gap_2()
        .child(render_icon(AppIcon::Plug, IconTone::Accent, 16.0, cx))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .text_size(px(typography.metadata_size))
                .line_height(px(typography.metadata_line_height))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Preparing tool call…"),
        )
        .child(
            div()
                .rounded_full()
                .bg(cx.theme().secondary)
                .px_2()
                .py_1()
                .text_size(px(typography.micro_size))
                .line_height(px(typography.micro_line_height))
                .text_color(cx.theme().primary)
                .child("Streaming"),
        )
        .into_any_element()
}

pub(super) fn render_tool_execution(
    app: &OneChat,
    execution: &ToolExecution,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let expanded = app.tool_execution_expanded(&execution.id);
    let status = tool_status_text(execution);
    let danger = matches!(
        execution.status,
        ToolExecutionStatus::Failed | ToolExecutionStatus::Interrupted
    );
    let active = execution.status.is_active();
    let execution_id = execution.id.clone();
    let mut card = div()
        .id(SharedString::from(format!(
            "tool-execution-{}",
            execution.id
        )))
        .rounded_xl()
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .shadow_xs()
        .px_3()
        .py_2()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(render_icon(
                    AppIcon::Plug,
                    if danger {
                        IconTone::Danger
                    } else if active {
                        IconTone::Accent
                    } else {
                        IconTone::Muted
                    },
                    16.0,
                    cx,
                ))
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .text_size(px(typography.metadata_size))
                        .line_height(px(typography.metadata_line_height))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(format!("{} · {}", execution.server_id, execution.tool_name)),
                )
                .child(
                    div()
                        .rounded_full()
                        .bg(if danger {
                            crate::desktop::ui::theme::palette(cx).danger_soft
                        } else {
                            cx.theme().secondary
                        })
                        .px_2()
                        .py_1()
                        .text_size(px(typography.micro_size))
                        .line_height(px(typography.micro_line_height))
                        .text_color(if danger {
                            cx.theme().danger
                        } else if active {
                            cx.theme().primary
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(status),
                )
                .child(
                    icon_button(
                        SharedString::from(format!("toggle-tool-{}", execution.id)),
                        if expanded {
                            AppIcon::ChevronUp
                        } else {
                            AppIcon::ChevronDown
                        },
                        IconTone::Muted,
                        cx,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_tool_execution(execution_id.clone(), cx)
                    })),
                ),
        );

    if expanded {
        let arguments = serde_json::to_string_pretty(&execution.arguments)
            .unwrap_or_else(|_| execution.arguments.to_string());
        card = card.child(
            div()
                .pt_3()
                .border_t_1()
                .border_color(cx.theme().border)
                .flex()
                .flex_col()
                .gap_3()
                .child(tool_detail(
                    &execution.id,
                    "ARGUMENTS",
                    arguments,
                    false,
                    app,
                    typography,
                    cx,
                ))
                .children(execution.result.as_ref().map(|result| {
                    tool_detail(
                        &execution.id,
                        "RESULT",
                        result.clone(),
                        false,
                        app,
                        typography,
                        cx,
                    )
                }))
                .children(execution.error.as_ref().map(|error| {
                    tool_detail(
                        &execution.id,
                        "ERROR",
                        error.clone(),
                        true,
                        app,
                        typography,
                        cx,
                    )
                })),
        );
    }
    card.into_any_element()
}

fn tool_detail(
    execution_id: &str,
    label: &'static str,
    content: String,
    danger: bool,
    app: &OneChat,
    typography: MessageTypography,
    cx: &App,
) -> AnyElement {
    let selection_group = app
        .chat
        .text_selection
        .group(format!("tool-{}-{execution_id}", label.to_lowercase()));
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(typography.micro_size))
                .line_height(px(typography.micro_line_height))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if danger {
                    cx.theme().danger
                } else {
                    cx.theme().muted_foreground
                })
                .child(label),
        )
        .child(
            div()
                .font(crate::desktop::ui::theme::code_font(cx))
                .text_size(px(typography.micro_size))
                .line_height(px(typography.micro_line_height + 4.0))
                .text_color(if danger {
                    cx.theme().danger
                } else {
                    cx.theme().foreground
                })
                .whitespace_normal()
                .child(selection_group.wrap(SelectableText::new(
                    selection_group.clone(),
                    0,
                    content,
                    selection_color(cx),
                ))),
        )
        .into_any_element()
}

pub(super) fn tool_status_text(execution: &ToolExecution) -> String {
    let label = match execution.status {
        ToolExecutionStatus::Queued => "Queued",
        ToolExecutionStatus::Running => "Running",
        ToolExecutionStatus::Completed => "Completed",
        ToolExecutionStatus::Failed => "Failed",
        ToolExecutionStatus::Stopped => "Stopped",
        ToolExecutionStatus::Interrupted => "Interrupted",
    };
    execution.duration_ms.map_or_else(
        || label.to_string(),
        |duration| format!("{label} · {}", format_tool_duration(duration)),
    )
}

pub(super) fn format_tool_duration(duration_ms: u64) -> String {
    if duration_ms < 1_000 {
        format!("{duration_ms} ms")
    } else {
        format!("{}.{:01} s", duration_ms / 1_000, duration_ms % 1_000 / 100)
    }
}
