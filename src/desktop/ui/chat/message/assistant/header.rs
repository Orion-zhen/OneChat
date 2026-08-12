use super::*;

pub(super) fn render_message_header(
    app: &OneChat,
    turn: &Turn,
    message: &AssistantResponse,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let assistant_label = format!("{} · {}", message.model_name, message.provider_name);
    let multiple_responses = turn.responses.len() > 1;
    let header_content = if multiple_responses {
        let tabs_scroll = app
            .chat
            .horizontal_scrolls
            .handle(format!("response-tabs:{}", turn.id));
        let boundary_scroll = tabs_scroll.clone();
        let mut tabs = div()
            .id(SharedString::from(format!("response-tabs-{}", turn.id)))
            .track_scroll(&tabs_scroll)
            .on_scroll_wheel(move |event, _, cx| {
                if nested_horizontal_scroll_captures(event, &boundary_scroll) {
                    cx.stop_propagation();
                }
            })
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
            let mut tab = response_tab_button(
                SharedString::from(format!("response-tab-{}", response.id)),
                label,
                typography,
            )
            .selected(selected)
            .flex()
            .items_center()
            .gap_1()
            .children(
                context.then(|| render_icon(AppIcon::ContextSelected, IconTone::Accent, 15.0, cx)),
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
            });
            #[cfg(target_os = "macos")]
            {
                let pressure_turn_id = tab_turn_id.clone();
                let pressure_response_id = tab_response_id.clone();
                tab = tab
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, _| this.begin_response_tab_pressure()),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, _, _| this.cancel_response_tab_pressure()),
                    )
                    .on_mouse_pressure(cx.listener(
                        move |this, event: &gpui::MousePressureEvent, _, cx| {
                            if this.update_response_tab_pressure(
                                pressure_turn_id.clone(),
                                pressure_response_id.clone(),
                                event,
                                cx,
                            ) {
                                cx.stop_propagation();
                            }
                        },
                    ));
            }
            tabs = tabs.child(tab.on_click(cx.listener(move |this, _, _, cx| {
                #[cfg(target_os = "macos")]
                if this.consume_response_tab_click(&tab_response_id) {
                    return;
                }
                this.show_response(tab_turn_id.clone(), tab_response_id.clone(), cx);
            })));
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
    div()
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
        )
        .into_any_element()
}
