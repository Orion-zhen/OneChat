use gpui::{AnyElement, App, Context, FontWeight, div, prelude::*, px};
use gpui_component::{
    ActiveTheme as _, Disableable as _,
    button::{Button, ButtonVariants as _},
};

use super::{components::tts_connected, output, source};
use crate::desktop::{
    app::OneChat,
    ui::icons::{AppIcon, IconTone, render_icon},
};

const STACKED_WORKBENCH_WIDTH: f32 = 880.0;

pub(crate) fn render(app: &OneChat, available_width: f32, cx: &mut Context<OneChat>) -> AnyElement {
    if !tts_connected(app) {
        return landing_page(
            app,
            AppIcon::Plug,
            if app.tts.controller.discovery.loading {
                "Connecting to audio.cpp…"
            } else {
                "Connect to audio.cpp"
            },
            "Connect your local speech service to discover models and voices.",
            "Configure Connection",
            false,
            cx,
        );
    }
    if app.tts.controller.discovery.catalog.tts.is_empty() {
        return landing_page(
            app,
            AppIcon::Layers,
            "No speech models found",
            "Refresh the audio.cpp catalog after loading a TTS model.",
            "Refresh Models",
            true,
            cx,
        );
    }
    let stacked = available_width < STACKED_WORKBENCH_WIDTH;
    let workbench = div()
        .size_full()
        .min_w_0()
        .flex()
        .when(stacked, |layout| layout.flex_col())
        .when(!stacked, |layout| layout.flex_row())
        .child(source::render(app, stacked, cx))
        .child(output::render(app, stacked, cx));

    page_canvas(cx)
        .id("tts-workbench-page")
        .when(stacked, |page| page.overflow_y_scroll())
        .child(workbench)
        .into_any_element()
}

fn page_canvas(cx: &App) -> gpui::Div {
    div()
        .size_full()
        .min_w_0()
        .flex()
        .bg(crate::desktop::ui::theme::palette(cx).canvas)
}

fn landing_page(
    app: &OneChat,
    icon: AppIcon,
    title: &'static str,
    detail: &'static str,
    action: &'static str,
    refresh: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let loading = app.tts.controller.discovery.loading;
    page_canvas(cx)
        .items_center()
        .justify_center()
        .p_6()
        .child(
            div()
                .w_full()
                .max_w(px(520.0))
                .flex()
                .flex_col()
                .items_center()
                .text_center()
                .child(
                    div()
                        .size(px(64.0))
                        .rounded(px(20.0))
                        .bg(crate::desktop::ui::theme::palette(cx).accent_soft)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(render_icon(icon, IconTone::Accent, 28.0, cx)),
                )
                .child(
                    div()
                        .pt_5()
                        .text_size(px(28.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(title),
                )
                .child(
                    div()
                        .max_w(px(440.0))
                        .pt_2()
                        .text_size(px(14.0))
                        .line_height(px(21.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                )
                .children(app.tts.controller.discovery.error.as_ref().map(|error| {
                    div()
                        .mt_4()
                        .w_full()
                        .rounded(px(12.0))
                        .bg(cx.theme().danger.opacity(0.1))
                        .px_3()
                        .py_2()
                        .text_size(px(11.0))
                        .line_height(px(16.0))
                        .text_color(cx.theme().danger)
                        .child(error.to_string())
                }))
                .child(
                    Button::new("tts-landing-action")
                        .primary()
                        .mt_5()
                        .h(px(42.0))
                        .px_4()
                        .rounded(px(11.0))
                        .label(if loading { "Connecting…" } else { action })
                        .disabled(loading)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if refresh {
                                this.refresh_tts_discovery(cx);
                            } else {
                                this.set_tts_connection_popover_open(true, cx);
                            }
                        })),
                )
                .child(
                    div()
                        .pt_4()
                        .flex()
                        .items_center()
                        .gap_1p5()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(render_icon(AppIcon::Info, IconTone::Muted, 13.0, cx))
                        .child("Connection details remain in memory only"),
                ),
        )
        .into_any_element()
}
