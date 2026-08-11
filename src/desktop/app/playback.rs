use std::time::Duration;

use gpui::{Context, Task};

use super::OneChat;
use crate::{
    desktop::audio_playback::{PlaybackSource, PlaybackStatus},
    domain::AttachmentFileKind,
};

impl OneChat {
    pub(crate) fn start_audio_playback_observer(&mut self, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.chat.audio_playback_task, Task::ready(()));
        previous.detach();
        self.chat.audio_playback_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        let snapshot = this.services.audio_playback.snapshot();
                        if snapshot == this.chat.audio_playback {
                            return;
                        }
                        if snapshot.status == PlaybackStatus::Failed
                            && snapshot.revision != this.chat.audio_playback.revision
                            && let Some(error) = snapshot.error.clone()
                        {
                            this.data.error = Some(error);
                        }
                        this.chat.audio_playback = snapshot;
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
        let state = self.services.audio_playback.snapshot();
        if state.attachment_id.as_deref() == Some(&attachment_id) {
            match state.status {
                PlaybackStatus::Playing => self.services.audio_playback.pause(),
                PlaybackStatus::Paused => self.services.audio_playback.resume(),
                PlaybackStatus::Loading => {}
                PlaybackStatus::Idle | PlaybackStatus::Failed => {
                    self.start_audio_attachment(attachment_id, cx)
                }
            }
        } else {
            self.start_audio_attachment(attachment_id, cx);
        }
    }

    fn start_audio_attachment(&mut self, attachment_id: String, cx: &mut Context<Self>) {
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
            self.attachment_file_path(file)
                .map(|path| (PlaybackSource::File(path), duration_ms))
        };
        let Some((source, duration_ms)) = draft.or_else(stored) else {
            self.stop_audio_playback();
            self.data.error = Some("The audio attachment is no longer available.".into());
            cx.notify();
            return;
        };

        self.services
            .audio_playback
            .play(attachment_id, source, duration_ms);
    }

    pub(crate) fn stop_audio_playback(&self) {
        self.services.audio_playback.stop();
    }

    pub(crate) fn stop_audio_playback_if(&self, attachment_id: &str) {
        if self
            .services
            .audio_playback
            .snapshot()
            .attachment_id
            .as_deref()
            == Some(attachment_id)
        {
            self.stop_audio_playback();
        }
    }
}
