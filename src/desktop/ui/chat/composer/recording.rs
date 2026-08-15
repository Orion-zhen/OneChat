use super::*;
use crate::desktop::audio_recording::{
    RECORDING_WAVEFORM_SAMPLES, RecordingSnapshot, RecordingStatus,
};

pub(super) fn render_recording_status(app: &OneChat, cx: &App) -> AnyElement {
    let snapshot = &app.chat.audio_recording;
    let (label, detail) = match snapshot.status {
        RecordingStatus::RequestingPermission => ("Microphone access", "Waiting for permission…"),
        RecordingStatus::Recording => ("Recording", "Enter to finish · Esc to cancel"),
        RecordingStatus::Finalizing => ("Voice message", "Preparing draft…"),
        RecordingStatus::Idle | RecordingStatus::Completed | RecordingStatus::Failed => {
            ("Voice message", "Ready")
        }
    };
    let recording = snapshot.status == RecordingStatus::Recording;

    div()
        .w_full()
        .min_w_0()
        .px_2()
        .flex()
        .items_center()
        .gap_3()
        .child(render_recording_indicator(recording, cx))
        .child(
            div()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.0))
                .child(
                    div()
                        .text_size(px(12.0))
                        .line_height(px(14.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(px(10.0))
                        .line_height(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(detail),
                ),
        )
        .children(recording.then(|| render_waveform(snapshot, cx)))
        .child(
            div()
                .min_w(px(38.0))
                .flex_none()
                .text_right()
                .text_size(px(12.0))
                .line_height(px(14.0))
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(if recording {
                    cx.theme().danger
                } else {
                    cx.theme().muted_foreground
                })
                .child(format_recording_elapsed(snapshot.elapsed_ms)),
        )
        .into_any_element()
}

fn render_recording_indicator(recording: bool, cx: &App) -> AnyElement {
    let color = if recording {
        cx.theme().danger
    } else {
        cx.theme().muted_foreground
    };
    let indicator = div()
        .size(px(14.0))
        .flex_none()
        .grid()
        .grid_cols(1)
        .grid_rows(1)
        .child(
            div()
                .col_start(1)
                .row_start(1)
                .size_full()
                .rounded_full()
                .bg(color)
                .map(|halo| {
                    if recording {
                        halo.with_animation(
                            "recording-indicator-halo",
                            Animation::new(Duration::from_millis(1_600)).repeat(),
                            |halo, progress| {
                                let pulse = (progress * std::f32::consts::TAU).sin();
                                halo.opacity(0.14 + (pulse + 1.0) * 0.07)
                            },
                        )
                        .into_any_element()
                    } else {
                        halo.opacity(0.14).into_any_element()
                    }
                }),
        )
        .child(
            div()
                .col_start(1)
                .row_start(1)
                .mt(px(4.0))
                .ml(px(4.0))
                .size(px(6.0))
                .rounded_full()
                .bg(color),
        );
    indicator.into_any_element()
}

fn render_waveform(snapshot: &RecordingSnapshot, cx: &App) -> AnyElement {
    let history = snapshot.level_history;
    let color = cx.theme().danger;

    div()
        .min_w(px(120.0))
        .h(px(26.0))
        .flex_1()
        .overflow_hidden()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(1.5))
        .with_animation(
            "recording-waveform",
            Animation::new(Duration::from_millis(1_400)).repeat(),
            move |waveform, progress| {
                let phase = progress * std::f32::consts::TAU;
                waveform.children(history.iter().enumerate().map(|(index, level)| {
                    let freshness = index as f32 / (RECORDING_WAVEFORM_SAMPLES - 1) as f32;
                    let live_edge = freshness.powi(12);
                    let flutter = 1.0 + live_edge * 0.08 * (phase + index as f32 * 0.7).sin();
                    let activity = (f32::from(*level) / 1_000.0).sqrt().clamp(0.0, 1.0);
                    let height = waveform_bar_height(*level) * flutter;
                    let opacity = (0.28 + freshness * 0.5 + activity * 0.22).clamp(0.0, 1.0);

                    div()
                        .min_w(px(1.5))
                        .max_w(px(3.0))
                        .flex_1()
                        .h(px(height))
                        .rounded_full()
                        .bg(color.opacity(opacity))
                }))
            },
        )
        .into_any_element()
}

fn waveform_bar_height(level_milli: u16) -> f32 {
    let level = (f32::from(level_milli) / 1_000.0).sqrt().clamp(0.0, 1.0);
    3.0 + level * 19.0
}

fn format_recording_elapsed(elapsed_ms: u64) -> String {
    let seconds = elapsed_ms / 1_000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::{format_recording_elapsed, waveform_bar_height};

    #[test]
    fn recording_elapsed_uses_voice_memo_style_time() {
        assert_eq!(format_recording_elapsed(0), "0:00");
        assert_eq!(format_recording_elapsed(61_999), "1:01");
        assert_eq!(format_recording_elapsed(300_000), "5:00");
    }

    #[test]
    fn waveform_keeps_silence_visible_and_caps_loud_audio() {
        assert_eq!(waveform_bar_height(0), 3.0);
        assert_eq!(waveform_bar_height(1_000), 22.0);
        assert_eq!(waveform_bar_height(u16::MAX), 22.0);
    }
}
