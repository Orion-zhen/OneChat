use std::f32::consts::PI;

use gpui::{Anchor, Hsla, PathBuilder, canvas, point};
use gpui_component::popover::Popover;

use super::*;
use crate::application::context_usage::{
    ContextUsage, ContextUsageReference, ContextUsageSource, context_usage_from_input_tokens,
    project_context_usage, provider_usage_reference,
};

pub(super) fn render_context_indicator(
    app: &OneChat,
    progress: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
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

    let progress = progress.clamp(0.0, 1.0);
    let offset = if cx.reduce_motion() {
        0.0
    } else {
        6.0 * (1.0 - progress)
    };
    let palette = *crate::desktop::ui::theme::palette(cx);
    let panel = div()
        .w(px(320.0))
        .p(px(16.0))
        .rounded(px(14.0))
        .border_1()
        .border_color(palette.floating_border)
        .bg(palette.floating_glass)
        .shadow(vec![BoxShadow {
            color: palette.floating_shadow,
            offset: point(px(0.0), px(6.0)),
            blur_radius: px(18.0),
            spread_radius: px(-7.0),
            inset: false,
        }])
        .opacity(progress)
        .child(context_usage_panel(usage, cx));
    let app_entity = cx.entity();

    Popover::new("composer-context-usage-popover")
        .anchor(Anchor::TopRight)
        .appearance(false)
        .open(app.chat.context_usage_popover_open)
        .on_open_change(move |open, _, cx| {
            app_entity.update(cx, |app, cx| {
                app.set_context_usage_popover_open(*open, cx);
            });
        })
        .trigger(trigger)
        .child(translated_y(panel, px(offset)))
        .into_any_element()
}

