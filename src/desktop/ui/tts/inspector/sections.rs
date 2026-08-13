use gpui::{AnyElement, App, Context, FontWeight, div, prelude::*, px};
use gpui_component::{ActiveTheme as _, Disableable as _, Sizable as _, switch::Switch};

use super::fields::{percent, settings_group, slider_setting, value_setting};
use crate::{
    desktop::{
        app::OneChat,
        ui::tts::components::{disclosure_button, separator},
    },
    speech::SpeechConfig,
};

pub(super) fn generation(config: &SpeechConfig, cx: &App) -> AnyElement {
    settings_group(
        "Generation",
        "Controls request limits and retries for each generated segment.",
        div()
            .flex()
            .flex_col()
            .child(value_setting(
                "Request time limit",
                "Stop a generation request after this wait.",
                &format!("{} s", config.request_timeout.as_secs()),
                cx,
            ))
            .child(separator(cx))
            .child(value_setting(
                "Service error retries",
                "Retry requests that fail because of a service or connection error.",
                &config.transport_retries.to_string(),
                cx,
            ))
            .child(separator(cx))
            .child(value_setting(
                "Quality failure retries",
                "Regenerate audio that fails an audio or transcript check.",
                &config.quality_retries.to_string(),
                cx,
            ))
            .child(separator(cx))
            .child(value_setting(
                "Voice generation controls",
                "Speed, randomness, and Top P use the selected model's defaults.",
                "Model defaults",
                cx,
            )),
        cx,
    )
}

pub(super) fn segmentation(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let config = app.tts.controller.config.segmentation;
    settings_group(
        "Text Segmentation",
        "Splits source text at natural sentence boundaries before generation.",
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(slider_setting(
                "Minimum segment length",
                "Avoid segments shorter than this when possible.",
                &app.tts.controls.tuning.min_chars,
                format!("{} chars", config.min_chars),
                cx,
            ))
            .child(slider_setting(
                "Preferred segment length",
                "Aim for this length while preserving sentence boundaries.",
                &app.tts.controls.tuning.target_chars,
                format!("{} chars", config.target_chars),
                cx,
            ))
            .child(slider_setting(
                "Maximum segment length",
                "Always split a segment when it exceeds this hard limit.",
                &app.tts.controls.tuning.max_chars,
                format!("{} chars", config.max_chars),
                cx,
            ))
            .child(slider_setting(
                "Length flexibility",
                "Higher values allow lengths to vary farther from the preferred length.",
                &app.tts.controls.tuning.spread,
                format!("{} chars", config.spread),
                cx,
            )),
        cx,
    )
}

