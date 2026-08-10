use super::*;

pub(super) fn render_settings_top_bar(cx: &mut Context<OneChat>) -> AnyElement {
    div()
        .h(px(60.0))
        .flex_none()
        .flex()
        .items_center()
        .justify_between()
        .px_5()
        .bg(cx.theme().title_bar)
        .shadow_xs()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .h_full()
                .flex()
                .items_center()
                .text_size(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child("Settings"),
        )
        .child(
            large_icon_button("chat-page", AppIcon::Close, IconTone::Muted, cx).on_click(
                cx.listener(|this, _, window, cx| this.request_leave_settings(window, cx)),
            ),
        )
        .into_any_element()
}
