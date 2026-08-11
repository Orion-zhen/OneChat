use gpui::{Context, Window};

use super::super::OneChat;
use crate::{
    application::generation::{ContextPolicy, PreparedGeneration},
    domain::{AttachmentKind, Conversation, MessageStatus, Model, Provider},
};

fn attempt_composer_submission(
    prompt: String,
    has_attachments: bool,
    start: impl FnOnce(String) -> bool,
) -> bool {
    (!prompt.trim().is_empty() || has_attachments) && start(prompt)
}

impl OneChat {
    pub(crate) fn is_conversation_generating(&self, conversation_id: &str) -> bool {
        self.chat.generations.is_active(conversation_id)
    }

    pub(crate) fn is_current_generating(&self) -> bool {
        self.current_conversation()
            .is_some_and(|conversation| self.is_conversation_generating(&conversation.id))
    }

    pub(crate) fn attachment_context_supported(&self) -> bool {
        self.current_model().is_none_or(|model| {
            self.chat
                .attachments
                .iter()
                .all(|attachment| Self::attachment_kind_supported(model, attachment.kind))
        })
    }

    pub(in crate::desktop::app) fn attachment_kind_supported(
        model: &Model,
        kind: AttachmentKind,
    ) -> bool {
        (!kind.requires_vision() || model.capabilities.vision)
            && (!kind.requires_audio_input() || model.capabilities.audio_input)
    }

    pub(crate) fn send_composer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.recording_active() {
            return;
        }
        let prompt = self.chat.composer.read(cx).value().to_string();
        if self.chat.attachments_loading {
            return;
        }
        if !attempt_composer_submission(prompt, !self.chat.attachments.is_empty(), |prompt| {
            self.start_generation(prompt, cx)
        }) {
            return;
        }

