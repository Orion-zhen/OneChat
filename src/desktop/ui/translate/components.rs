use gpui::{App, FontWeight, div, prelude::*, px};
use gpui_component::ActiveTheme as _;

use crate::desktop::ui::{
    icons::{AppIcon, IconTone, render_icon},
    theme,
};

pub(super) fn panel(stacked: bool, cx: &App) -> gpui::Div {
    div()
        .min_w_0()
        .when(stacked, |panel| panel.w_full().flex_none().min_h(px(340.0)))
        .when(!stacked, |panel| panel.min_h_0().flex_1())
        .rounded(px(16.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(theme::palette(cx).raised)
        .shadow_xs()
        .flex()
        .flex_col()
}

pub(super) fn panel_header(
    title: &'static str,
    icon: AppIcon,
    narrow: bool,
    cx: &App,
) -> gpui::Div {
    div()
        .w_full()
        .flex_none()
        .px_4()
        .border_b_1()
        .border_color(cx.theme().border)
        .rounded_tl(px(15.0))
        .rounded_tr(px(15.0))
        .bg(theme::palette(cx).toolbar)
        .flex()
        .when(narrow, |header| {
            header
                .h(px(96.0))
                .py_3()
                .flex_col()
                .justify_center()
                .gap_2()
        })
        .when(!narrow, |header| header.h(px(56.0)).items_center().gap_3())
        .child(
            div()
                .flex_none()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .size(px(28.0))
                        .flex_none()
                        .rounded(px(8.0))
                        .bg(theme::palette(cx).accent_soft)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(render_icon(icon, IconTone::Accent, 15.0, cx)),
                )
                .child(
                    div()
                        .flex_none()
                        .whitespace_nowrap()
                        .text_size(px(13.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                ),
        )
}

pub(super) fn header_controls(narrow: bool) -> gpui::Div {
    div()
        .min_w_0()
        .flex()
        .items_center()
        .gap_1()
        .when(narrow, |controls| controls.w_full())
        .when(!narrow, |controls| controls.flex_1())
}

pub(super) fn language_select_slot(narrow: bool) -> gpui::Div {
    div()
        .min_w_0()
        .h(px(32.0))
        .w(px(220.0))
        .max_w_full()
        .when(!narrow, |slot| slot.flex_none())
}
