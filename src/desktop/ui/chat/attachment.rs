use super::*;
use crate::{
    desktop::audio_playback::PlaybackStatus,
    domain::{AttachmentFileKind, AttachmentKind, AudioAttachmentMetadata, AudioAttachmentSource},
};

pub(super) fn attachment_detail(
    name: &str,
    kind: AttachmentKind,
    files: impl IntoIterator<Item = AttachmentFileKind>,
    audio: Option<&AudioAttachmentMetadata>,
) -> String {
    let (file_count, image_count) = files.into_iter().fold((0, 0), |(files, images), kind| {
        (
            files + 1,
            images + usize::from(kind == AttachmentFileKind::Image),
        )
    });
    match kind {
        AttachmentKind::Text => "Text document".into(),
        AttachmentKind::Image => "Image".into(),
        AttachmentKind::Audio => audio.map_or_else(
            || "Audio".into(),
            |audio| format!("Audio · {}", format_audio_duration(audio.duration_ms)),
        ),
        AttachmentKind::Pdf => format!(
            "PDF · {file_count} page{}",
            if file_count == 1 { "" } else { "s" }
        ),
        AttachmentKind::Document => {
            let extension = std::path::Path::new(name).extension();
            let document = match extension {
                Some(extension) if extension.eq_ignore_ascii_case("docx") => "Word document",
                Some(extension) if extension.eq_ignore_ascii_case("xlsx") => "Excel spreadsheet",
                Some(extension) if extension.eq_ignore_ascii_case("pptx") => "PowerPoint slides",
                _ => "Document",
            };
            if image_count == 0 {
                document.into()
            } else {
                format!(
                    "{document} · {image_count} image{}",
                    if image_count == 1 { "" } else { "s" }
                )
            }
        }
    }
}

pub(super) fn format_audio_duration(duration_ms: u64) -> String {
    let seconds = duration_ms.div_ceil(1_000);
    let minutes = seconds / 60;
    let seconds = seconds % 60;
    format!("{minutes}:{seconds:02}")
}

pub(super) fn render_audio_attachment_card(
    app: &OneChat,
    attachment_id: &str,
    name: &str,
    audio: Option<&AudioAttachmentMetadata>,
    width: f32,
    remove: Option<AnyElement>,
    cx: &mut Context<OneChat>,
) -> AnyElement {
    let is_current = app.chat.audio_playback.attachment_id.as_deref() == Some(attachment_id);
    let status = if is_current {
        app.chat.audio_playback.status
    } else {
        PlaybackStatus::Idle
    };
    let duration_ms = audio.map_or(0, |audio| audio.duration_ms);
    let position_ms = if is_current {
        app.chat.audio_playback.position_ms.min(duration_ms)
    } else {
        0
    };
    let progress = if duration_ms == 0 {
        0.0
    } else {
        position_ms as f32 / duration_ms as f32
    };
    let title = if audio.is_some_and(|audio| audio.source == AudioAttachmentSource::Voice) {
        "Voice message".to_string()
    } else {
        name.to_string()
    };
    let detail = match status {
        PlaybackStatus::Loading => format!("Loading… · {}", format_audio_duration(duration_ms)),
        PlaybackStatus::Playing | PlaybackStatus::Paused => format!(
            "{} / {}",
            format_audio_position(position_ms),
            format_audio_duration(duration_ms)
        ),
        PlaybackStatus::Idle | PlaybackStatus::Failed => {
            format!("Audio · {}", format_audio_duration(duration_ms))
        }
    };
    let id = attachment_id.to_string();
    let playing = status == PlaybackStatus::Playing;
    let loading = status == PlaybackStatus::Loading;
    let background = if remove.is_some() {
        cx.theme().muted
    } else {
        cx.theme().popover
    };

    div()
        .relative()
        .w(px(width))
        .h(px(76.0))
        .flex_none()
        .rounded(px(16.0))
        .border_1()
        .border_color(cx.theme().border)
        .bg(background)
        .shadow_xs()
        .p_2()
        .pr(px(if remove.is_some() { 34.0 } else { 10.0 }))
        .flex()
        .items_center()
        .gap_3()
        .child(
            Button::new(SharedString::from(format!(
                "audio-playback-{attachment_id}"
            )))
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
            .on_click(
                cx.listener(move |this, _, _, cx| this.toggle_audio_playback(id.clone(), cx)),
            ),
        )
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
                        .child(title),
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
                        .child(detail),
                )
                .child(
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
                        ),
                ),
        )
        .children(remove)
        .into_any_element()
}

fn format_audio_position(duration_ms: u64) -> String {
    let seconds = duration_ms / 1_000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn office_details_are_format_specific_and_only_count_images() {
        let cases = [
            ("Report.DOCX", "Word document"),
            ("Budget.XLSX", "Excel spreadsheet"),
            ("Deck.PPTX", "PowerPoint slides"),
        ];

        for (name, detail) in cases {
            assert_eq!(
                attachment_detail(
                    name,
                    AttachmentKind::Document,
                    [AttachmentFileKind::Text],
                    None
                ),
                detail
            );
            assert_eq!(
                attachment_detail(
                    name,
                    AttachmentKind::Document,
                    [
                        AttachmentFileKind::Text,
                        AttachmentFileKind::Image,
                        AttachmentFileKind::Image,
                    ],
                    None,
                ),
                format!("{detail} · 2 images")
            );
        }
    }

    #[test]
    fn standard_attachment_details_are_consistent() {
        assert_eq!(
            attachment_detail(
                "document.pdf",
                AttachmentKind::Pdf,
                [AttachmentFileKind::Image],
                None,
            ),
            "PDF · 1 page"
        );
        assert_eq!(
            attachment_detail("photo.png", AttachmentKind::Image, [], None),
            "Image"
        );
        assert_eq!(
            attachment_detail("notes.txt", AttachmentKind::Text, [], None),
            "Text document"
        );
        assert_eq!(
            attachment_detail(
                "recording.wav",
                AttachmentKind::Audio,
                [AttachmentFileKind::Audio],
                Some(&AudioAttachmentMetadata {
                    duration_ms: 65_100,
                    source: AudioAttachmentSource::Upload,
                }),
            ),
            "Audio · 1:06"
        );
    }

    #[test]
    fn audio_duration_uses_a_compact_minutes_and_seconds_format() {
        assert_eq!(format_audio_duration(1), "0:01");
        assert_eq!(format_audio_duration(60_000), "1:00");
        assert_eq!(format_audio_duration(300_000), "5:00");
    }
}
