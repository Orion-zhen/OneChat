use super::*;

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
        if self.chat.message_editor.is_some() {
            return;
        }
        let Some(response) = self
            .response(&response_id)
            .map(|(_, response)| response.clone())
        else {
            return;
        };
        if response.status != MessageStatus::Completed || response.content.is_empty() {
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

    pub(crate) fn copy_user(&mut self, turn_id: String, cx: &mut Context<Self>) {
        let Some(content) = self
            .data
            .snapshot
            .current_turns
            .iter()
            .find(|turn| turn.id == turn_id)
            .map(|turn| turn.user.content.clone())
        else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(content));
    }

    pub(crate) fn copy_assistant(&mut self, response_id: String, cx: &mut Context<Self>) {
        let Some(content) = self
            .response(&response_id)
            .map(|(_, response)| response.content.clone())
        else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(content));
    }

    pub(crate) fn user_message_editor(&self, turn: &Turn) -> Option<Entity<InputState>> {
        self.chat
            .message_editor
            .as_ref()
            .filter(
                |editor| matches!(&editor.target, MessageEditorTarget::User(id) if id == &turn.id),
            )
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn assistant_message_editor(
        &self,
        response: &AssistantResponse,
    ) -> Option<Entity<InputState>> {
        self.chat
            .message_editor
            .as_ref()
            .filter(|editor| {
                matches!(&editor.target, MessageEditorTarget::Assistant(id) if id == &response.id)
            })
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn active_message_editor(&self) -> Option<Entity<InputState>> {
        self.chat
            .message_editor
            .as_ref()
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn begin_edit_user(
        &mut self,
        turn_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_current_generating() || self.chat.message_editor.is_some() {
            return;
        }
        let Some(content) = self
            .data
            .snapshot
            .current_turns
            .iter()
            .find(|turn| turn.id == turn_id)
            .map(|turn| turn.user.content.clone())
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
        });
        self.navigation.pending_focus = Some(PendingFocus::MessageEditor);
        cx.notify();
    }

    pub(crate) fn begin_edit_assistant(
        &mut self,
        response_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.is_current_generating() || self.chat.message_editor.is_some() {
            return;
        }
        let Some((turn, response)) = self.response(&response_id) else {
            return;
        };
        if !self.is_latest_turn(&turn.id) {
            self.data.error = Some("Only responses in the latest turn can be edited.".into());
            cx.notify();
            return;
        }
        let content = response.content.clone();
        let input = cx.new(|cx| {
            multiline_input(content, "Edit assistant response", window, cx)
                .auto_grow(1, ASSISTANT_EDITOR_MAX_ROWS)
        });
        self.chat.message_editor = Some(MessageEditor {
            target: MessageEditorTarget::Assistant(response_id),
            input,
        });
        self.navigation.pending_focus = Some(PendingFocus::MessageEditor);
        cx.notify();
    }

    pub(crate) fn cancel_message_edit(&mut self, cx: &mut Context<Self>) {
        self.chat.message_editor = None;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn save_user_edit(&mut self, turn_id: String, cx: &mut Context<Self>) {
        let Some(editor) = self.chat.message_editor.as_ref().filter(
            |editor| matches!(&editor.target, MessageEditorTarget::User(id) if id == &turn_id),
        ) else {
            return;
        };
        let content = editor.input.read(cx).value().trim().to_string();
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
        if content.is_empty() && turn.user.attachments.is_empty() {
            self.data.error = Some("User messages cannot be empty.".into());
            cx.notify();
            return;
        }
        if content == turn.user.content {
            self.cancel_message_edit(cx);
            return;
        }
        let (conversation, provider, model) = match self.generation_target(None) {
            Ok(target) => target,
            Err(error) => {
                self.data.error = Some(error);
                cx.notify();
                return;
            }
        };
        if !model.capabilities.vision
            && turn
                .user
                .attachments
                .iter()
                .any(|attachment| attachment.kind != AttachmentKind::Text)
        {
            self.data.error = Some(
                "The selected model cannot use the image or PDF attachments on this message."
                    .into(),
            );
            cx.notify();
            return;
        }
        let storage = self.services.storage.clone();
        let conversation_id = conversation.id.clone();
        let user_message = |user: &crate::domain::UserMessage| {
            storage
                .message_for_user(&conversation_id, user)
                .map_err(|error| error.to_string())
        };
        let prepared = match PreparedGeneration::new(
            &conversation,
            &provider,
            &model,
            &self.data.snapshot.current_turns,
            turn.parent_response_id,
            crate::domain::UserMessage::new(content, turn.user.attachments),
            &user_message,
        ) {
            Ok(prepared) => prepared,
            Err(error) => {
                self.data.error = Some(format!("Could not load attachments: {error}"));
                cx.notify();
                return;
            }
        };
        self.chat.message_editor = None;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        self.begin_prepared_generation(prepared, cx);
    }

    pub(crate) fn select_user_branch(&mut self, turn_id: String, cx: &mut Context<Self>) {
        if self.is_current_generating() || self.chat.message_editor.is_some() {
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
        let Some(editor) = self.chat.message_editor.as_ref().filter(|editor| {
            matches!(&editor.target, MessageEditorTarget::Assistant(id) if id == &response_id)
        }) else {
            return;
        };
        let content = editor.input.read(cx).value().to_string();
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
        response.replace_content(content);
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

    pub(crate) fn error_detail_expanded(&self, response_id: &str) -> bool {
        self.chat.expanded_error_ids.contains(response_id)
    }

    pub(crate) fn toggle_error_detail(&mut self, response_id: String, cx: &mut Context<Self>) {
        if !self.chat.expanded_error_ids.remove(&response_id) {
            self.chat.expanded_error_ids.insert(response_id);
        }
        cx.notify();
    }

    pub(crate) fn tool_execution_expanded(&self, execution_id: &str) -> bool {
        self.chat.expanded_tool_execution_ids.contains(execution_id)
    }

    pub(crate) fn toggle_tool_execution(&mut self, execution_id: String, cx: &mut Context<Self>) {
        if !self.chat.expanded_tool_execution_ids.remove(&execution_id) {
            self.chat.expanded_tool_execution_ids.insert(execution_id);
        }
        cx.notify();
    }

    pub(crate) fn thinking_expanded(&self, response_id: &str, default_expanded: bool) -> bool {
        default_expanded != self.chat.thinking_expansion_overrides.contains(response_id)
    }

    pub(crate) fn toggle_thinking(
        &mut self,
        response_id: String,
        default_expanded: bool,
        cx: &mut Context<Self>,
    ) {
        let expanding = !self.thinking_expanded(&response_id, default_expanded);
        if !self.chat.thinking_expansion_overrides.remove(&response_id) {
            self.chat
                .thinking_expansion_overrides
                .insert(response_id.clone());
        }
        self.capture_thinking_motion(response_id, !expanding);
        cx.notify();
    }

    pub(crate) fn finish_thinking(&mut self, response_id: String) {
        self.chat.thinking_expansion_overrides.remove(&response_id);
        self.capture_thinking_motion(response_id, true);
    }

    fn capture_thinking_motion(&mut self, response_id: String, scroll_to_bottom: bool) {
        let Some(scroll) = self.chat.thinking_scrolls.get(&response_id) else {
            return;
        };
        let from_height = f32::from(scroll.bounds().size.height);
        let measured_height = from_height + f32::from(scroll.max_offset().y);
        self.chat.thinking_motions.insert(
            response_id,
            ThinkingMotion {
                from_height: if from_height > 0.0 {
                    from_height
                } else {
                    COLLAPSED_THINKING_HEIGHT
                },
                full_height: if measured_height > 0.0 {
                    measured_height
                } else {
                    COLLAPSED_THINKING_HEIGHT
                },
            },
        );
        if scroll_to_bottom {
            scroll.scroll_to_bottom();
        }
    }

    pub(crate) fn on_message_scroll(
        &mut self,
        event: &ScrollWheelEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.chat.message_scroll_motion.cancel();
        let delta = event.delta.pixel_delta(window.line_height()).y;
        let distance =
            self.chat.message_scroll.max_offset().y + self.chat.message_scroll.offset().y;
        self.chat.follow_latest = follow_after_scroll(
            self.chat.follow_latest,
            f32::from(delta),
            f32::from(distance),
        );
        cx.notify();
    }

    pub(crate) fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        let from = f32::from(self.chat.message_scroll.offset().y);
        let target = -f32::from(self.chat.message_scroll.max_offset().y);
        if (target - from).abs() < 1.0 {
            self.chat.follow_latest = true;
            self.chat.message_scroll.scroll_to_bottom();
        } else {
            self.chat.message_scroll_motion.start(from);
        }
        cx.notify();
    }

    pub(crate) fn advance_message_scroll(&mut self, window: &mut Window) {
        let target = -f32::from(self.chat.message_scroll.max_offset().y);
        let Some((offset_y, finished)) = self.chat.message_scroll_motion.offset(target, window)
        else {
            return;
        };

        let mut offset = self.chat.message_scroll.offset();
        offset.y = gpui::px(offset_y);
        self.chat.message_scroll.set_offset(offset);
        if finished {
            self.chat.follow_latest = true;
            self.chat.message_scroll.scroll_to_bottom();
        }
    }
}
