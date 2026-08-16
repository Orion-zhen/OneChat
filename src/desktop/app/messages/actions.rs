use gpui::{App, Context, Entity, TouchPhase, Window, prelude::*};
use gpui_component::input::{InputEvent, TextareaState};

use super::super::{
    AssistantTextEditor, AssistantTextKind, CONTENT_EDITOR_MAX_ROWS, MessageEditor,
    MessageEditorTarget, OneChat, PendingFocus, multiline_input,
};
use crate::{
    application::generation::PreparedGeneration,
    desktop::{
        branch_swipe::{BranchSwipeAction, BranchSwipeAvailability, BranchSwipeTarget},
        pressure_touch::{self, Feedback},
        ui::inspector::InspectorTab,
    },
    domain::{AssistantBlock, AssistantResponse, Turn, new_id, now_timestamp},
};

#[derive(Debug, PartialEq, Eq)]
struct SwipeNeighbors {
    previous: Option<String>,
    next: Option<String>,
}

impl SwipeNeighbors {
    fn availability(&self) -> BranchSwipeAvailability {
        BranchSwipeAvailability {
            previous: self.previous.is_some(),
            next: self.next.is_some(),
        }
    }

    fn destination(&self, action: BranchSwipeAction) -> Option<String> {
        match action {
            BranchSwipeAction::Previous => self.previous.clone(),
            BranchSwipeAction::Next => self.next.clone(),
            BranchSwipeAction::Boundary => None,
        }
    }
}

