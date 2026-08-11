use gpui::{App, Context, Entity, Window, prelude::*};
use gpui_component::input::{InputEvent, InputState};

use super::super::{
    AssistantOutputEditor, MessageEditor, MessageEditorTarget, OneChat, PendingFocus,
    multiline_input,
};
use crate::{
    application::generation::PreparedGeneration,
    desktop::ui::inspector::InspectorTab,
    domain::{AssistantBlock, AssistantResponse, Turn, new_id, now_timestamp},
};

const ASSISTANT_EDITOR_MAX_ROWS: usize = 24;

impl OneChat {
    pub(crate) fn show_response(
        &mut self,
        turn_id: String,
        response_id: String,
        cx: &mut Context<Self>,
    ) {
        let valid = self
            .data
            .snapshot
            .current_turns
            .iter()
            .find(|turn| turn.id == turn_id)
            .is_some_and(|turn| turn.response(&response_id).is_some());
        if valid {
            self.chat.visible_response_ids.insert(turn_id, response_id);
            cx.notify();
        }
    }

    pub(crate) fn use_response_for_context(
        &mut self,
        turn_id: String,
        response_id: String,
        cx: &mut Context<Self>,
    ) {
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            return;
        };
        self.mutate_and_reload(
            move |storage| {
                storage.set_continuation_response(&conversation_id, &turn_id, &response_id)
            },
            cx,
        );
    }

    pub(crate) fn fork_from_response(&mut self, response_id: String, cx: &mut Context<Self>) {
        self.cancel_voice_recording(cx);
        if self.chat.message_editor.is_some() {
            return;
        }
        let Some(response) = self
            .response(&response_id)
            .map(|(_, response)| response.clone())
        else {
            return;
        };
        if !response.is_usable_as_context() {
            return;
        }
        let Some(source) = self.current_conversation().cloned() else {
            return;
        };

        let now = now_timestamp();
        let mut conversation = source.clone();
        conversation.id = new_id("conversation");
        conversation.title = format!("{} (fork)", source.title);
        if self
            .data
            .snapshot
            .models
            .iter()
            .any(|model| model.id == response.model_id)
        {
            conversation.model_id = Some(response.model_id);
        }
        conversation.pinned = false;
        conversation.created_at = now;
        conversation.updated_at = now;

        let source_id = source.id;
        let fork_id = conversation.id.clone();
        let mut settings = self.data.snapshot.settings.clone();
        settings.current_conversation_id = Some(fork_id);
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(
            move |storage| {
                storage.fork_conversation(&source_id, &response_id, &conversation)?;
                storage.save_settings(&settings)
            },
            cx,
        );
    }

    pub(crate) fn user_message_editor(&self, turn: &Turn) -> Option<&MessageEditor> {
        self.chat.message_editor.as_ref().filter(
            |editor| matches!(&editor.target, MessageEditorTarget::User(id) if id == &turn.id),
        )
    }

    pub(crate) fn assistant_message_editor(
        &self,
        response: &AssistantResponse,
    ) -> Option<&MessageEditor> {
        self.chat.message_editor.as_ref().filter(|editor| {
            matches!(&editor.target, MessageEditorTarget::Assistant(id) if id == &response.id)
        })
    }

    pub(crate) fn active_message_editor(&self) -> Option<Entity<InputState>> {
        self.chat
            .message_editor
            .as_ref()
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn can_save_user_edit(&self, turn_id: &str, cx: &App) -> bool {
        let Some(editor) = self.chat.message_editor.as_ref().filter(
            |editor| matches!(&editor.target, MessageEditorTarget::User(id) if id == turn_id),
        ) else {
            return false;
        };
        let Some(turn) = self
            .data
            .snapshot
            .current_turns
            .iter()
            .find(|turn| turn.id == turn_id)
        else {
            return false;
        };
        let content = editor.input.read(cx).value().trim().to_string();
        let valid = !content.is_empty()
            || !editor.attachments.is_empty()
            || !editor.attachment_drafts.is_empty();
        let changed = content != turn.user.content
            || editor.attachments != turn.user.attachments
            || !editor.attachment_drafts.is_empty();
        editor.attachment_load_id.is_none() && valid && changed
    }

    pub(crate) fn can_save_assistant_edit(&self, response_id: &str, cx: &App) -> bool {
        let Some(editor) = self.chat.message_editor.as_ref().filter(|editor| {
            matches!(&editor.target, MessageEditorTarget::Assistant(id) if id == response_id)
        }) else {
            return false;
        };
        let Some((_, response)) = self.response(response_id) else {
            return false;
        };
        let outputs = editor
            .output_editors
            .iter()
            .map(|output| (output.block_id.as_str(), output.input.read(cx).value()))
            .collect::<Vec<_>>();
        let valid = outputs
            .iter()
            .any(|(_, content)| !content.trim().is_empty());
        let changed = outputs.iter().any(|(block_id, content)| {
            if response.blocks.is_empty() {
                content.as_str() != response.content
            } else {
                response.blocks.iter().find_map(|block| match block {
                    AssistantBlock::Output { id, content } if id == block_id => {
                        Some(content.as_str())
                    }
                    _ => None,
                }) != Some(content.as_str())
            }
        });
        valid && changed
    }

    pub(crate) fn begin_edit_user(
        &mut self,
        turn_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_current_generating()
            || self.recording_active()
            || self.chat.message_editor.is_some()
        {
            return;
        }
        let Some((content, attachments)) = self
            .data
            .snapshot
            .current_turns
            .iter()
            .find(|turn| turn.id == turn_id)
            .map(|turn| (turn.user.content.clone(), turn.user.attachments.clone()))
        else {
            return;
        };
        let input = cx.new(|cx| multiline_input(content, "Edit user message", window, cx));
        cx.subscribe_in(&input, window, |_, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        self.chat.message_editor = Some(MessageEditor {
            target: MessageEditorTarget::User(turn_id),
            input,
            output_editors: Vec::new(),
            attachments,
            attachment_drafts: Vec::new(),
            attachment_previews: Default::default(),
            attachment_load_id: None,
        });
        self.navigation.pending_focus = Some(PendingFocus::MessageEditor);
        self.jump_to_message_editor(cx);
        cx.notify();
    }

    pub(crate) fn begin_edit_assistant(
        &mut self,
        response_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_current_generating()
            || self.recording_active()
            || self.chat.message_editor.is_some()
        {
            return;
        }
        let Some((_, response)) = self.response(&response_id) else {
            return;
        };
        let outputs = if response.blocks.is_empty() {
            vec![(response.id.clone(), response.content.clone())]
        } else {
            response
                .blocks
                .iter()
                .filter_map(|block| match block {
                    AssistantBlock::Output { id, content } => Some((id.clone(), content.clone())),
                    _ => None,
                })
                .collect()
        };
        if outputs.is_empty() {
            return;
        }
        let mut output_editors = Vec::with_capacity(outputs.len());
        for (block_id, content) in outputs {
            let input = cx.new(|cx| {
                multiline_input(content, "Edit assistant output", window, cx)
                    .auto_grow(1, ASSISTANT_EDITOR_MAX_ROWS)
            });
            cx.subscribe_in(&input, window, |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
            output_editors.push(AssistantOutputEditor { block_id, input });
        }
        let input = output_editors[0].input.clone();
        self.chat.message_editor = Some(MessageEditor {
            target: MessageEditorTarget::Assistant(response_id),
            input,
            output_editors,
            attachments: Vec::new(),
            attachment_drafts: Vec::new(),
            attachment_previews: Default::default(),
            attachment_load_id: None,
        });
        self.navigation.pending_focus = Some(PendingFocus::MessageEditor);
        self.jump_to_message_editor(cx);
        cx.notify();
    }

    pub(crate) fn cancel_message_edit(&mut self, cx: &mut Context<Self>) {
        self.stop_audio_playback();
        self.chat.message_editor = None;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn save_user_edit(&mut self, turn_id: String, cx: &mut Context<Self>) {
        if !self.can_save_user_edit(&turn_id, cx) {
            return;
        }
        let Some(editor) = self.chat.message_editor.as_ref().filter(
            |editor| matches!(&editor.target, MessageEditorTarget::User(id) if id == &turn_id),
        ) else {
            return;
        };
        let content = editor.input.read(cx).value().trim().to_string();
        let retained_attachments = editor.attachments.clone();
        let attachment_drafts = editor.attachment_drafts.clone();
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
        let (conversation, provider, model) = match self.generation_target(None) {
            Ok(target) => target,
            Err(error) => {
                self.data.error = Some(error);
                cx.notify();
                return;
            }
        };
        if retained_attachments
            .iter()
            .map(|attachment| attachment.kind)
            .chain(attachment_drafts.iter().map(|attachment| attachment.kind))
            .any(|kind| !Self::attachment_kind_supported(&model, kind))
        {
            self.data.error = Some(
                "The selected model cannot read one or more attachments in the edited message."
                    .into(),
            );
            cx.notify();
            return;
        }
        let new_attachments = match self
            .services
            .storage
            .store_attachments(&conversation.id, &attachment_drafts)
        {
            Ok(attachments) => attachments,
            Err(error) => {
                self.data.error = Some(format!("Could not save attachments: {error}"));
                cx.notify();
                return;
            }
        };
        let mut attachments = retained_attachments;
        attachments.extend(new_attachments.clone());
        let prepared =
            match self.prepare_with_storage_context(&conversation, &model, |context_policy| {
                PreparedGeneration::new(
                    &conversation,
                    &provider,
                    &model,
                    &self.data.snapshot.current_turns,
                    turn.parent_response_id,
                    crate::domain::UserMessage::new(content, attachments),
                    context_policy,
                )
            }) {
                Ok(prepared) => prepared,
                Err(error) => {
                    let _ = self
                        .services
                        .storage
                        .remove_attachments(&conversation.id, &new_attachments);
                    self.data.error = Some(format!("Could not load attachments: {error}"));
                    cx.notify();
                    return;
                }
            }
            .with_new_attachments(new_attachments);
        self.stop_audio_playback();
        self.chat.message_editor = None;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        self.begin_prepared_generation(prepared, cx);
    }

    pub(crate) fn select_user_branch(&mut self, turn_id: String, cx: &mut Context<Self>) {
        if self.is_current_generating()
            || self.recording_active()
            || self.chat.message_editor.is_some()
        {
            return;
        }
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            return;
        };
        self.chat.selected_request_id = None;
        self.chat.visible_response_ids.clear();
        self.mutate_and_reload(
            move |storage| storage.select_user_branch(&conversation_id, &turn_id),
            cx,
        );
    }

    pub(crate) fn save_assistant_edit(&mut self, response_id: String, cx: &mut Context<Self>) {
        if !self.can_save_assistant_edit(&response_id, cx) {
            return;
        }
        let Some(editor) = self.chat.message_editor.as_ref().filter(|editor| {
            matches!(&editor.target, MessageEditorTarget::Assistant(id) if id == &response_id)
        }) else {
            return;
        };
        let outputs = editor
            .output_editors
            .iter()
            .map(|output| {
                (
                    output.block_id.clone(),
                    output.input.read(cx).value().to_string(),
                )
            })
            .collect::<Vec<_>>();
        let Some((turn_id, mut response)) = self
            .response(&response_id)
            .map(|(turn, response)| (turn.id.clone(), response.clone()))
        else {
            return;
        };
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            return;
        };
        response.replace_outputs(&outputs);
        response.updated_at = now_timestamp();
        self.chat.message_editor = None;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(
            move |storage| storage.update_response(&conversation_id, &turn_id, &response),
            cx,
        );
    }

    pub(crate) fn inspect_message_request(&mut self, response_id: String, cx: &mut Context<Self>) {
        let request_id = self
            .response(&response_id)
            .and_then(|(_, response)| response.request_id.clone());
        if let Some(request_id) = request_id {
            self.chat.selected_request_id = Some(request_id);
            self.navigation.inspector_tab = InspectorTab::Info;
            self.set_inspector_open(true, true, cx);
        }
    }
}
