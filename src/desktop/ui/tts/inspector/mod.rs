mod fields;
mod sections;

use gpui::{AnyElement, Context, FontWeight, MouseButton, div, prelude::*, px};
use gpui_component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
};

use crate::desktop::{
    app::OneChat,
    ui::{
        icons::{AppIcon, IconActionSize::Compact, IconTone, render_icon},
        motion::translated_x,
    },
};

const INSPECTOR_WIDTH: f32 = 460.0;

pub(crate) fn render_overlay(
    app: &OneChat,
    progress: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .id("tts-inspector-overlay")
        .absolute()
        .top_0()
        .right_0()
        .bottom_0()
        .left_0()
        .occlude()
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _, _, cx| this.set_tts_inspector_open(false, cx)),
        )
        .child(
            div()
                .absolute()
                .top(px(60.0))
                .right_0()
                .bottom_0()
                .left_0()
                .child(translated_x(
                    render_drawer(app, cx),
                    px((INSPECTOR_WIDTH + 16.0) * (1.0 - progress)),
                )),
        )
        .into_any_element()
}

fn render_drawer(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let config = &app.tts.controller.config;
    div()
        .absolute()
        .occlude()
        .top(px(8.0))
        .right(px(8.0))
        .bottom(px(8.0))
        .w(px(INSPECTOR_WIDTH))
        .rounded(px(20.0))
        .border_1()
        .border_color(crate::desktop::ui::theme::palette(cx).floating_border)
        .bg(crate::desktop::ui::theme::palette(cx).floating_glass)
        .shadow_lg()
        .flex()
        .flex_col()
        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
        .child(
            div()
                .flex_none()
                .px_4()
                .pt_4()
                .pb_3()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .child(
                            div()
                                .text_size(px(16.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("TTS Tuning"),
                        )
                        .child(
                            div()
                                .text_size(px(11.0))
                                .text_color(cx.theme().muted_foreground)
                                .child("Changes apply to the next run"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(
                            Compact
                                .icon_action(
                                    "reset-tts-tuning",
                                    AppIcon::Regenerate,
                                    IconTone::Muted,
                                    "Reset tuning to defaults",
                                    cx,
                                )
                                .on_click(cx.listener(|this, _, _, cx| this.reset_tts_tuning(cx))),
                        )
                        .child(
                            Button::new("close-tts-inspector")
                                .ghost()
                                .size(px(30.0))
                                .p_0()
                                .tooltip("Close TTS tuning")
                                .child(render_icon(AppIcon::Close, IconTone::Muted, 16.0, cx))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_tts_inspector_open(false, cx)
                                })),
                        ),
                ),
        )
        .child(
            div()
                .id("tts-inspector-scroll")
                .min_h_0()
                .flex_1()
                .overflow_y_scroll()
                .px_4()
                .pb_4()
                .flex()
                .flex_col()
                .gap_4()
                .child(sections::generation(config, cx))
                .child(sections::segmentation(app, cx))
                .child(sections::audio_validation(app, cx))
                .child(sections::transcript_validation(app, cx)),
        )
        .into_any_element()
}
