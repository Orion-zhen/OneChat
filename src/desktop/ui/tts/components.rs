use gpui::{AnyElement, App, ElementId, FontWeight, div, prelude::*, px, relative};
use gpui_component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
};

use crate::desktop::{
    app::OneChat,
    ui::icons::{AppIcon, IconTone, render_icon},
};

const SOURCE_PANE_RATIO: f32 = 0.5;

pub(super) fn panel(stacked: bool, source: bool, cx: &App) -> gpui::Div {
    div()
        .min_w_0()
        .when(stacked, |panel| panel.w_full().min_h(px(520.0)))
        .when(!stacked && source, |panel| {
            panel
                .w(relative(SOURCE_PANE_RATIO))
                .flex_none()
                .h_full()
                .border_r_1()
        })
        .when(!stacked && !source, |panel| panel.flex_1().h_full())
        .when(stacked && source, |panel| panel.border_b_1())
        .border_color(cx.theme().border)
        .bg(if source {
            crate::desktop::ui::theme::palette(cx).toolbar
        } else {
            crate::desktop::ui::theme::palette(cx).canvas
        })
        .when(source, |panel| panel.p_4())
        .when(!source, |panel| panel.p_5())
        .flex()
        .flex_col()
        .gap_4()
}

pub(super) fn panel_header(
    title: &'static str,
    detail: &'static str,
    icon: AppIcon,
    cx: &App,
) -> AnyElement {
    div()
        .flex_none()
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(render_icon(icon, IconTone::Accent, 16.0, cx))
                .child(
                    div()
                        .text_size(px(16.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                ),
        )
        .child(
            div()
                .text_size(px(12.0))
                .text_color(cx.theme().muted_foreground)
                .child(detail),
        )
        .into_any_element()
}

pub(super) fn tts_connected(app: &OneChat) -> bool {
    app.tts
        .controller
        .discovery
        .health
        .as_ref()
        .is_some_and(|health| health.ready)
        && app.tts.controller.discovery.error.is_none()
}

pub(super) fn tone_color(tone: IconTone, cx: &App) -> gpui::Hsla {
    match tone {
        IconTone::Success => cx.theme().success,
        IconTone::Warning => cx.theme().warning,
        IconTone::Danger => cx.theme().danger,
        IconTone::Accent | IconTone::OnAccent => cx.theme().primary,
        _ => cx.theme().muted_foreground,
    }
}

pub(super) fn value_row(label: &str, value: &str, cx: &App) -> AnyElement {
    div()
        .min_h(px(34.0))
        .py_2()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .text_size(px(12.0))
        .child(
            div()
                .min_w_0()
                .text_color(cx.theme().muted_foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .flex_none()
                .text_right()
                .font_weight(FontWeight::MEDIUM)
                .child(value.to_string()),
        )
        .into_any_element()
}

pub(super) fn disclosure_button(
    id: impl Into<ElementId>,
    label: &'static str,
    expanded: bool,
    cx: &App,
) -> Button {
    Button::new(id).ghost().w_full().h(px(34.0)).px_2().child(
        div()
            .w_full()
            .flex()
            .items_center()
            .justify_between()
            .text_size(px(11.0))
            .font_weight(FontWeight::MEDIUM)
            .child(label)
            .child(render_icon(
                if expanded {
                    AppIcon::ChevronUp
                } else {
                    AppIcon::ChevronDown
                },
                IconTone::Muted,
                14.0,
                cx,
            )),
    )
}

pub(super) fn separator(cx: &App) -> gpui::Div {
    div().h(px(1.0)).w_full().bg(cx.theme().border)
}
