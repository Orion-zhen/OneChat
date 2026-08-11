use std::f32::consts::PI;

use gpui::{Anchor, Hsla, PathBuilder, canvas, point};
use gpui_component::popover::Popover;

use super::*;
use crate::application::context_usage::{
    ContextUsage, ContextUsageReference, ContextUsageSource, project_context_usage,
    provider_usage_reference,
};

pub(super) fn render_context_indicator(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let usage = current_context_usage(app, cx).expect("context indicator requires a model");
    let tooltip = usage.remaining_ratio.map_or_else(
        || "Context window unavailable".to_string(),
        |ratio| format!("Context remaining: ~{}%", percent(ratio)),
    );
    let color = indicator_color(usage.remaining_ratio, cx);
    let trigger = Button::new("composer-context-usage")
        .ghost()
        .rounded(px(17.0))
        .tooltip(tooltip)
        .size(px(34.0))
        .p_0()
        .child(capacity_ring(usage.remaining_ratio, color, cx));

    Popover::new("composer-context-usage-popover")
        .anchor(Anchor::TopRight)
        .w(px(300.0))
        .p(px(14.0))
        .rounded(px(12.0))
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .shadow_md()
        .trigger(trigger)
        .child(context_usage_panel(usage, cx))
        .into_any_element()
}

fn current_context_usage(app: &OneChat, cx: &App) -> Option<ContextUsage> {
    let conversation = app.current_conversation()?;
    let model = app.current_model()?;
    let mut messages = app.current_context_messages();
    let mut audio_duration_ms = app.current_context_audio_duration_ms();
    let draft = app.chat.composer.read(cx).value();
    if !draft.is_empty() || !app.chat.attachments.is_empty() {
        messages.push(crate::domain::Message::user(draft.to_string()));
        audio_duration_ms = app
            .chat
            .attachments
            .iter()
            .filter_map(|attachment| attachment.audio.as_ref())
            .fold(audio_duration_ms, |duration_ms, audio| {
                duration_ms.saturating_add(audio.duration_ms)
            });
    }
    let reference_request = app.current_request().filter(|request| {
        request.status == crate::domain::RequestStatus::Completed
            && request.model_id.as_deref() == Some(model.id.as_str())
            && request.provider_id.as_deref() == Some(model.provider_id.as_str())
    });
    let system_prompt = reference_request
        .and_then(|request| request.system_prompt.as_ref())
        .filter(|prompt| prompt.template == conversation.system_prompt)
        .map_or(conversation.system_prompt.as_str(), |prompt| {
            prompt.resolved.as_str()
        });
    let reference =
        reference_request.and_then(|request| request_usage_reference(app, request, system_prompt));

    Some(project_context_usage(
        system_prompt,
        &messages,
        audio_duration_ms,
        model.context_window_tokens,
        reference,
    ))
}

fn request_usage_reference(
    app: &OneChat,
    request: &RequestInfo,
    system_prompt: &str,
) -> Option<ContextUsageReference> {
    if let (Some(input_tokens), Some(estimated_input_tokens)) = (
        request.last_step_input_tokens,
        request.last_step_estimated_input_tokens,
    ) {
        return Some(ContextUsageReference {
            input_tokens,
            estimated_input_tokens,
        });
    }
    if request.usage.estimated || request.tool_call_count > 0 {
        return None;
    }

    let (turn, _) = app.response(&request.response_id)?;
    let history_limit = request
        .context
        .map_or(crate::domain::HistoryLimit::Unlimited, |context| {
            if context.history_limit == crate::domain::HistoryLimit::Unlimited
                && !context.limited_by_context_window
            {
                crate::domain::HistoryLimit::Unlimited
            } else {
                crate::domain::HistoryLimit::Last(context.included_history_turns)
            }
        });
    let messages = crate::application::generation::history_for_turn(
        &app.data.snapshot.current_turns,
        turn,
        history_limit,
    );
    let audio_duration_ms = crate::application::generation::history_audio_duration_ms_for_turn(
        &app.data.snapshot.current_turns,
        turn,
        history_limit,
    );
    provider_usage_reference(
        request.usage.input_tokens?,
        system_prompt,
        &messages,
        audio_duration_ms,
    )
}

fn indicator_color(remaining_ratio: Option<f32>, cx: &App) -> Hsla {
    match remaining_ratio {
        Some(ratio) if ratio <= 0.1 => cx.theme().danger,
        Some(ratio) if ratio <= 0.25 => cx.theme().warning,
        _ => cx.theme().muted_foreground,
    }
}

fn capacity_ring(remaining_ratio: Option<f32>, color: Hsla, cx: &App) -> AnyElement {
    let track = cx.theme().border;
    let ring = circular_progress(remaining_ratio.unwrap_or(0.0), track, color);
    div()
        .relative()
        .size(px(18.0))
        .flex_none()
        .child(ring)
        .children(remaining_ratio.is_none().then(|| {
            div()
                .absolute()
                .left(px(6.0))
                .top(px(8.0))
                .w(px(6.0))
                .h(px(1.5))
                .rounded_full()
                .bg(color)
        }))
        .into_any_element()
}

