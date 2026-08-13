use gpui::{AnyElement, App, Entity, FontWeight, SharedString, div, prelude::*, px, relative};
use gpui_component::{
    ActiveTheme as _, Disableable as _,
    button::{Button, ButtonVariants as _},
    slider::{Slider, SliderState},
};

use crate::desktop::{
    audio_playback::PlaybackStatus,
    ui::icons::{AppIcon, IconTone, render_icon},
};

pub(crate) fn play_button(id: SharedString, status: PlaybackStatus, cx: &App) -> Button {
    let playing = status == PlaybackStatus::Playing;
    let loading = status == PlaybackStatus::Loading;
    Button::new(id)
        .ghost()
        .rounded_full()
        .tooltip(if loading {
            "Preparing audio"
        } else if playing {
            "Pause audio"
        } else {
            "Play audio"
        })
        .disabled(loading)
        .size(px(40.0))
        .p_0()
        .bg(cx.theme().accent)
        .child(render_icon(
            if playing {
                AppIcon::Pause
            } else {
                AppIcon::Play
            },
            if loading {
                IconTone::Muted
            } else {
                IconTone::Accent
            },
            18.0,
            cx,
        ))
}

pub(crate) fn render(
    button: Button,
    title: impl Into<SharedString>,
    detail: impl Into<SharedString>,
    current: bool,
    progress: f32,
    seek_slider: &Entity<SliderState>,
    cx: &App,
) -> AnyElement {
    div()
        .min_w_0()
        .w_full()
        .flex()
        .items_center()
        .gap_3()
        .child(button)
        .child(
            div()
                .min_w_0()
                .flex_1()
                .flex()
                .flex_col()
                .gap(px(5.0))
                .child(
                    div()
                        .min_w_0()
                        .text_size(px(12.0))
                        .line_height(px(15.0))
                        .font_weight(FontWeight::SEMIBOLD)
                        .truncate()
                        .child(title.into()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_size(px(10.0))
                        .line_height(px(12.0))
                        .text_color(cx.theme().muted_foreground)
                        .child(render_icon(AppIcon::AudioLines, IconTone::Muted, 11.0, cx))
                        .child(detail.into()),
                )
                .child(render_progress(current, progress, seek_slider, cx)),
        )
        .into_any_element()
}

fn render_progress(
    current: bool,
    progress: f32,
    seek_slider: &Entity<SliderState>,
    cx: &App,
) -> AnyElement {
    if current {
        Slider::new(seek_slider)
            .w_full()
            .bg(cx.theme().primary)
            .into_any_element()
    } else {
        div()
            .w_full()
            .h(px(3.0))
            .overflow_hidden()
            .rounded_full()
            .bg(cx.theme().border)
            .child(
                div()
                    .h_full()
                    .w(relative(progress.clamp(0.0, 1.0)))
                    .rounded_full()
                    .bg(cx.theme().primary),
            )
            .into_any_element()
    }
}

pub(crate) fn format_audio_duration(duration_ms: u64) -> String {
    format_audio_seconds(duration_ms.div_ceil(1_000))
}

pub(crate) fn format_audio_position(position_ms: u64) -> String {
    format_audio_seconds(position_ms / 1_000)
}

fn format_audio_seconds(seconds: u64) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_time_uses_compact_minutes_and_seconds() {
        assert_eq!(format_audio_duration(1), "0:01");
        assert_eq!(format_audio_duration(60_000), "1:00");
        assert_eq!(format_audio_duration(300_000), "5:00");
        assert_eq!(format_audio_position(999), "0:00");
        assert_eq!(format_audio_position(65_100), "1:05");
    }
}
