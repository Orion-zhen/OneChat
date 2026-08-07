use super::*;

pub(super) fn render_composer(
    app: &OneChat,
    colors: Colors,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let generating = app.is_current_generating();
    let can_send = !app.chat.composer.read(cx).text().trim().is_empty()
        && app.current_model().is_some()
        && app.current_conversation().is_some();
    let action = if generating {
        div()
            .id("composer-stop")
            .absolute()
            .right(px(9.0))
            .bottom(px(9.0))
            .size(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(colors.danger)
            .cursor_pointer()
            .hover(|style| style.opacity(0.88))
            .active(|style| style.opacity(0.72))
            .child(render_icon(
                Icon::Stop,
                IconTone::OnAccent,
                colors,
                scale_factor,
                18.0,
            ))
            .on_click(cx.listener(|this, _, _, cx| this.stop_current_generation(cx)))
            .into_any_element()
    } else if can_send {
        primary_icon_button("composer-send", Icon::ArrowUp, colors, scale_factor)
            .absolute()
            .right(px(9.0))
            .bottom(px(9.0))
            .on_click(cx.listener(|this, _, _, cx| this.send_composer(cx)))
            .into_any_element()
    } else {
        div()
            .id("composer-send-disabled")
            .absolute()
            .right(px(9.0))
            .bottom(px(9.0))
            .size(px(32.0))
            .flex()
            .items_center()
            .justify_center()
            .rounded_full()
            .bg(colors.raised)
            .cursor_default()
            .child(render_icon(
                Icon::ArrowUp,
                IconTone::Muted,
                colors,
                scale_factor,
                20.0,
            ))
            .into_any_element()
    };

    let input = div()
        .relative()
        .min_w_0()
        .flex_1()
        .child(app.chat.composer.clone())
        .child(action);

    div()
        .flex_none()
        .w_full()
        .px_6()
        .pb_5()
        .child(div().mx_auto().w_full().max_w(px(800.0)).child(input))
        .into_any_element()
}
