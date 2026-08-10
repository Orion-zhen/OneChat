use super::super::super::*;

pub(super) fn readonly_field(
    label: &'static str,
    value: impl Into<SharedString>,
    cx: &App,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(field_label(label, cx))
        .child(
            div()
                .min_h(px(40.0))
                .rounded_lg()
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().muted)
                .px_3()
                .flex()
                .items_center()
                .text_sm()
                .child(value.into()),
        )
        .into_any_element()
}

pub(super) fn field_label(label: &'static str, cx: &App) -> AnyElement {
    div()
        .text_size(px(11.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(cx.theme().muted_foreground)
        .child(label)
        .into_any_element()
}