pub(super) fn audio_validation(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let config = app.tts.controller.config.audio_validation;
    let expanded = app.tts.view.audio_thresholds_expanded;
    let details = expanded.then(|| {
        div()
            .mt_2()
            .rounded(px(10.0))
            .bg(crate::desktop::ui::theme::palette(cx).secondary)
            .p_3()
            .flex()
            .flex_col()
            .gap_4()
            .child(slider_setting(
                "Minimum clip duration",
                "Reject generated clips shorter than this.",
                &app.tts.controls.tuning.min_duration,
                format!("{:.2} s", config.min_duration_sec),
                cx,
            ))
            .child(slider_setting(
                "Silence threshold (RMS)",
                "Audio at or below this normalized loudness is treated as silence.",
                &app.tts.controls.tuning.min_rms,
                format!("{:.4}", config.min_rms),
                cx,
            ))
            .child(slider_setting(
                "Minimum audible content",
                "Reject clips with audible audio below this share of their duration.",
                &app.tts.controls.tuning.min_active_ratio,
                percent(config.min_active_ratio),
                cx,
            ))
            .child(
                div()
                    .text_size(px(10.0))
                    .line_height(px(15.0))
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "A clip is flagged as noise only when all three checks below are crossed.",
                    ),
            )
            .child(slider_setting(
                "Noise spectrum threshold",
                "A flatter spectrum above this level counts toward noise detection.",
                &app.tts.controls.tuning.noise_flatness,
                percent(config.noise_flatness),
                cx,
            ))
            .child(slider_setting(
                "Noise sign-change threshold",
                "A waveform sign-change rate above this level counts toward noise detection.",
                &app.tts.controls.tuning.noise_zcr,
                percent(config.noise_zcr),
                cx,
            ))
            .child(slider_setting(
                "Noise activity threshold",
                "Only flag noise when audible audio exceeds this share.",
                &app.tts.controls.tuning.noise_active_ratio,
                percent(config.noise_active_ratio),
                cx,
            ))
            .child(slider_setting(
                "Edge silence trim trigger",
                "Trim leading or trailing silence only when it exceeds this duration.",
                &app.tts.controls.tuning.trim_max_silence,
                format!("{:.2} s", config.trim_max_edge_silence_sec),
                cx,
            ))
            .child(slider_setting(
                "Edge silence to keep",
                "Keep this much silence at each edge after trimming.",
                &app.tts.controls.tuning.trim_keep_silence,
                format!("{:.2} s", config.trim_keep_edge_silence_sec),
                cx,
            ))
            .child(slider_setting(
                "Pause between segments",
                "Ensure at least this much silence when joining segments.",
                &app.tts.controls.tuning.merge_silence,
                format!("{:.2} s", app.tts.controller.config.merge.min_silence_sec),
                cx,
            ))
    });
    settings_group(
        "Audio Validation",
        "Rejects silent, too-short, or noise-like clips before they reach the output.",
        div()
            .child(
                disclosure_button(
                    "tts-audio-thresholds",
                    "Audio checks and spacing",
                    expanded,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_tts_audio_thresholds(cx))),
            )
            .children(details),
        cx,
    )
}

pub(super) fn transcript_validation(app: &OneChat, cx: &mut Context<OneChat>) -> AnyElement {
    let validation = &app.tts.controller.config.transcript_validation;
    let expanded = app.tts.view.transcript_details_expanded;
    let model = validation.model.as_deref().unwrap_or("No ASR model found");
    let details = expanded.then(|| {
        div()
            .mt_2()
            .flex()
            .flex_col()
            .gap_4()
            .child(value_setting(
                "Speech recognition model",
                "Transcribes generated audio so it can be compared with the source text.",
                model,
                cx,
            ))
            .child(slider_setting(
                "Required transcript match",
                "Reject audio when the recognized text matches less than this percentage.",
                &app.tts.controls.tuning.similarity,
                percent(validation.similarity_threshold),
                cx,
            ))
    });
    settings_group(
        "Transcript Validation",
        "Optionally checks whether generated speech says the intended text.",
        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .min_h(px(48.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .min_w_0()
                            .child(
                                div()
                                    .text_size(px(12.0))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("Check spoken text with ASR"),
                            )
                            .child(
                                div()
                                    .pt_0p5()
                                    .text_size(px(10.0))
                                    .line_height(px(15.0))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        "Transcribe each clip and compare it with the source text.",
                                    ),
                            ),
                    )
                    .child(
                        Switch::new("tts-transcript-validation")
                            .small()
                            .checked(validation.enabled)
                            .color(cx.theme().primary)
                            .disabled(
                                app.tts.controller.discovery.catalog.asr.is_empty()
                                    && !validation.enabled,
                            )
                            .on_click(cx.listener(|this, _: &bool, _, cx| {
                                this.toggle_tts_transcript_validation(cx)
                            })),
                    ),
            )
            .child(
                disclosure_button(
                    "tts-transcript-details",
                    "ASR model and match threshold",
                    expanded,
                    cx,
                )
                .on_click(cx.listener(|this, _, _, cx| this.toggle_tts_transcript_details(cx))),
            )
            .children(details),
        cx,
    )
}