fn current_context_usage(app: &OneChat, cx: &App) -> Option<ContextUsage> {
    let conversation = app.current_conversation()?;
    let model = app.current_model()?;
    let mut messages = app.current_context_messages();
    let current_request = app.current_request().filter(|request| {
        request.model_id.as_deref() == Some(model.id.as_str())
            && request.provider_id.as_deref() == Some(model.provider_id.as_str())
    });
    if let Some((input_tokens, source)) = current_request.and_then(running_request_input_usage) {
        return Some(context_usage_from_input_tokens(
            input_tokens,
            &messages,
            model.context_window_tokens,
            source,
        ));
    }
    let reference_request =
        current_request.filter(|request| request.status == crate::domain::RequestStatus::Completed);
    if let Some(opening) = reference_request
        .and_then(|request| request.assistant_opening.as_ref())
        .filter(|opening| opening.template == conversation.assistant_opening)
    {
        messages[0] = crate::domain::Message::assistant(opening.resolved.clone());
    }
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

fn running_request_input_usage(request: &RequestInfo) -> Option<(u64, ContextUsageSource)> {
    if !matches!(
        request.status,
        crate::domain::RequestStatus::Sending | crate::domain::RequestStatus::Streaming
    ) {
        return None;
    }

    request
        .last_step_input_tokens
        .filter(|tokens| *tokens > 0)
        .map(|tokens| (tokens, ContextUsageSource::ProviderAnchored))
        .or_else(|| {
            request
                .last_step_estimated_input_tokens
                .filter(|tokens| *tokens > 0)
                .map(|tokens| (tokens, ContextUsageSource::Estimated))
        })
        .or_else(|| {
            request
                .usage
                .input_tokens
                .filter(|tokens| *tokens > 0)
                .map(|tokens| {
                    let source = if request.usage.estimated {
                        ContextUsageSource::Estimated
                    } else {
                        ContextUsageSource::ProviderAnchored
                    };
                    (tokens, source)
                })
        })
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
    let mut messages = crate::application::generation::history_for_turn(
        &app.data.snapshot.current_turns,
        turn,
        history_limit,
    );
    if let Some(opening) = request.assistant_opening.as_ref() {
        messages.insert(
            0,
            crate::domain::Message::assistant(opening.resolved.clone()),
        );
    }
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
        .size(px(18.0))
        .flex_none()
        .grid()
        .grid_cols(1)
        .grid_rows(1)
        .child(div().col_start(1).row_start(1).size_full().child(ring))
        .children(remaining_ratio.is_none().then(|| {
            div()
                .col_start(1)
                .row_start(1)
                .mt(px(8.0))
                .ml(px(6.0))
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
    let palette = *crate::desktop::ui::theme::palette(cx);
    let known_window = usage.context_window_tokens;
    let over_limit = known_window.is_some_and(|window| usage.input_tokens > u64::from(window));
    let metric = usage.remaining_ratio.map_or_else(
        || format!("~{}", format_tokens(usage.input_tokens)),
        |ratio| format!("~{}%", percent(ratio)),
    );
    let supporting = known_window.map_or_else(
        || "Estimated input tokens".to_string(),
        |window| {
            format!(
                "~{} of {} estimated input tokens used",
                format_tokens(usage.input_tokens),
                crate::domain::format_compact_token_count(window)
            )
        },
    );

    let header = div()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .text_size(px(11.0))
                .line_height(px(15.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(palette.muted_foreground)
                .child(if known_window.is_some() {
                    "Context remaining"
                } else {
                    "Context usage"
                }),
        )
        .child(
            div()
                .text_size(px(28.0))
                .line_height(px(32.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(metric),
        )
        .child(
            div()
                .text_size(px(11.0))
                .line_height(px(16.0))
                .text_color(palette.muted_foreground)
                .child(supporting),
        )
        .children(usage.remaining_ratio.map(|ratio| {
            let color = match ratio {
                ratio if ratio <= 0.1 => palette.danger,
                ratio if ratio <= 0.25 => cx.theme().warning,
                _ => palette.accent,
            };
            div()
                .mt_2()
                .h(px(4.0))
                .w_full()
                .overflow_hidden()
                .rounded_full()
                .bg(palette.border)
                .child(
                    div()
                        .h_full()
                        .w(relative(ratio.clamp(0.0, 1.0)))
                        .rounded_full()
                        .bg(color),
                )
        }));

    let details = div()
        .rounded(px(10.0))
        .bg(palette.secondary)
        .px_3()
        .child(detail_row(
            "Estimate basis",
            match usage.source {
                ContextUsageSource::Estimated => "Local estimate",
                ContextUsageSource::ProviderAnchored => "Provider-adjusted",
            },
            cx,
        ))
        .child(div().h(px(1.0)).w_full().bg(palette.border))
        .child(detail_row(
            "Reasoning history",
            if usage.replays_reasoning {
                "Included"
            } else {
                "Not present"
            },
            cx,
        ));

    let mut panel = div().flex().flex_col().gap_4().child(header);
    if known_window.is_none() {
        panel = panel.child(context_notice(
            "Context limit unavailable",
            "Set a limit in model settings to see remaining capacity.",
            palette.accent,
            palette.accent_soft,
        ));
    } else if over_limit {
        panel = panel.child(context_notice(
            "Over the configured limit",
            "Older turns may be removed before the message is sent.",
            cx.theme().warning,
            cx.theme().warning.opacity(0.12),
        ));
    }

    panel
        .child(details)
        .child(
            div()
                .min_w_0()
                .flex()
                .items_center()
                .gap_2()
                .text_size(px(10.0))
                .line_height(px(14.0))
                .text_color(palette.muted_foreground)
                .child(render_icon(AppIcon::Info, IconTone::Muted, 13.0, cx))
                .child(
                    div().min_w_0().flex_1().whitespace_normal().child(
                        "Actual usage can vary with attachments, tools, and provider framing.",
                    ),
                ),
        )
        .into_any_element()
}

fn context_notice(
    title: &'static str,
    detail: &'static str,
    tone: Hsla,
    background: Hsla,
) -> AnyElement {
    div()
        .rounded(px(10.0))
        .bg(background)
        .p_3()
        .flex()
        .items_start()
        .gap_2()
        .child(
            div()
                .mt(px(5.0))
                .size(px(6.0))
                .flex_none()
                .rounded_full()
                .bg(tone),
        )
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(11.0))
                        .line_height(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .line_height(px(14.0))
                        .child(detail),
                ),
        )
        .into_any_element()
}

fn detail_row(label: &'static str, value: &'static str, cx: &App) -> AnyElement {
    div()
        .min_h(px(34.0))
        .py_2()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .text_size(px(11.0))
        .line_height(px(16.0))
        .child(div().text_color(cx.theme().muted_foreground).child(label))
        .child(
            div()
                .text_right()
                .font_weight(FontWeight::MEDIUM)
                .child(value),
        )
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn running_request_keeps_the_prepared_full_context_estimate() {
        let mut request = RequestInfo::new("conversation", "turn", "response");
        request.usage.input_tokens = Some(12_000);
        request.usage.estimated = true;

        assert_eq!(
            running_request_input_usage(&request),
            Some((12_000, ContextUsageSource::Estimated))
        );
    }

    #[test]
    fn running_request_prefers_current_step_provider_usage() {
        let mut request = RequestInfo::new("conversation", "turn", "response");
        request.status = crate::domain::RequestStatus::Streaming;
        request.usage.input_tokens = Some(12_000);
        request.usage.estimated = true;
        request.last_step_estimated_input_tokens = Some(12_500);
        request.last_step_input_tokens = Some(13_000);

        assert_eq!(
            running_request_input_usage(&request),
            Some((13_000, ContextUsageSource::ProviderAnchored))
        );
    }

    #[test]
    fn completed_request_is_left_to_next_turn_projection() {
        let mut request = RequestInfo::new("conversation", "turn", "response");
        request.status = crate::domain::RequestStatus::Completed;
        request.usage.input_tokens = Some(12_000);

        assert_eq!(running_request_input_usage(&request), None);
    }
}
