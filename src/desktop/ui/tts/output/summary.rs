use gpui::{AnyElement, App, Context, FontWeight, div, prelude::*, px, relative};
use gpui_component::{
    ActiveTheme as _, Disableable as _,
    button::{Button, ButtonVariants as _},
};

use super::segment;
use crate::{
    desktop::{
        app::{OneChat, tts_combined_source_id},
        ui::{
            icons::{AppIcon, IconTone, render_icon},
            tts::{components::tone_color, player},
        },
    },
    speech::{RunStatus, SegmentStatus, SpeechRun},
};

const SEGMENT_LIST_RADIUS: f32 = 15.0;

pub(super) fn render(app: &OneChat, run: &SpeechRun, cx: &mut Context<OneChat>) -> AnyElement {
    let total = run.segments.len();
    let ready = run
        .segments
        .iter()
        .filter(|segment| segment.status == SegmentStatus::Ready)
        .count();
    let failed = run
        .segments
        .iter()
        .filter(|segment| segment.status == SegmentStatus::Failed)
        .count();
    let settled = run
        .segments
        .iter()
        .filter(|segment| {
            matches!(
                segment.status,
                SegmentStatus::Ready | SegmentStatus::Failed | SegmentStatus::Cancelled
            )
        })
        .count();
    let progress = if total == 0 {
        0.0
    } else {
        settled as f32 / total as f32
    };
    let (stage, stage_icon, stage_tone) = run_stage(run.status);
    let operation_active = app.tts.controller.operation.active().is_some();

    div()
        .flex()
        .flex_col()
        .gap_4()
        .child(
            div()
                .rounded(px(16.0))
                .border_1()
                .border_color(cx.theme().border)
                .bg(crate::desktop::ui::theme::palette(cx).raised)
                .p_4()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap_4()
                        .child(
                            div()
                                .min_w_0()
                                .flex()
                                .items_center()
                                .gap_3()
                                .child(
                                    div()
                                        .size(px(38.0))
                                        .flex_none()
                                        .rounded(px(12.0))
                                        .bg(tone_color(stage_tone, cx).opacity(0.14))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .child(render_icon(stage_icon, stage_tone, 18.0, cx)),
                                )
                                .child(
                                    div()
                                        .min_w_0()
                                        .child(
                                            div()
                                                .text_size(px(15.0))
                                                .font_weight(FontWeight::SEMIBOLD)
                                                .child(stage),
                                        )
                                        .child(
                                            div()
                                                .pt_0p5()
                                                .text_size(px(11.0))
                                                .text_color(cx.theme().muted_foreground)
                                                .child(format!(
                                                    "{settled} of {total} segments processed"
                                                )),
                                        ),
                                ),
                        )
                        .child(
                            div()
                                .flex_none()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(summary_chip(
                                    &format!("{ready} ready"),
                                    IconTone::Success,
                                    cx,
                                ))
                                .children((failed > 0).then(|| {
                                    summary_chip(&format!("{failed} failed"), IconTone::Danger, cx)
                                })),
                        ),
                )
                .child(
                    div()
                        .h(px(5.0))
                        .w_full()
                        .overflow_hidden()
                        .rounded_full()
                        .bg(cx.theme().border)
                        .child(div().h_full().w(relative(progress)).rounded_full().bg(
                            if failed > 0 {
                                cx.theme().warning
                            } else {
                                cx.theme().primary
                            },
                        )),
                )
                .children(run.error.as_ref().map(|error| {
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
                .children(run.combined_clip.as_ref().map(|clip| {
                    player::render(
                        app,
                        tts_combined_source_id(app.tts.controller.audio_revision),
                        clip,
                        if run.status == RunStatus::Completed {
                            "Combined Speech"
                        } else {
                            "Partial Combined Speech"
                        },
                        false,
                        cx,
                    )
                }))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_end()
                        .gap_2()
                        .children((failed > 0).then(|| {
                            Button::new("retry-failed-tts-segments")
                                .secondary()
                                .label(format!("Retry {failed} Failed"))
                                .disabled(operation_active)
                                .on_click(
                                    cx.listener(|this, _, _, cx| {
                                        this.retry_failed_tts_segments(cx)
                                    }),
                                )
                        }))
                        .children(
                            run.combined_clip
                                .as_ref()
                                .map(|_| save_audio_buttons(run.status, cx)),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .items_end()
                .justify_between()
                .px_1()
                .child(
                    div()
                        .child(
                            div()
                                .text_size(px(14.0))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Segments"),
                        )
                        .child(
                            div()
                                .pt_0p5()
                                .text_size(px(11.0))
                                .text_color(cx.theme().muted_foreground)
                                .child("Open a segment to inspect or regenerate it"),
                        ),
                )
                .child(
                    div()
                        .text_size(px(11.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("{total} total")),
                ),
        )
        .child(
            div()
                .overflow_hidden()
                .rounded(px(SEGMENT_LIST_RADIUS))
                .border_1()
                .border_color(cx.theme().border)
                .bg(crate::desktop::ui::theme::palette(cx).raised)
                .flex()
                .flex_col()
                .children(run.segments.iter().enumerate().map(|(position, result)| {
                    segment::render(
                        app,
                        result,
                        position == 0,
                        position + 1 == total,
                        operation_active,
                        cx,
                    )
                })),
        )
        .into_any_element()
}

fn summary_chip(label: &str, tone: IconTone, cx: &App) -> AnyElement {
    let color = tone_color(tone, cx);
    div()
        .rounded_full()
        .bg(color.opacity(0.12))
        .px_2()
        .py_1()
        .text_size(px(10.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(color)
        .child(label.to_string())
        .into_any_element()
}

fn run_stage(status: RunStatus) -> (&'static str, AppIcon, IconTone) {
    match status {
        RunStatus::Planning => ("Planning segments", AppIcon::Search, IconTone::Accent),
        RunStatus::Running => ("Generating speech", AppIcon::Sparkles, IconTone::Accent),
        RunStatus::Completed => ("Speech ready", AppIcon::ContextSelected, IconTone::Success),
        RunStatus::Partial => ("Partial audio ready", AppIcon::Info, IconTone::Warning),
        RunStatus::Failed => ("Generation failed", AppIcon::Info, IconTone::Danger),
        RunStatus::Cancelled => ("Generation stopped", AppIcon::Stop, IconTone::Muted),
    }
}

fn save_audio_labels(status: RunStatus) -> (&'static str, &'static str) {
    if status == RunStatus::Completed {
        ("Save WAV", "Save MP3")
    } else {
        ("Save Partial WAV", "Save Partial MP3")
    }
}

fn save_audio_buttons(status: RunStatus, cx: &mut Context<OneChat>) -> AnyElement {
    let (wav_label, mp3_label) = save_audio_labels(status);
    div()
        .flex()
        .items_center()
        .gap_2()
        .child(
            Button::new("save-tts-wav")
                .secondary()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(render_icon(AppIcon::AudioLines, IconTone::Muted, 15.0, cx))
                        .child(wav_label),
                )
                .on_click(cx.listener(|this, _, window, cx| this.export_tts_wav(window, cx))),
        )
        .child(
            Button::new("save-tts-mp3")
                .secondary()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(render_icon(AppIcon::FileDown, IconTone::Muted, 15.0, cx))
                        .child(mp3_label),
                )
                .on_click(cx.listener(|this, _, window, cx| this.export_tts_mp3(window, cx))),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_actions_identify_partial_audio() {
        assert_eq!(
            save_audio_labels(RunStatus::Completed),
            ("Save WAV", "Save MP3")
        );
        assert_eq!(
            save_audio_labels(RunStatus::Partial),
            ("Save Partial WAV", "Save Partial MP3")
        );
    }

    #[test]
    fn non_completed_exportable_runs_follow_partial_export_policy() {
        assert_eq!(
            save_audio_labels(RunStatus::Cancelled),
            ("Save Partial WAV", "Save Partial MP3")
        );
    }
}
