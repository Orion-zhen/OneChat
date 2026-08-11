use super::*;

pub(super) fn render_reasoning(
    app: &OneChat,
    message: &AssistantResponse,
    request: Option<&RequestInfo>,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> Option<AnyElement> {
    if message.thinking.is_empty() {
        return None;
    }
    render_reasoning_block(
        app,
        &message.id,
        &message.thinking,
        0,
        request.and_then(|request| request.thinking_duration_ms),
        message.status,
        request,
        typography,
        cx,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn render_reasoning_block(
    app: &OneChat,
    reasoning_id: &str,
    content: &str,
    started_after_ms: u64,
    duration_ms: Option<u64>,
    status: MessageStatus,
    request: Option<&RequestInfo>,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> Option<AnyElement> {
    if content.is_empty() {
        return None;
    }

    let live = matches!(status, MessageStatus::Pending | MessageStatus::Streaming)
        && duration_ms.is_none()
        && request.is_some();
    let expanded = app.thinking_expanded(reasoning_id, live);
    let duration = reasoning_duration_ms(app, request, started_after_ms, duration_ms, live)
        .map(format_reasoning_duration);

    let mut controls = div().flex().items_center().gap_2();
    if let Some(duration) = duration {
        controls = controls.child(
            div()
                .rounded_full()
                .bg(cx.theme().secondary)
                .px_2()
                .py_1()
                .flex()
                .items_center()
                .gap_1()
                .text_size(px(typography.micro_size))
                .line_height(px(typography.micro_line_height))
                .text_color(if live {
                    cx.theme().primary
                } else {
                    cx.theme().muted_foreground
                })
                .children(live.then(|| div().size(px(5.0)).rounded_full().bg(cx.theme().primary)))
                .child(duration),
        );
    }
    let toggle_id = reasoning_id.to_string();
    controls = controls.child(
        icon_button(
            SharedString::from(format!("thinking-{reasoning_id}")),
            if expanded {
                AppIcon::ChevronUp
            } else {
                AppIcon::ChevronDown
            },
            IconTone::Accent,
            cx,
        )
        .on_click(
            cx.listener(move |this, _, _, cx| this.toggle_thinking(toggle_id.clone(), live, cx)),
        )
        .with_animation(
            SharedString::from(format!(
                "thinking-toggle-{}-{reasoning_id}",
                if expanded { "expanded" } else { "collapsed" },
            )),
            Animation::new(Duration::from_millis(180)).with_easing(ease_out_quint()),
            |button, delta| button.opacity(0.7 + delta * 0.3),
        ),
    );

    let scroll = app.chat.thinking_scrolls.get(reasoning_id).cloned();
    let boundary_scroll = scroll.clone();
    let body = div()
        .id(SharedString::from(format!(
            "thinking-content-{reasoning_id}"
        )))
        .whitespace_normal()
        .overflow_y_scroll()
        .pr_2()
        .child(SelectableText::new(
            SharedString::from(format!("thinking-text-{reasoning_id}")),
            content.to_string(),
            app.chat.text_selection.clone(),
            selection_color(cx),
        ));
    let body = if let Some(scroll) = scroll.as_ref() {
        body.track_scroll(scroll)
    } else {
        body
    };
    let body = if let Some(motion) = app.chat.thinking_motions.get(reasoning_id).copied() {
        let target_height = if expanded {
            motion.full_height
        } else {
            motion.full_height.min(COLLAPSED_THINKING_HEIGHT)
        };
        let animation_id = SharedString::from(format!(
            "thinking-{}-{reasoning_id}",
            if expanded { "expand" } else { "collapse" },
        ));
        body.with_animation(
            animation_id,
            Animation::new(Duration::from_millis(220)).with_easing(ease_out_quint()),
            move |body, delta| {
                if delta < 1.0
                    && let Some(scroll) = scroll.as_ref()
                {
                    scroll.scroll_to_bottom();
                }
                if expanded && delta >= 1.0 {
                    body
                } else {
                    body.max_h(px(
                        motion.from_height + (target_height - motion.from_height) * delta
                    ))
                }
            },
        )
        .into_any_element()
    } else if expanded {
        body.into_any_element()
    } else {
        body.max_h(px(COLLAPSED_THINKING_HEIGHT)).into_any_element()
    };
    let body = if expanded {
        body
    } else {
        div()
            .id(SharedString::from(format!(
                "thinking-scroll-boundary-{reasoning_id}"
            )))
            .on_scroll_wheel(move |event, window, cx| {
                let Some(scroll) = boundary_scroll.as_ref() else {
                    return;
                };
                let delta_y = event.delta.pixel_delta(window.line_height()).y;
                let offset_before_event = scroll.offset().y - delta_y;
                if should_capture_nested_scroll(
                    f32::from(delta_y),
                    f32::from(offset_before_event),
                    f32::from(scroll.max_offset().y),
                ) {
                    cx.stop_propagation();
                }
            })
            .child(body)
            .into_any_element()
    };

    Some(
        div()
            .mb_4()
            .rounded_xl()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().popover)
            .shadow_xs()
            .p_4()
            .text_size(px(typography.secondary_size))
            .line_height(px(typography.secondary_line_height))
            .text_color(cx.theme().muted_foreground)
            .child(
                div()
                    .pb_2()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_size(px(typography.micro_size))
                            .line_height(px(typography.micro_line_height))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("REASONING"),
                    )
                    .child(controls),
            )
            .child(body)
            .into_any_element(),
    )
}

fn reasoning_duration_ms(
    app: &OneChat,
    request: Option<&RequestInfo>,
    started_after_ms: u64,
    duration_ms: Option<u64>,
    live: bool,
) -> Option<u64> {
    if let Some(duration_ms) = duration_ms {
        return Some(duration_ms);
    }
    if !live {
        return None;
    }
    let request = request?;
    app.chat
        .thinking_started_at
        .get(&request.id)
        .map(|started_at| {
            (started_at.elapsed().as_millis() as u64).saturating_sub(started_after_ms)
        })
}

pub(super) fn format_reasoning_duration(duration_ms: u64) -> String {
    format!("{}.{:01}s", duration_ms / 1_000, duration_ms % 1_000 / 100)
}