fn swipe_neighbors(ids: &[String], current_id: &str) -> Option<SwipeNeighbors> {
    let index = ids.iter().position(|id| id == current_id)?;
    Some(SwipeNeighbors {
        previous: index
            .checked_sub(1)
            .and_then(|index| ids.get(index))
            .cloned(),
        next: ids.get(index + 1).cloned(),
    })
}

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

    pub(crate) fn swipe_user_branch(
        &mut self,
        turn_id: &str,
        delta_x: f32,
        delta_y: f32,
        phase: TouchPhase,
        cx: &mut Context<Self>,
    ) {
        if self.is_current_generating()
            || self.recording_active()
            || self.chat.message_editor.is_some()
        {
            self.chat.branch_swipe.reset();
            return;
        }
        let Some(turn) = self
            .data
            .snapshot
            .current_turns
            .iter()
            .find(|turn| turn.id == turn_id)
        else {
            self.chat.branch_swipe.reset();
            return;
        };
        let ids = self
            .user_branches(turn)
            .into_iter()
            .map(|branch| branch.id.clone())
            .collect::<Vec<_>>();
        let Some(neighbors) = swipe_neighbors(&ids, turn_id) else {
            self.chat.branch_swipe.reset();
            return;
        };
        if ids.len() < 2 {
            self.chat.branch_swipe.reset();
            return;
        }
        let group_id = turn.parent_response_id.clone().unwrap_or_else(|| {
            format!(
                "root:{}",
                self.current_conversation()
                    .map_or("conversation", |conversation| conversation.id.as_str())
            )
        });
        let action = self.chat.branch_swipe.update(
            BranchSwipeTarget::User(group_id),
            delta_x,
            delta_y,
            phase,
            neighbors.availability(),
        );
        if self.chat.branch_swipe.captures_parent_scroll() {
            cx.stop_propagation();
        }
        let Some(action) = action else {
            return;
        };
        if let Some(destination) = neighbors.destination(action) {
            pressure_touch::feedback(Feedback::SelectionChanged);
            self.select_user_branch(destination, cx);
        } else {
            pressure_touch::feedback(Feedback::Boundary);
        }
    }

    pub(crate) fn swipe_assistant_response(
        &mut self,
        turn_id: &str,
        delta_x: f32,
        delta_y: f32,
        phase: TouchPhase,
        cx: &mut Context<Self>,
    ) {
        if self.chat.message_editor.is_some() {
            self.chat.branch_swipe.reset();
            return;
        }
        let Some(turn) = self
            .data
            .snapshot
            .current_turns
            .iter()
            .find(|turn| turn.id == turn_id)
        else {
            self.chat.branch_swipe.reset();
            return;
        };
        let ids = turn
            .responses
            .iter()
            .map(|response| response.id.clone())
            .collect::<Vec<_>>();
        let Some(current_id) = self
            .visible_response(turn)
            .map(|response| response.id.clone())
        else {
            self.chat.branch_swipe.reset();
            return;
        };
        let Some(neighbors) = swipe_neighbors(&ids, &current_id) else {
            self.chat.branch_swipe.reset();
            return;
        };
        if ids.len() < 2 {
            self.chat.branch_swipe.reset();
            return;
        }
        let action = self.chat.branch_swipe.update(
            BranchSwipeTarget::Assistant(turn_id.to_string()),
            delta_x,
            delta_y,
            phase,
            neighbors.availability(),
        );
        if self.chat.branch_swipe.captures_parent_scroll() {
            cx.stop_propagation();
        }
        let Some(action) = action else {
            return;
        };
        if let Some(destination) = neighbors.destination(action) {
            pressure_touch::feedback(Feedback::SelectionChanged);
            self.show_response(turn_id.to_string(), destination, cx);
        } else {
            pressure_touch::feedback(Feedback::Boundary);
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

    pub(crate) fn assistant_reasoning_editor(
        &self,
        response: &AssistantResponse,
        block_id: &str,
    ) -> Option<&AssistantTextEditor> {
        self.assistant_text_editor(response, AssistantTextKind::Reasoning, block_id)
    }

    pub(crate) fn assistant_output_editor(
        &self,
        response: &AssistantResponse,
        block_id: &str,
    ) -> Option<&AssistantTextEditor> {
        self.assistant_text_editor(response, AssistantTextKind::Output, block_id)
    }

    pub(crate) fn assistant_output_editing(&self, response: &AssistantResponse) -> bool {
        self.assistant_message_editor(response)
            .is_some_and(|editor| {
                editor
                    .text_editors
                    .iter()
                    .any(|editor| editor.kind == AssistantTextKind::Output)
            })
    }

    fn assistant_text_editor(
        &self,
        response: &AssistantResponse,
        kind: AssistantTextKind,
        block_id: &str,
    ) -> Option<&AssistantTextEditor> {
        self.assistant_message_editor(response)?
            .text_editors
            .iter()
            .find(|editor| editor.kind == kind && editor.block_id == block_id)
    }

    pub(crate) fn active_message_editor(&self) -> Option<Entity<TextareaState>> {
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
            .text_editors
            .iter()
            .filter(|text| text.kind == AssistantTextKind::Output)
            .collect::<Vec<_>>();
        let valid = outputs.is_empty()
            || outputs
                .iter()
                .any(|text| !text.input.read(cx).value().trim().is_empty());
        let changed = editor.text_editors.iter().any(|text| {
            assistant_text(response, text.kind, &text.block_id)
                != Some(text.input.read(cx).value().as_str())
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
        let input = cx.new(|cx| multiline_input(content, "Edit user message", 8, window, cx));
        cx.subscribe_in(&input, window, |_, _, event: &InputEvent, _, cx| {
            if matches!(event, InputEvent::Change) {
                cx.notify();
            }
        })
        .detach();
        self.chat.message_editor = Some(MessageEditor {
            target: MessageEditorTarget::User(turn_id),
            input,
            text_editors: Vec::new(),
            attachments,
            attachment_drafts: Vec::new(),
            attachment_previews: Default::default(),
            attachment_load_id: None,
        });
        self.navigation.pending_focus = Some(PendingFocus::MessageEditor);
        self.jump_to_message_editor(cx);
        cx.notify();
    }

    pub(crate) fn begin_edit_assistant_output(
        &mut self,
        response_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_begin_message_edit() {
            return;
        }
        let Some((_, response)) = self.response(&response_id) else {
            return;
        };
        let texts = if response.blocks.is_empty() {
            vec![(
                AssistantTextKind::Output,
                response.id.clone(),
                response.content.clone(),
            )]
        } else {
            response
                .blocks
                .iter()
                .filter_map(|block| match block {
                    AssistantBlock::Output { id, content } => {
                        Some((AssistantTextKind::Output, id.clone(), content.clone()))
                    }
                    _ => None,
                })
                .collect()
        };
        self.begin_assistant_text_edit(response_id, texts, window, cx);
    }

    pub(crate) fn begin_edit_assistant_reasoning(
        &mut self,
        response_id: String,
        block_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if !self.can_begin_message_edit() {
            return;
        }
        let Some((_, response)) = self.response(&response_id) else {
            return;
        };
        let content = if response.blocks.is_empty() && block_id == response.id {
            Some(response.thinking.clone())
        } else {
            response.blocks.iter().find_map(|block| match block {
                AssistantBlock::Reasoning { id, content, .. } if id == &block_id => {
                    Some(content.clone())
                }
                _ => None,
            })
        };
        let Some(content) = content else {
            return;
        };
        self.begin_assistant_text_edit(
            response_id,
            vec![(AssistantTextKind::Reasoning, block_id, content)],
            window,
            cx,
        );
    }

    fn can_begin_message_edit(&self) -> bool {
        !self.is_current_generating()
            && !self.recording_active()
            && self.chat.message_editor.is_none()
    }

    fn begin_assistant_text_edit(
        &mut self,
        response_id: String,
        texts: Vec<(AssistantTextKind, String, String)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if texts.is_empty() {
            return;
        }
        let mut text_editors = Vec::with_capacity(texts.len());
        for (kind, block_id, content) in texts {
            let placeholder = match kind {
                AssistantTextKind::Reasoning => "Edit assistant reasoning",
                AssistantTextKind::Output => "Edit assistant output",
            };
            let input = cx.new(|cx| {
                multiline_input(content, placeholder, CONTENT_EDITOR_MAX_ROWS, window, cx)
            });
            cx.subscribe_in(&input, window, |_, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            })
            .detach();
            text_editors.push(AssistantTextEditor {
                kind,
                block_id,
                input,
            });
        }
        let input = text_editors[0].input.clone();
        self.chat.message_editor = Some(MessageEditor {
            target: MessageEditorTarget::Assistant(response_id),
            input,
            text_editors,
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
        let mut reasoning = Vec::new();
        let mut outputs = Vec::new();
        for text in &editor.text_editors {
            let edit = (
                text.block_id.clone(),
                text.input.read(cx).value().to_string(),
            );
            match text.kind {
                AssistantTextKind::Reasoning => reasoning.push(edit),
                AssistantTextKind::Output => outputs.push(edit),
            }
        }
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
        response.replace_editable_text(&reasoning, &outputs);
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

fn assistant_text<'a>(
    response: &'a AssistantResponse,
    kind: AssistantTextKind,
    block_id: &str,
) -> Option<&'a str> {
    if response.blocks.is_empty() {
        return (block_id == response.id).then_some(match kind {
            AssistantTextKind::Reasoning => response.thinking.as_str(),
            AssistantTextKind::Output => response.content.as_str(),
        });
    }
    response
        .blocks
        .iter()
        .find_map(|block| match (kind, block) {
            (AssistantTextKind::Reasoning, AssistantBlock::Reasoning { id, content, .. })
                if id == block_id =>
            {
                Some(content.as_str())
            }
            (AssistantTextKind::Output, AssistantBlock::Output { id, content })
                if id == block_id =>
            {
                Some(content.as_str())
            }
            _ => None,
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn swipe_neighbors_follow_visual_order_without_wrapping() {
        let ids = ids(&["first", "second", "third"]);

        assert_eq!(
            swipe_neighbors(&ids, "first"),
            Some(SwipeNeighbors {
                previous: None,
                next: Some("second".into()),
            })
        );
        assert_eq!(
            swipe_neighbors(&ids, "second"),
            Some(SwipeNeighbors {
                previous: Some("first".into()),
                next: Some("third".into()),
            })
        );
        assert_eq!(
            swipe_neighbors(&ids, "third"),
            Some(SwipeNeighbors {
                previous: Some("second".into()),
                next: None,
            })
        );
    }

    #[test]
    fn swipe_neighbors_reject_unknown_current_item() {
        assert_eq!(swipe_neighbors(&ids(&["first"]), "missing"), None);
    }
}
