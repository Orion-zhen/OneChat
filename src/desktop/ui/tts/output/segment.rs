use std::time::Duration;

use gpui::{
    Animation, AnimationExt as _, AnyElement, App, Context, FontWeight, SharedString, div,
    ease_out_quint, prelude::*, px,
};
use gpui_component::{
    ActiveTheme as _, Disableable as _,
    button::{Button, ButtonVariants as _},
};

use crate::{
    desktop::{
        app::{OneChat, tts_segment_source_id},
        ui::{
            icons::{AppIcon, IconTone, render_icon},
            tts::{
                components::{tone_color, value_row},
                player,
            },
        },
    },
    speech::{SegmentResult, SegmentStatus},
};

const SEGMENT_ROW_RADIUS: f32 = 14.0;

pub(super) fn render(
    app: &OneChat,
    result: &SegmentResult,
    first: bool,
    last: bool,
    operation_active: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let index = result.segment.index;
    let expanded = app.tts.view.expanded_segments.contains(&index);
    let technical = app.tts.view.technical_segments.contains(&index);
    let (label, icon, tone) = segment_status(result.status);
    let status_id: SharedString = format!(
        "tts-segment-status-{index}-{:?}-{}",
        result.status, result.attempt
    )
    .into();
    let status_color = tone_color(tone, cx);
    let status = div()
        .flex_none()
        .flex()
        .items_center()
        .gap_1p5()
        .rounded_full()
        .bg(status_color.opacity(0.11))
        .px_2()
        .py_1()
        .text_size(px(10.0))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(status_color)
        .child(render_icon(icon, tone, 12.0, cx))
        .child(label)
        .with_animation(
            status_id,
            Animation::new(Duration::from_millis(160)).with_easing(ease_out_quint()),
            move |status, progress| status.opacity(progress),
        );

    let active = matches!(
        result.status,
        SegmentStatus::Generating | SegmentStatus::Validating | SegmentStatus::Retrying
    );
    let row_background = if result.status == SegmentStatus::Failed {
        cx.theme().danger.opacity(0.06)
    } else if active {
        crate::desktop::ui::theme::palette(cx).accent_soft
    } else {
        cx.theme().transparent
    };

    div()
        .when(!first, |row| row.border_t_1())
        .when(first, |row| {
            row.rounded_tl(px(SEGMENT_ROW_RADIUS))
                .rounded_tr(px(SEGMENT_ROW_RADIUS))
        })
        .when(last, |row| {
            row.rounded_bl(px(SEGMENT_ROW_RADIUS))
                .rounded_br(px(SEGMENT_ROW_RADIUS))
        })
        .border_color(cx.theme().border)
        .bg(row_background)
        .child(
            Button::new(SharedString::from(format!("tts-segment-{index}")))
                .ghost()
                .rounded(px(0.0))
                .when(first, |button| {
                    button
                        .rounded_tl(px(SEGMENT_ROW_RADIUS))
                        .rounded_tr(px(SEGMENT_ROW_RADIUS))
                })
                .when(last && !expanded, |button| {
                    button
                        .rounded_bl(px(SEGMENT_ROW_RADIUS))
                        .rounded_br(px(SEGMENT_ROW_RADIUS))
                })
                .w_full()
                .h(px(54.0))
                .px_3()
                .tooltip(if expanded {
                    "Collapse segment"
                } else {
                    "Expand segment"
                })
                .child(
                    div()
                        .w_full()
                        .min_w_0()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .w(px(22.0))
                                .flex_none()
                                .text_right()
                                .text_size(px(10.0))
                                .font_weight(FontWeight::MEDIUM)
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{:02}", index + 1)),
                        )
                        .child(
                            div()
                                .min_w_0()
                                .flex_1()
                                .text_left()
                                .text_size(px(12.0))
                                .line_height(px(18.0))
                                .font_weight(FontWeight::MEDIUM)
                                .truncate()
                                .child(result.segment.text.clone()),
                        )
                        .child(status)
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
                .on_click(cx.listener(move |this, _, _, cx| this.toggle_tts_segment(index, cx))),
        )
        .children(expanded.then(|| details(app, result, technical, operation_active, cx)))
        .into_any_element()
}

