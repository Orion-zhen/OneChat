use gpui::{AnyElement, Context, FontWeight, div, prelude::*, px};
use gpui_component::ActiveTheme as _;

use super::components::{header_controls, language_select_slot, panel, panel_header};
use crate::desktop::{
    app::OneChat,
    ui::{
        chat::render_readonly_assistant_content,
        controls::select_control,
        copy_button::CopyButton,
        icons::{AppIcon, IconTone, render_icon},
        layout::LayoutClass,
        theme,
        typography::MessageTypography,
    },
};

pub(super) fn render(
    app: &OneChat,
    layout: LayoutClass,
    scale_factor: f32,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let stacked = !layout.is_wide();
    let narrow = layout.is_narrow();
    let response = app.translation.response.as_ref();
    let request = app.translation.request.as_ref();
    let output = response
        .map(|response| response.content.clone())
        .unwrap_or_default();
    let stats = request.map(format_stats).unwrap_or_default();
    let request_error = request.and_then(|request| request.error.as_ref());
    let body = match response {
        Some(response) => render_readonly_assistant_content(
            app,
            response,
            request,
            scale_factor,
            MessageTypography::new(app.settings().message_font_size()),
            cx,
        ),
        None => empty_result(stacked, cx),
    };

    panel(stacked, cx)
        .child(
            panel_header("Translation", AppIcon::Sparkles, narrow, cx).child(
                header_controls(narrow)
                    .child(
                        language_select_slot(narrow).child(
                            select_control(&app.translation.controls.target_language)
                                .w_full()
                                .disabled(app.translation.is_generating()),
                        ),
                    )
                    .children((!narrow).then(|| div().min_w_0().flex_1()))
                    .children(
                        (!output.is_empty())
                            .then(|| CopyButton::new("copy-translation-output", output)),
                    ),
            ),
        )
        .child(
            div()
                .id("translation-result-scroll")
                .min_h(px(if stacked { 240.0 } else { 0.0 }))
                .min_w_0()
                .flex_1()
                .overflow_y_scroll()
                .track_scroll(&app.translation.result_scroll)
                .p_4()
                .child(body)
                .children(request_error.map(|error| {
                    div()
                        .mt_3()
                        .rounded(px(10.0))
                        .bg(cx.theme().danger.opacity(0.1))
                        .px_3()
                        .py_2()
                        .text_size(px(12.0))
                        .line_height(px(18.0))
                        .text_color(cx.theme().danger)
                        .child(error.message.clone())
                })),
        )
        .children((!stats.is_empty()).then(|| {
            div()
                .flex_none()
                .min_h(px(30.0))
                .px_4()
                .pb_3()
                .flex()
                .items_center()
                .justify_end()
                .text_size(px(11.0))
                .text_color(cx.theme().muted_foreground)
                .child(stats)
        }))
        .into_any_element()
}

fn empty_result(stacked: bool, cx: &gpui::App) -> AnyElement {
    div()
        .size_full()
        .min_h(px(if stacked { 240.0 } else { 0.0 }))
        .flex()
        .items_center()
        .justify_center()
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_3()
                .text_center()
                .child(
                    div()
                        .size(px(44.0))
                        .rounded(px(14.0))
                        .bg(theme::palette(cx).accent_soft)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(render_icon(AppIcon::Languages, IconTone::Accent, 21.0, cx)),
                )
                .child(
                    div()
                        .text_size(px(14.0))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(cx.theme().muted_foreground)
                        .child("Translation will appear here"),
                ),
        )
        .into_any_element()
}

fn format_stats(request: &crate::domain::RequestInfo) -> String {
    let input = request
        .usage
        .input_tokens
        .map_or_else(|| "—".into(), |value| value.to_string());
    let output = request
        .usage
        .output_tokens
        .map_or_else(|| "—".into(), |value| value.to_string());
    let duration = request
        .duration_ms
        .map(|value| format!(" · {:.1}s", value as f32 / 1_000.0))
        .unwrap_or_default();
    format!("{input} in / {output} out{duration}")
}
