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

    let streaming = matches!(
        message.status,
        MessageStatus::Pending | MessageStatus::Streaming
    );
    let live = streaming && request.is_some_and(|request| request.thinking_duration_ms.is_none());
    let expanded = app.thinking_expanded(&message.id, live);
    let duration = request
        .and_then(|request| reasoning_duration_ms(app, request, live))
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
    let thinking_id = message.id.clone();
    controls = controls.child(
        icon_button(
            SharedString::from(format!("thinking-{}", message.id)),
            if expanded {
                AppIcon::ChevronUp
            } else {
                AppIcon::ChevronDown
            },
            IconTone::Accent,
            cx,
        )
        .on_click(
            cx.listener(move |this, _, _, cx| this.toggle_thinking(thinking_id.clone(), live, cx)),
        )
        .with_animation(
            SharedString::from(format!(
                "thinking-toggle-{}-{}",
                if expanded { "expanded" } else { "collapsed" },
                message.id
            )),
            Animation::new(Duration::from_millis(180)).with_easing(ease_out_quint()),
            |button, delta| button.opacity(0.7 + delta * 0.3),
        ),
    );

    let scroll = app.chat.thinking_scrolls.get(&message.id).cloned();
    let boundary_scroll = scroll.clone();
    let body = div()
        .id(SharedString::from(format!(
            "thinking-content-{}",
            message.id
        )))
        .whitespace_normal()
        .overflow_y_scroll()
        .pr_2()
        .child(SelectableText::new(
            SharedString::from(format!("thinking-text-{}", message.id)),
            message.thinking.clone(),
            app.chat.text_selection.clone(),
            selection_color(cx.theme().is_dark()),
        ));
    let body = if let Some(scroll) = scroll.as_ref() {
        body.track_scroll(scroll)
    } else {
        body
    };
    let body = if let Some(motion) = app.chat.thinking_motions.get(&message.id).copied() {
        let target_height = if expanded {
            motion.full_height
        } else {
            motion.full_height.min(COLLAPSED_THINKING_HEIGHT)
        };
        let animation_id = SharedString::from(format!(
            "thinking-{}-{}",
            if expanded { "expand" } else { "collapse" },
            message.id
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
                "thinking-scroll-boundary-{}",
                message.id
            )))
            .on_scroll_wheel(move |event, window, cx| {
                let Some(scroll) = boundary_scroll.as_ref() else {
                    return;
                };
                let delta_y = event.delta.pixel_delta(window.line_height()).y;
                // GPUI scrolls the child before this ancestor listener runs.
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

fn reasoning_duration_ms(app: &OneChat, request: &RequestInfo, live: bool) -> Option<u64> {
    if let Some(duration) = request.thinking_duration_ms {
        return Some(duration);
    }
    if !live {
        return None;
    }
    app.chat
        .thinking_started_at
        .get(&request.id)
        .map(|started_at| started_at.elapsed().as_millis() as u64)
}

pub(super) fn format_reasoning_duration(duration_ms: u64) -> String {
    format!("{}.{:01}s", duration_ms / 1_000, duration_ms % 1_000 / 100)
}