fn circular_progress(progress: f32, track: Hsla, color: Hsla) -> AnyElement {
    let size = px(18.0);
    let stroke = px(1.8);
    let radius = px(7.0);
    let progress = progress.clamp(0.0, 1.0);

    canvas(
        |_, _, _| {},
        move |bounds, _, window, _| {
            let center_x = bounds.origin.x + bounds.size.width / 2.0;
            let center_y = bounds.origin.y + bounds.size.height / 2.0;
            let mut background = PathBuilder::stroke(stroke);
            background.move_to(point(center_x + radius, center_y));
            background.arc_to(
                point(radius, radius),
                px(0.0),
                false,
                true,
                point(center_x - radius, center_y),
            );
            background.arc_to(
                point(radius, radius),
                px(0.0),
                false,
                true,
                point(center_x + radius, center_y),
            );
            background.close();
            if let Ok(path) = background.build() {
                window.paint_path(path, track);
            }

            if progress <= 0.0 {
                return;
            }
            let mut foreground = PathBuilder::stroke(stroke);
            if progress >= 0.999 {
                foreground.move_to(point(center_x + radius, center_y));
                foreground.arc_to(
                    point(radius, radius),
                    px(0.0),
                    false,
                    true,
                    point(center_x - radius, center_y),
                );
                foreground.arc_to(
                    point(radius, radius),
                    px(0.0),
                    false,
                    true,
                    point(center_x + radius, center_y),
                );
                foreground.close();
            } else {
                foreground.move_to(point(center_x, center_y - radius));
                let angle = -PI / 2.0 + progress * 2.0 * PI;
                foreground.arc_to(
                    point(radius, radius),
                    px(0.0),
                    progress > 0.5,
                    true,
                    point(
                        center_x + radius * angle.cos(),
                        center_y + radius * angle.sin(),
                    ),
                );
            }
            if let Ok(path) = foreground.build() {
                window.paint_path(path, color);
            }
        },
    )
    .size(size)
    .into_any_element()
}

fn context_usage_panel(usage: ContextUsage, cx: &App) -> AnyElement {
    let title = div()
        .text_size(px(13.0))
        .font_weight(FontWeight::SEMIBOLD)
        .child(if usage.context_window_tokens.is_some() {
            "Context Remaining"
        } else {
            "Context Usage"
        });
    let summary = usage.remaining_ratio.map_or_else(
        || format!("~{} input tokens used", format_tokens(usage.input_tokens)),
        |ratio| format!("~{}% remaining", percent(ratio)),
    );
    let mut panel = div().flex().flex_col().gap_3().child(title).child(
        div()
            .text_size(px(22.0))
            .line_height(px(26.0))
            .font_weight(FontWeight::SEMIBOLD)
            .child(summary),
    );

    if let Some(window) = usage.context_window_tokens {
        panel = panel.child(detail_row(
            "Estimated input",
            &format!(
                "~{} of {}",
                format_tokens(usage.input_tokens),
                crate::domain::format_compact_token_count(window)
            ),
            cx,
        ));
    } else {
        panel = panel.child(
            div()
                .rounded_lg()
                .bg(cx.theme().muted)
                .p_3()
                .text_size(px(11.0))
                .line_height(px(16.0))
                .text_color(cx.theme().muted_foreground)
                .child(
                    "The selected model does not provide a context window. Set it in the model settings to calculate remaining capacity.",
                ),
        );
    }

    panel = panel
        .child(detail_row(
            "Usage basis",
            match usage.source {
                ContextUsageSource::Estimated => "Local estimate",
                ContextUsageSource::ProviderAnchored => "Last provider usage + current changes",
            },
            cx,
        ))
        .child(detail_row(
            "Replayed reasoning",
            if usage.replays_reasoning {
                "Included"
            } else {
                "Not present"
            },
            cx,
        ));

    if usage
        .context_window_tokens
        .is_some_and(|window| usage.input_tokens > u64::from(window))
    {
        panel = panel.child(
            div()
                .rounded_lg()
                .bg(cx.theme().warning.opacity(0.12))
                .p_3()
                .text_size(px(11.0))
                .line_height(px(16.0))
                .child(
                    "This exceeds the configured window. OneChat will remove older turns before sending when possible.",
                ),
        );
    }

    panel
        .child(
            div()
                .text_size(px(10.0))
                .line_height(px(14.0))
                .text_color(cx.theme().muted_foreground)
                .child(
                    "Attachments, tool definitions, prompt variables, and provider framing can change actual usage.",
                ),
        )
        .into_any_element()
}

fn detail_row(label: &'static str, value: &str, cx: &App) -> AnyElement {
    div()
        .flex()
        .items_start()
        .justify_between()
        .gap_3()
        .text_size(px(11.0))
        .child(div().text_color(cx.theme().muted_foreground).child(label))
        .child(div().text_right().child(value.to_string()))
        .into_any_element()
}

fn percent(ratio: f32) -> u32 {
    (ratio.clamp(0.0, 1.0) * 100.0).round() as u32
}

fn format_tokens(tokens: u64) -> String {
    u32::try_from(tokens).map_or_else(
        |_| format!("{:.1}B", tokens as f64 / 1_000_000_000.0),
        crate::domain::format_compact_token_count,
    )
}
