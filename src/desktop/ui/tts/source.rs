use gpui::{AnyElement, Context, div, prelude::*, px};
use gpui_component::{
    ActiveTheme as _, Disableable as _,
    button::{Button, ButtonVariants as _},
    input::Input,
};

use super::components::{panel, panel_header, tts_connected};
use crate::desktop::{
    app::{OneChat, TtsOperationKind},
    ui::icons::{AppIcon, IconTone, render_icon},
};

pub(super) fn render(app: &OneChat, stacked: bool, cx: &mut Context<OneChat>) -> AnyElement {
    let source = app.tts.controls.source.read(cx).value();
    let char_count = source.chars().count();
    let connected = tts_connected(app);
    let has_model = !app.tts.controller.config.generation.model.is_empty();
    let source_empty = source.trim().is_empty();
    let estimated_segments = char_count
        .div_ceil(app.tts.controller.config.segmentation.target_chars.max(1))
        .max(usize::from(!source_empty));
    let active = app.tts.controller.operation.active();
    let generating =
        active.is_some_and(|operation| !matches!(operation.kind, TtsOperationKind::Discovery));
    let stale = app.tts.controller.run_is_stale();
    let action_label = if generating {
        "Stop Generation"
    } else if app.tts.controller.run.is_none() {
        "Generate Speech"
    } else if stale {
        "Generate New Version"
    } else {
        "Generate Again"
    };
    let action_disabled =
        !generating && (!connected || !has_model || source_empty || active.is_some());

    panel(stacked, true, cx)
        .child(panel_header(
            "Source",
            "Text stays in memory and is never saved",
            AppIcon::FileText,
            cx,
        ))
        .child(
            div()
                .min_h(px(if stacked { 280.0 } else { 0.0 }))
                .min_w_0()
                .flex_1()
                .rounded(px(14.0))
                .border_1()
                .border_color(cx.theme().border)
                .bg(crate::desktop::ui::theme::palette(cx).panel)
                .px_4()
                .py_3()
                .child(
                    Input::new(&app.tts.controls.source)
                        .appearance(false)
                        .w_full()
                        .h_full()
                        .px_0()
                        .py_0()
                        .text_size(px(15.0))
                        .line_height(px(22.0))
                        .aria_label("Text to turn into speech"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(
                    div()
                        .min_h(px(24.0))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_3()
                        .text_size(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .child(render_icon(AppIcon::FileText, IconTone::Muted, 13.0, cx))
                                .child(format!("{char_count} characters")),
                        )
                        .child(
                            div()
                                .rounded_full()
                                .bg(crate::desktop::ui::theme::palette(cx).secondary)
                                .px_2()
                                .py_1()
                                .child(if source_empty {
                                    "Auto-segment".to_string()
                                } else {
                                    format!("About {estimated_segments} segments")
                                }),
                        ),
                )
                .children((stale && generating).then(|| {
                    div()
                        .rounded(px(11.0))
                        .bg(cx.theme().warning.opacity(0.12))
                        .px_3()
                        .py_2()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_size(px(11.0))
                        .text_color(cx.theme().warning)
                        .child(render_icon(AppIcon::Info, IconTone::Warning, 13.0, cx))
                        .child("Edits apply to the next generation")
                }))
                .children(app.tts.controller.error.as_ref().map(|error| {
                    div()
                        .rounded(px(11.0))
                        .bg(cx.theme().danger.opacity(0.1))
                        .px_3()
                        .py_2()
                        .text_size(px(11.0))
                        .line_height(px(16.0))
                        .text_color(cx.theme().danger)
                        .child(error.to_string())
                }))
                .child(
                    Button::new("tts-primary-action")
                        .when(generating, |button| button.danger())
                        .when(!generating, |button| button.primary())
                        .w_full()
                        .h(px(44.0))
                        .rounded(px(12.0))
                        .disabled(action_disabled)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .justify_center()
                                .gap_2()
                                .child(render_icon(
                                    if generating {
                                        AppIcon::Stop
                                    } else {
                                        AppIcon::AudioLines
                                    },
                                    IconTone::OnAccent,
                                    16.0,
                                    cx,
                                ))
                                .child(action_label),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if generating {
                                this.stop_tts_operation(cx);
                            } else {
                                this.start_tts_run(cx);
                            }
                        })),
                ),
        )
        .into_any_element()
}
