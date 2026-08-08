use super::*;

pub(super) fn render_composer(
    app: &OneChat,
    message_max_width: f32,
    typography: MessageTypography,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let generating = app.is_current_generating();
    let can_send = !app.chat.composer.read(cx).value().trim().is_empty()
        && app.current_model().is_some()
        && app.current_conversation().is_some();
    let action = if generating {
        Button::new("composer-stop")
            .danger()
            .bg(cx.theme().danger)
            .rounded(px(17.0))
            .tooltip("Stop generating")
            .size(px(34.0))
            .p_0()
            .absolute()
            .right(px(7.0))
            .bottom(px(7.0))
            .child(render_icon(AppIcon::Stop, IconTone::OnAccent, 16.0, cx))
            .on_click(cx.listener(|this, _, _, cx| this.stop_current_generation(cx)))
            .into_any_element()
    } else {
        Button::new("composer-send")
            .primary()
            .rounded(px(17.0))
            .tooltip("Send message")
            .disabled(!can_send)
            .size(px(34.0))
            .p_0()
            .absolute()
            .right(px(7.0))
            .bottom(px(7.0))
            .child(render_icon(
                AppIcon::ArrowUp,
                if can_send {
                    IconTone::OnAccent
                } else {
                    IconTone::Muted
                },
                20.0,
                cx,
            ))
            .on_click(cx.listener(|this, _, window, cx| this.send_composer(window, cx)))
            .into_any_element()
    };

    let input = div()
        .relative()
        .min_w_0()
        .flex_1()
        .overflow_hidden()
        .rounded(px(22.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().popover)
        .shadow_md()
        .child(
            Input::new(&app.chat.composer)
                .aria_label("Message")
                .appearance(false)
                .pl_4()
                .pr(px(56.0))
                .py(px(12.0))
                .text_size(px(typography.body_size))
                .line_height(px(typography.body_line_height)),
        )
        .child(action);

    div()
        .flex_none()
        .w_full()
        .px_6()
        .pt_2()
        .pb_4()
        .child(
            div()
                .mx_auto()
                .w_full()
                .max_w(px(message_max_width))
                .child(input),
        )
        .into_any_element()
}
