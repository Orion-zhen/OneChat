use gpui::{App, Div, SharedString, div, prelude::*, px};
use gpui_component::{ActiveTheme as _, switch::Switch};

pub(super) fn tool_row(
    label: impl Into<SharedString>,
    description: Option<SharedString>,
    toggle: Switch,
    cx: &App,
) -> Div {
    div()
        .rounded(px(7.0))
        .bg(cx.theme().popover)
        .px_3()
        .py_2()
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .text_size(px(12.0))
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(label.into()),
                )
                .child(toggle),
        )
        .children(description.map(|description| {
            div()
                .pt_0p5()
                .text_size(px(11.0))
                .line_height(px(16.0))
                .text_color(cx.theme().muted_foreground)
                .child(description)
        }))
}
