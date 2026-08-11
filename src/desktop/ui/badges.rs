use gpui::{AnyElement, App, SharedString, div, prelude::*, px};
use gpui_component::ActiveTheme as _;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StatusPillBackground {
    Muted,
    Background,
}

pub(super) fn status_pill(
    label: impl Into<SharedString>,
    accent: bool,
    background: StatusPillBackground,
    cx: &App,
) -> AnyElement {
    div()
        .flex_none()
        .rounded_full()
        .bg(if accent {
            cx.theme().accent
        } else {
            match background {
                StatusPillBackground::Muted => cx.theme().muted,
                StatusPillBackground::Background => cx.theme().background,
            }
        })
        .px_2()
        .py_1()
        .text_size(px(10.0))
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(if accent {
            cx.theme().primary
        } else {
            cx.theme().muted_foreground
        })
        .child(label.into())
        .into_any_element()
}
