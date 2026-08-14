use gpui::{AnyElement, Context, SharedString, div, prelude::*, px};
use gpui_component::ActiveTheme as _;

use crate::{
    desktop::{
        app::OneChat,
        audio_playback::PlaybackStatus,
        ui::{
            playback::{self, format_audio_duration, format_audio_position},
            theme::palette,
        },
    },
    speech::AudioClip,
};

pub(super) fn render(
    app: &OneChat,
    source_id: String,
    clip: &AudioClip,
    title: &'static str,
    compact: bool,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let current = app.playback.snapshot.source_id.as_deref() == Some(&source_id);
    let status = if current {
        app.playback.snapshot.status
    } else {
        PlaybackStatus::Idle
    };
    let duration_ms = audio_duration_ms(clip);
    let position_ms = if current {
        app.playback.snapshot.position_ms.min(duration_ms)
    } else {
        0
    };
    let progress = if duration_ms == 0 {
        0.0
    } else {
        position_ms as f32 / duration_ms as f32
    };
    let detail = match status {
        PlaybackStatus::Loading if compact => "Loading…".into(),
        PlaybackStatus::Loading => format!("Loading… · {}", format_audio_duration(duration_ms)),
        PlaybackStatus::Playing | PlaybackStatus::Paused => format!(
            "{} / {}",
            format_audio_position(position_ms),
            format_audio_duration(duration_ms)
        ),
        PlaybackStatus::Idle | PlaybackStatus::Failed if compact => {
            format_audio_duration(duration_ms)
        }
        PlaybackStatus::Idle | PlaybackStatus::Failed => {
            format!("Audio · {}", format_audio_duration(duration_ms))
        }
    };
    let id = source_id.clone();
    let clip = clip.clone();
    let play_button =
        playback::play_button(SharedString::from(format!("play-{source_id}")), status, cx)
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_tts_audio_playback(id.clone(), clip.clone(), cx)
            }));

    div()
        .when(compact, |player| player.py_1())
        .when(!compact, |player| {
            player
                .rounded(px(14.0))
                .border_1()
                .border_color(cx.theme().border)
                .bg(palette(cx).panel)
                .p_2()
        })
        .flex()
        .items_center()
        .child(playback::render(
            play_button,
            title,
            detail,
            current,
            progress,
            &app.playback.seek_slider,
            cx,
        ))
        .into_any_element()
}

fn audio_duration_ms(clip: &AudioClip) -> u64 {
    (f64::from(clip.duration_sec()) * 1_000.0).round() as u64
}