fn segment_status(status: SegmentStatus) -> (&'static str, AppIcon, IconTone) {
    match status {
        SegmentStatus::Waiting => ("Waiting", AppIcon::AudioLines, IconTone::Muted),
        SegmentStatus::Generating => ("Generating", AppIcon::Sparkles, IconTone::Accent),
        SegmentStatus::Validating => ("Validating", AppIcon::Search, IconTone::Accent),
        SegmentStatus::Retrying => ("Retrying", AppIcon::Regenerate, IconTone::Warning),
        SegmentStatus::Ready => ("Ready", AppIcon::ContextSelected, IconTone::Success),
        SegmentStatus::Failed => ("Failed", AppIcon::Info, IconTone::Danger),
        SegmentStatus::Cancelled => ("Cancelled", AppIcon::Stop, IconTone::Muted),
    }
}

fn details(
    app: &OneChat,
    result: &SegmentResult,
    technical: bool,
    operation_active: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let index = result.segment.index;
    div()
        .px_3()
        .pt_1()
        .pb_4()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .rounded(px(12.0))
                .bg(crate::desktop::ui::theme::palette(cx).secondary)
                .px_4()
                .py_3()
                .text_size(px(12.0))
                .line_height(px(20.0))
                .child(result.segment.text.clone()),
        )
        .children(result.clip.as_ref().map(|clip| {
            player::render(
                app,
                tts_segment_source_id(app.tts.controller.audio_revision, index),
                clip,
                "Audio",
                true,
                cx,
            )
        }))
        .children(result.error.as_ref().map(|error| {
            div()
                .rounded(px(9.0))
                .bg(cx.theme().danger.opacity(0.1))
                .px_3()
                .py_2()
                .text_size(px(10.0))
                .line_height(px(15.0))
                .text_color(cx.theme().danger)
                .child(error.to_string())
        }))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    Button::new(SharedString::from(format!(
                        "segment-technical-details-{index}"
                    )))
                    .ghost()
                    .compact()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1p5()
                            .text_size(px(10.0))
                            .text_color(cx.theme().muted_foreground)
                            .child("Technical details")
                            .child(render_icon(
                                if technical {
                                    AppIcon::ChevronUp
                                } else {
                                    AppIcon::ChevronDown
                                },
                                IconTone::Muted,
                                12.0,
                                cx,
                            )),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.toggle_tts_technical_details(index, cx)
                    })),
                )
                .child(
                    Button::new(SharedString::from(format!("regenerate-segment-{index}")))
                        .ghost()
                        .compact()
                        .disabled(operation_active)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_1p5()
                                .child(render_icon(AppIcon::Regenerate, IconTone::Muted, 13.0, cx))
                                .child("Regenerate"),
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.regenerate_tts_segment(index, cx)
                        })),
                ),
        )
        .children(technical.then(|| technical_details(result, cx)))
        .into_any_element()
}

fn technical_details(result: &SegmentResult, cx: &App) -> AnyElement {
    let mut details = div()
        .rounded(px(10.0))
        .bg(crate::desktop::ui::theme::palette(cx).secondary)
        .px_3()
        .child(value_row("Attempt", &result.attempt.to_string(), cx))
        .child(value_row(
            "Seed",
            &result
                .seed
                .map_or_else(|| "Random".into(), |seed| seed.to_string()),
            cx,
        ));
    if let Some(audio) = &result.audio_validation {
        for (label, value) in [
            ("Duration", format!("{:.2} s", audio.duration_sec)),
            ("RMS", format!("{:.4}", audio.rms)),
            ("Peak", format!("{:.3}", audio.peak)),
            ("Active", format!("{:.0}%", audio.active_ratio * 100.0)),
            ("Flatness", format!("{:.3}", audio.spectral_flatness)),
            ("ZCR", format!("{:.3}", audio.zero_crossing_rate)),
        ] {
            details = details.child(value_row(label, &value, cx));
        }
    }
    if let Some(transcript) = &result.transcript_validation {
        details = details
            .child(technical_text("Expected", &transcript.expected, cx))
            .child(technical_text("Transcript", &transcript.transcript, cx))
            .child(value_row(
                "Similarity",
                &format!("{:.1}%", transcript.similarity * 100.0),
                cx,
            ));
    }
    details.into_any_element()
}

fn technical_text(label: &'static str, value: &str, cx: &App) -> AnyElement {
    div()
        .py_2()
        .flex()
        .flex_col()
        .gap_1()
        .text_size(px(11.0))
        .child(div().text_color(cx.theme().muted_foreground).child(label))
        .child(div().line_height(px(16.0)).child(value.to_string()))
        .into_any_element()
}
