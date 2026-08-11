use std::time::Duration;

use gpui::{Context, Task};

use super::OneChat;
use crate::{
    application::attachments::MAX_ATTACHMENTS,
    desktop::audio_recording::{RecordingLimit, RecordingStatus},
    domain::{
        AttachmentDraft, AttachmentDraftFile, AttachmentFileKind, AttachmentKind,
        AudioAttachmentMetadata, AudioAttachmentSource, new_id,
    },
};

impl OneChat {
    pub(crate) fn start_audio_recording_observer(&mut self, cx: &mut Context<Self>) {
        let previous = std::mem::replace(&mut self.chat.audio_recording_task, Task::ready(()));
        previous.detach();
        self.chat.audio_recording_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(50))
                    .await;
                if this
                    .update(cx, |this, cx| {
                        let snapshot = this.services.audio_recording.snapshot();
                        if snapshot == this.chat.audio_recording {
                            return;
                        }
                        this.chat.audio_recording = snapshot.clone();
                        match snapshot.status {
                            RecordingStatus::Completed => {
                                this.finish_recorded_voice(snapshot.output.as_deref(), cx);
                            }
                            RecordingStatus::Failed => {
                                this.chat.recording_conversation_id = None;
                                if let Some(error) = snapshot.error {
                                    this.data.error = Some(error);
                                }
                                this.services.audio_recording.reset();
                            }
                            _ => {}
                        }
                        cx.notify();
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
    }

    pub(crate) fn recording_active(&self) -> bool {
        self.services.audio_recording.snapshot().status.is_active()
    }

    pub(crate) fn can_start_voice_recording(&self) -> bool {
        !self.recording_active()
            && !self.is_current_generating()
            && self.chat.message_editor.is_none()
            && !self.chat.attachments_loading
            && self.chat.attachments.len() < MAX_ATTACHMENTS
            && self.current_conversation().is_some()
            && self
                .current_model()
                .is_some_and(|model| model.capabilities.audio_input)
    }

    pub(crate) fn toggle_voice_recording(&mut self, cx: &mut Context<Self>) {
        match self.services.audio_recording.snapshot().status {
            RecordingStatus::Recording => self.stop_voice_recording(cx),
            RecordingStatus::RequestingPermission | RecordingStatus::Finalizing => {}
            RecordingStatus::Idle | RecordingStatus::Completed | RecordingStatus::Failed => {
                self.start_voice_recording(cx)
            }
        }
    }

    fn start_voice_recording(&mut self, cx: &mut Context<Self>) {
        if !self.can_start_voice_recording() {
            return;
        }
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            return;
        };
        self.stop_audio_playback();
        self.chat.recording_conversation_id = Some(conversation_id);
        self.services.audio_recording.start();
        self.chat.audio_recording = self.services.audio_recording.snapshot();
        cx.notify();
    }

    pub(crate) fn stop_voice_recording(&mut self, cx: &mut Context<Self>) {
        if self.services.audio_recording.snapshot().status == RecordingStatus::Recording {
            self.services.audio_recording.stop();
            self.chat.audio_recording = self.services.audio_recording.snapshot();
            cx.notify();
        }
    }

    pub(crate) fn cancel_voice_recording(&mut self, cx: &mut Context<Self>) {
        if self.services.audio_recording.snapshot().status == RecordingStatus::Idle {
            return;
        }
        self.services.audio_recording.cancel();
        self.chat.audio_recording = self.services.audio_recording.snapshot();
        self.chat.recording_conversation_id = None;
        cx.notify();
    }

    fn finish_recorded_voice(
        &mut self,
        output: Option<&crate::desktop::audio_recording::RecordingOutput>,
        cx: &mut Context<Self>,
    ) {
        let valid_context = self.chat.recording_conversation_id.as_deref()
            == self
                .current_conversation()
                .map(|conversation| conversation.id.as_str());
        self.chat.recording_conversation_id = None;
        let Some(output) = output.filter(|_| valid_context) else {
            self.services.audio_recording.reset();
            return;
        };
        if self.chat.attachments.len() >= MAX_ATTACHMENTS {
            self.data.error = Some(format!(
                "A message can contain at most {MAX_ATTACHMENTS} attachments. The recording was discarded."
            ));
            self.services.audio_recording.reset();
            return;
        }
        if !self
            .current_model()
            .is_some_and(|model| model.capabilities.audio_input)
        {
            self.data.error = Some(
                "The selected model no longer accepts audio. The recording was discarded.".into(),
            );
            self.services.audio_recording.reset();
            return;
        }

        self.chat.attachments.push(voice_attachment(output));
        self.chat.attachments_revision = self.chat.attachments_revision.wrapping_add(1);
        if let Some(limit) = output.limit {
            self.data.error = Some(
                match limit {
                    RecordingLimit::Duration => {
                        "The recording reached the 5-minute limit and was added as a voice draft."
                    }
                    RecordingLimit::Size => {
                        "The recording reached the 10 MiB limit and was added as a voice draft."
                    }
                }
                .into(),
            );
        }
        self.services.audio_recording.reset();
        self.navigation.pending_focus = Some(super::PendingFocus::Composer);
        cx.notify();
    }
}

fn voice_attachment(output: &crate::desktop::audio_recording::RecordingOutput) -> AttachmentDraft {
    AttachmentDraft {
        id: new_id("attachment"),
        name: "Voice message.wav".into(),
        kind: AttachmentKind::Audio,
        files: vec![AttachmentDraftFile {
            name: "content.wav".into(),
            kind: AttachmentFileKind::Audio,
            media_type: "audio/wav".into(),
            bytes: output.wav.clone(),
        }],
        audio: Some(AudioAttachmentMetadata {
            duration_ms: output.duration_ms,
            source: AudioAttachmentSource::Voice,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desktop::audio_recording::RecordingOutput;

    #[test]
    fn completed_recording_becomes_a_valid_voice_draft() {
        let draft = voice_attachment(&RecordingOutput {
            wav: b"RIFF voice".to_vec(),
            duration_ms: 1_250,
            limit: None,
        });

        assert_eq!(draft.name, "Voice message.wav");
        assert_eq!(draft.kind, AttachmentKind::Audio);
        assert_eq!(draft.files[0].media_type, "audio/wav");
        assert_eq!(draft.files[0].bytes, b"RIFF voice");
        assert_eq!(draft.validate_files(), Ok(()));
        assert_eq!(
            draft.audio,
            Some(AudioAttachmentMetadata {
                duration_ms: 1_250,
                source: AudioAttachmentSource::Voice,
            })
        );
    }
}