        self.chat.composer.update(cx, |composer, cx| {
            composer.set_value("", window, cx);
        });
        self.stop_audio_playback();
        self.chat.attachments.clear();
        self.chat.attachment_previews.clear();
    }

    pub(crate) fn stop_current_generation(&mut self, cx: &mut Context<Self>) {
        if let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone()) {
            self.chat.generations.stop(&conversation_id);
            cx.notify();
        }
    }

    pub(super) fn start_generation(&mut self, prompt: String, cx: &mut Context<Self>) -> bool {
        if self.is_current_generating() {
            return false;
        }
        let (conversation, provider, model) = match self.generation_target(None) {
            Ok(target) => target,
            Err(error) => {
                self.data.error = Some(error);
                cx.notify();
                return false;
            }
        };
        if self
            .chat
            .attachments
            .iter()
            .any(|attachment| !Self::attachment_kind_supported(&model, attachment.kind))
        {
            self.data.error = Some(
                "The selected model cannot read one or more attachments in the current message."
                    .into(),
            );
            cx.notify();
            return false;
        }
        let parent_response_id = self
            .active_leaf_turn()
            .and_then(|turn| turn.continuation_response_id.clone());
        if self.active_leaf_turn().is_some_and(|turn| {
            parent_response_id
                .as_deref()
                .and_then(|id| turn.response(id))
                .is_none_or(|response| {
                    response.status != MessageStatus::Completed || response.content.is_empty()
                })
        }) {
            self.data.error = Some("Choose a completed response before continuing.".into());
            cx.notify();
            return false;
        }
        let history_limit = self.effective_history_limit(&conversation);
        let attachments = match self
            .services
            .storage
            .store_attachments(&conversation.id, &self.chat.attachments)
        {
            Ok(attachments) => attachments,
            Err(error) => {
                self.data.error = Some(format!("Could not save attachments: {error}"));
                cx.notify();
                return false;
            }
        };
        let storage = self.services.storage.clone();
        let conversation_id = conversation.id.clone();
        let include_document_images = model.capabilities.vision;
        let user_message = |user: &crate::domain::UserMessage| {
            storage
                .message_for_user(&conversation_id, user, include_document_images)
                .map_err(|error| error.to_string())
        };
        let prepared = match PreparedGeneration::new(
            &conversation,
            &provider,
            &model,
            &self.data.snapshot.current_turns,
            parent_response_id,
            crate::domain::UserMessage::new(prompt, attachments.clone()),
            ContextPolicy::new(history_limit, &user_message),
        ) {
            Ok(prepared) => prepared.with_new_attachments(attachments.clone()),
            Err(error) => {
                let _ = self
                    .services
                    .storage
                    .remove_attachments(&conversation.id, &attachments);
                self.data.error = Some(format!("Could not prepare attachments: {error}"));
                cx.notify();
                return false;
            }
        };
        self.begin_prepared_generation(prepared, cx);
        true
    }

    pub(crate) fn start_additional_response(
        &mut self,
        turn_id: String,
        model_id: String,
        cx: &mut Context<Self>,
    ) {
        let (conversation, provider, model) = match self.generation_target(Some(&model_id)) {
            Ok(target) => target,
            Err(error) => {
                self.data.error = Some(error);
                cx.notify();
                return;
            }
        };
        let Some(turn) = self
            .data
            .snapshot
            .current_turns
            .iter()
            .find(|turn| turn.id == turn_id)
            .cloned()
        else {
            return;
        };
        if turn
            .responses
            .iter()
            .any(|response| response.model_id == model.id)
        {
            self.data.error = Some("This model has already answered this message.".into());
            cx.notify();
            return;
        }
        let history_limit = self.effective_history_limit(&conversation);
        let storage = self.services.storage.clone();
        let conversation_id = conversation.id.clone();
        let include_document_images = model.capabilities.vision;
        let user_message = |user: &crate::domain::UserMessage| {
            storage
                .message_for_user(&conversation_id, user, include_document_images)
                .map_err(|error| error.to_string())
        };
        match PreparedGeneration::additional(
            &conversation,
            &provider,
            &model,
            &self.data.snapshot.current_turns,
            &turn,
            ContextPolicy::new(history_limit, &user_message),
        ) {
            Ok(prepared) => self.begin_prepared_generation(prepared, cx),
            Err(error) => {
                self.data.error = Some(format!("Could not load attachments: {error}"));
                cx.notify();
            }
        }
    }

    pub(in crate::desktop::app) fn generation_target(
        &self,
        model_id: Option<&str>,
    ) -> Result<(Conversation, Provider, Model), String> {
        let conversation = self
            .current_conversation()
            .cloned()
            .ok_or_else(|| "Create or select a conversation first.".to_string())?;
        let model = if let Some(model_id) = model_id {
            self.data
                .snapshot
                .models
                .iter()
                .find(|model| model.id == model_id)
        } else {
            self.current_model()
        }
        .cloned()
        .ok_or_else(|| "Choose a model before sending.".to_string())?;
        let provider = self
            .provider_for_model(&model)
            .cloned()
            .ok_or_else(|| "The selected model has no provider.".to_string())?;
        if !provider.streaming {
            return Err("The selected provider does not support streaming.".into());
        }
        if !provider.enabled {
            return Err("The selected provider is disabled.".into());
        }
        Ok((conversation, provider, model))
    }

    pub(crate) fn regenerate_assistant(&mut self, response_id: String, cx: &mut Context<Self>) {
        let Some((turn, response)) = self
            .response(&response_id)
            .map(|(turn, response)| (turn.clone(), response.clone()))
        else {
            return;
        };
        if !self.is_latest_turn(&turn.id) {
            self.data.error = Some("Only responses in the latest turn can be regenerated.".into());
            cx.notify();
            return;
        }
        let current_reasoning_preset = self
            .chat
            .generation_config_editor
            .as_ref()
            .map(|editor| editor.reasoning_preset().map(str::to_owned));
        let (mut conversation, provider, model) =
            match self.generation_target(Some(&response.model_id)) {
                Ok(target) => target,
                Err(error) => {
                    self.data.error = Some(error);
                    cx.notify();
                    return;
                }
            };
        if let Some(reasoning_preset) = current_reasoning_preset {
            conversation.generation_config.reasoning_preset = reasoning_preset;
        }
        let history_limit = self.effective_history_limit(&conversation);
        let storage = self.services.storage.clone();
        let conversation_id = conversation.id.clone();
        let include_document_images = model.capabilities.vision;
        let user_message = |user: &crate::domain::UserMessage| {
            storage
                .message_for_user(&conversation_id, user, include_document_images)
                .map_err(|error| error.to_string())
        };
        match PreparedGeneration::regenerate(
            &conversation,
            &provider,
            &model,
            &self.data.snapshot.current_turns,
            &turn,
            &response,
            ContextPolicy::new(history_limit, &user_message),
        ) {
            Ok(prepared) => self.begin_prepared_generation(prepared, cx),
            Err(error) => {
                self.data.error = Some(format!("Could not load attachments: {error}"));
                cx.notify();
            }
        }
    }
}
