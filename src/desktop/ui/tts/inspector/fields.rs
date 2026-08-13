use gpui::{AnyElement, App, Context, Entity, FontWeight, IntoElement, div, prelude::*, px};
use gpui_component::{
    ActiveTheme as _,
    slider::{Slider, SliderState},
};

use crate::desktop::app::OneChat;

pub(super) fn settings_group(
    title: &'static str,
    detail: &'static str,
    content: impl IntoElement,
    cx: &App,
) -> AnyElement {
    div()
        .rounded(px(16.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(crate::desktop::ui::theme::palette(cx).panel)
        .p_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .pt_1()
                        .text_size(px(11.0))
                        .line_height(px(16.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                ),
        )
        .child(content)
        .into_any_element()
}

pub(super) fn value_setting(
    label: &'static str,
    detail: &'static str,
    value: &str,
    cx: &App,
) -> AnyElement {
    div()
        .py_2()
        .flex()
        .items_start()
        .justify_between()
        .gap_4()
        .child(setting_label(label, detail, cx))
        .child(
            div()
                .flex_none()
                .pt_0p5()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .child(value.to_string()),
        )
        .into_any_element()
}

pub(super) fn slider_setting(
    label: &'static str,
    detail: &'static str,
    state: &Entity<SliderState>,
    value: String,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap_4()
                .child(setting_label(label, detail, cx))
                .child(
                    div()
                        .flex_none()
                        .pt_0p5()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(value),
                ),
        )
        .child(Slider::new(state).w_full().bg(cx.theme().primary))
        .into_any_element()
}

fn setting_label(label: &'static str, detail: &'static str, cx: &App) -> gpui::Div {
    div()
        .min_w_0()
        .child(
            div()
                .text_size(px(12.0))
                .font_weight(FontWeight::MEDIUM)
                .child(label),
        )
        .child(
            div()
                .pt_0p5()
                .text_size(px(10.0))
                .line_height(px(15.0))
                .text_color(cx.theme().muted_foreground)
                .child(detail),
        )
}

pub(super) fn percent(value: f32) -> String {
    format!("{:.0}%", value * 100.0)
}
