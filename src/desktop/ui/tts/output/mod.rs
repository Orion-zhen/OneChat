mod segment;
mod summary;

use gpui::{AnyElement, Context, FontWeight, div, prelude::*, px};
use gpui_component::{
    ActiveTheme as _,
    button::{Button, ButtonVariants as _},
};

use super::components::{panel, panel_header, tts_connected};
use crate::desktop::{
    app::OneChat,
    ui::icons::{AppIcon, IconTone, render_icon},
};

pub(super) fn render(app: &OneChat, stacked: bool, cx: &mut Context<OneChat>) -> AnyElement {
    let content = if !tts_connected(app) {
        empty_state(
            AppIcon::Plug,
            "Connect audio.cpp",
            "Test the local service, then refresh its TTS models and voices.",
            Some("Configure Connection"),
            cx,
        )
    } else if app.tts.controller.discovery.catalog.tts.is_empty() {
        empty_state(
            AppIcon::Layers,
            "No TTS models found",
            "Check the audio.cpp server configuration and refresh the model catalog.",
            Some("Refresh Models"),
            cx,
        )
    } else if let Some(run) = &app.tts.controller.run {
        summary::render(app, run, cx)
    } else {
        let source_empty = app.tts.controls.source.read(cx).value().trim().is_empty();
        empty_state(
            AppIcon::AudioLines,
            if source_empty {
                "Your audio will appear here"
            } else {
                "Ready to generate"
            },
            if source_empty {
                "Add text in Source. OneChat will plan safe segments before synthesis."
            } else {
                "Generate once to see segment progress, validation, and combined audio."
            },
            None,
            cx,
        )
    };

    panel(stacked, false, cx)
        .child(panel_header(
            "Output",
            "Listen, inspect, and export",
            AppIcon::AudioLines,
            cx,
        ))
        .child(
            div()
                .id("tts-output-scroll")
                .min_h(px(if stacked { 340.0 } else { 0.0 }))
                .min_w_0()
                .flex_1()
                .overflow_y_scroll()
                .track_scroll(&app.tts.output_scroll)
                .child(content),
        )
        .into_any_element()
}

fn empty_state(
    icon: AppIcon,
    title: &str,
    detail: &str,
    action: Option<&'static str>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    div()
        .min_h(px(340.0))
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_3()
        .px_6()
        .pb_6()
        .text_center()
        .child(
            div()
                .size(px(56.0))
                .rounded_full()
                .bg(crate::desktop::ui::theme::palette(cx).accent_soft)
                .flex()
                .items_center()
                .justify_center()
                .child(render_icon(icon, IconTone::Accent, 24.0, cx)),
        )
        .child(
            div()
                .pt_2()
                .text_size(px(24.0))
                .font_weight(FontWeight::SEMIBOLD)
                .child(title.to_string()),
        )
        .child(
            div()
                .max_w(px(440.0))
                .text_size(px(13.0))
                .line_height(px(20.0))
                .text_color(cx.theme().muted_foreground)
                .child(detail.to_string()),
        )
        .children(action.map(|label| {
            Button::new("tts-empty-state-action")
                .primary()
                .mt_2()
                .label(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if label == "Refresh Models" {
                        this.refresh_tts_discovery(cx);
                    } else {
                        this.set_tts_connection_popover_open(true, cx);
                    }
                }))
        }))
        .into_any_element()
}
