use gpui::{App, FocusHandle, FontWeight, Role, SharedString, div, prelude::*, px};
use gpui_component::{ActiveTheme as _, FocusTrapElement as _};

use crate::desktop::ui::motion::translated_y;

pub(crate) fn panel(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    focus: &FocusHandle,
    width: f32,
    padding: f32,
    cx: &App,
) -> impl IntoElement + Styled + ParentElement {
    div()
        .id(id.into())
        .role(Role::Dialog)
        .aria_label(label.into())
        .track_focus(focus)
        .focus_trap("shell-overlay-focus", focus)
        .w_full()
        .max_w(px(width))
        .max_h_full()
        .p(px(padding))
        .rounded(px(22.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(crate::desktop::ui::theme::palette(cx).overlay_panel)
        .shadow_xl()
        .flex()
        .flex_col()
        .overflow_hidden()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
}

pub(crate) fn header(
    title: &'static str,
    subtitle: &'static str,
    close: impl IntoElement,
    cx: &App,
) -> gpui::Div {
    div()
        .w_full()
        .flex()
        .items_start()
        .child(
            div()
                .min_w_0()
                .flex_1()
                .pr(px(4.0))
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_size(px(19.0))
                        .line_height(px(24.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(subtitle),
                ),
        )
        .child(div().flex_none().child(close))
}

pub(crate) fn backdrop(
    id: impl Into<SharedString>,
    panel: impl IntoElement,
    progress: f32,
    reduce_motion: bool,
    cx: &App,
) -> impl IntoElement + InteractiveElement {
    let offset = if reduce_motion {
        0.0
    } else {
        -8.0 * (1.0 - progress)
    };
    div()
        .id(id.into())
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .left_0()
        .occlude()
        .bg(cx.theme().overlay)
        .opacity(progress)
        .child(
            div()
                .size_full()
                .p_4()
                .pt_12()
                .flex()
                .items_start()
                .justify_center()
                .child(translated_y(panel, px(offset))),
        )
}
