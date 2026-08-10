use super::*;

mod actions;
mod content;
mod header;

use actions::render_message_actions;
use content::render_message_content;
use header::render_message_header;

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
    let latest = app.is_latest_turn(&turn.id);
    let generating = app.is_current_generating();
    let content = render_message_content(app, message, scale_factor, typography, cx);
    let actions = render_message_actions(
        app,
        turn,
        message,
        latest,
        generating,
        action_group.clone(),
        cx,
    );
    let header = render_message_header(turn, message, typography, cx);
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
    if request.usage.input_tokens.is_some() || request.usage.output_tokens.is_some() {
        let format_tokens = |tokens: Option<u64>| {
            tokens.map_or_else(
                || "—".into(),
                |tokens| format!("{}{tokens}", if request.usage.estimated { "~" } else { "" }),
            )
        };
        stats.push(format!(
            "Tokens {} in / {} out",
            format_tokens(request.usage.input_tokens),
            format_tokens(request.usage.output_tokens)
        ));
    }
    if let Some(tokens) = request.usage.output_tokens
        && let (Some(duration_ms), Some(ttft_ms)) = (request.duration_ms, request.ttft_ms)
    {
        let generation_ms = duration_ms.saturating_sub(ttft_ms);
        if generation_ms > 0 {
            stats.push(format!(
                "{:.1} tok/s",
                tokens as f64 * 1000.0 / generation_ms as f64
            ));
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
            crate::desktop::ui::theme::palette(cx).danger_soft
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
            .bg(crate::desktop::ui::theme::palette(cx).danger_subtle)
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
