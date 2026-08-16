use std::time::Duration;

use gpui::{AppContext as _, Context, Task};
use gpui_component::slider::{SliderEvent, SliderState};

use super::{OneChat, PlaybackState};
use crate::{
    desktop::audio_playback::{PlaybackSnapshot, PlaybackSource, PlaybackStatus},
    domain::AttachmentFileKind,
    speech::AudioClip,
};

const ATTACHMENT_SOURCE_PREFIX: &str = "attachment:";
const TTS_SOURCE_PREFIX: &str = "tts:";

pub(crate) fn tts_combined_source_id(revision: u64) -> String {
    format!("{TTS_SOURCE_PREFIX}{revision}:combined")
}

pub(crate) fn tts_segment_source_id(revision: u64, index: usize) -> String {
    format!("{TTS_SOURCE_PREFIX}{revision}:segment:{index}")
}

pub(crate) fn attachment_source_id(attachment_id: &str) -> String {
    format!("{ATTACHMENT_SOURCE_PREFIX}{attachment_id}")
}

impl PlaybackState {
    pub(crate) fn new(cx: &mut Context<OneChat>) -> Self {
        let seek_slider = cx.new(|_| {
            SliderState::new()
                .min(0.0)
                .max(1.0)
                .step(0.001)
                .default_value(0.0)
        });
        cx.subscribe(
            &seek_slider,
            |this, _, event: &SliderEvent, cx| match event {
                SliderEvent::Change(value) => {
                    this.playback.seek_preview = Some(value.start());
                    cx.notify();
                }
                SliderEvent::Release(value) => {
                    this.seek_audio_playback(value.start(), cx);
                }
            },
        )
        .detach();
        Self {
            snapshot: PlaybackSnapshot::default(),
            seek_slider,
            seek_preview: None,
            seek_target_ms: None,
            observer_task: Task::ready(()),
        }
    }
}

impl OneChat {
    pub(crate) fn start_audio_playback_observer(&mut self, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.playback.observer_task, Task::ready(()));
        previous.detach();
        self.playback.observer_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        let snapshot = this.services.audio_playback.snapshot();
                        if snapshot.source_id != this.playback.snapshot.source_id {
                            this.playback.seek_preview = None;
                            this.playback.seek_target_ms = None;
                        }
                        if this
                            .playback
                            .seek_target_ms
                            .is_some_and(|target| snapshot.position_ms.abs_diff(target) <= 100)
                        {
                            this.playback.seek_target_ms = None;
                            this.playback.seek_preview = None;
                        }
                        if snapshot == this.playback.snapshot {
                            return;
                        }
                        if snapshot.revision != this.playback.snapshot.revision
                            && let Some(error) = snapshot.error.clone()
                        {
                            this.data.error = Some(error);
                        }
                        this.playback.snapshot = snapshot;
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
    }

    pub(crate) fn toggle_audio_playback(&mut self, attachment_id: String, cx: &mut Context<Self>) {
        let source_id = attachment_source_id(&attachment_id);
        if self.toggle_current_playback(&source_id) {
            return;
        }
        self.start_audio_attachment(attachment_id, source_id, cx);
    }

    pub(crate) fn toggle_tts_audio_playback(
        &mut self,
        source_id: String,
        clip: AudioClip,
        _: &mut Context<Self>,
    ) {
        if self.toggle_current_playback(&source_id) {
            return;
        }
        let duration_ms = clip_duration_ms(&clip);
        self.services
            .audio_playback
            .play(source_id, PlaybackSource::Clip(clip), duration_ms);
    }

    fn toggle_current_playback(&self, source_id: &str) -> bool {
        let state = self.services.audio_playback.snapshot();
        if state.source_id.as_deref() != Some(source_id) {
            return false;
        }
        match state.status {
            PlaybackStatus::Playing => self.services.audio_playback.pause(),
            PlaybackStatus::Paused => self.services.audio_playback.resume(),
            PlaybackStatus::Loading => {}
            PlaybackStatus::Idle | PlaybackStatus::Failed => return false,
        }
        true
    }

    fn start_audio_attachment(
        &mut self,
        attachment_id: String,
        source_id: String,
        cx: &mut Context<Self>,
    ) {
        let draft = self
            .chat
            .attachments
            .iter()
            .chain(
                self.chat
                    .message_editor
                    .iter()
                    .flat_map(|editor| &editor.attachment_drafts),
            )
            .find(|attachment| attachment.id == attachment_id)
            .and_then(|attachment| {
                let duration_ms = attachment.audio.as_ref()?.duration_ms;
                let bytes = attachment
                    .files
                    .iter()
                    .find(|file| file.kind == AttachmentFileKind::Audio)?
                    .bytes
                    .clone();
                Some((PlaybackSource::Bytes(bytes), duration_ms))
            });
        let stored = || {
            let attachment = self
                .data
                .snapshot
                .current_turns
                .iter()
                .flat_map(|turn| &turn.user.attachments)
                .find(|attachment| attachment.id == attachment_id)?;
            let duration_ms = attachment.audio.as_ref()?.duration_ms;
            let file = attachment
                .files
                .iter()
                .find(|file| file.kind == AttachmentFileKind::Audio)?;
            self.temporary_attachment_bytes(file)
                .map(|bytes| (PlaybackSource::Bytes(bytes.to_vec()), duration_ms))
                .or_else(|| {
                    self.attachment_file_path(file)
                        .map(|path| (PlaybackSource::File(path), duration_ms))
                })
        };
        let Some((source, duration_ms)) = draft.or_else(stored) else {
            self.stop_audio_playback();
            self.data.error = Some("The audio attachment is no longer available.".into());
            cx.notify();
            return;
        };

        self.services
            .audio_playback
            .play(source_id, source, duration_ms);
    }

    pub(crate) fn seek_audio_playback(&mut self, ratio: f32, cx: &mut Context<Self>) {
        let duration_ms = self.playback.snapshot.duration_ms;
        if duration_ms > 0 {
            let ratio = ratio.clamp(0.0, 1.0);
            let position_ms = (duration_ms as f64 * f64::from(ratio)).round() as u64;
            self.playback.seek_preview = Some(ratio);
            self.playback.seek_target_ms = Some(position_ms);
            self.services.audio_playback.seek(position_ms);
        }
        cx.notify();
    }

    pub(crate) fn stop_audio_playback(&self) {
        self.services.audio_playback.stop();
    }

    pub(crate) fn stop_tts_audio_playback(&self) {
        if self
            .services
            .audio_playback
            .snapshot()
            .source_id
            .as_deref()
            .is_some_and(|source| source.starts_with(TTS_SOURCE_PREFIX))
        {
            self.stop_audio_playback();
        }
    }

    pub(crate) fn stop_audio_playback_if(&self, attachment_id: &str) {
        let source_id = attachment_source_id(attachment_id);
        if self.services.audio_playback.snapshot().source_id.as_deref() == Some(&source_id) {
            self.stop_audio_playback();
        }
    }
}

fn clip_duration_ms(clip: &AudioClip) -> u64 {
    (f64::from(clip.duration_sec()) * 1_000.0).round() as u64
}
