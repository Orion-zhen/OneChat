use super::*;

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

    pub(crate) fn copy_assistant(&mut self, response_id: String, cx: &mut Context<Self>) {
        let Some(content) = self
            .response(&response_id)
            .map(|(_, response)| response.content.clone())
        else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(content));
    }

    pub(crate) fn assistant_message_editor(
        &self,
        response: &AssistantResponse,
    ) -> Option<Entity<Composer>> {
        self.chat
            .message_editor
            .as_ref()
            .filter(|editor| editor.message_id == response.id)
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn active_message_editor(&self) -> Option<Entity<Composer>> {
        self.chat
            .message_editor
            .as_ref()
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn begin_edit_assistant(&mut self, response_id: String, cx: &mut Context<Self>) {
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
        let input = cx.new(|cx| Composer::multiline(content, "Edit assistant response", cx));
        cx.subscribe(&input, |this, _, event, cx| {
            if matches!(event, ComposerEvent::Cancel) {
                this.cancel_assistant_edit(cx);
            }
        })
        .detach();
        self.chat.message_editor = Some(MessageEditor {
            message_id: response_id,
            input,
        });
        self.navigation.pending_focus = Some(PendingFocus::MessageEditor);
        cx.notify();
    }

    pub(crate) fn cancel_assistant_edit(&mut self, cx: &mut Context<Self>) {
        self.chat.message_editor = None;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn save_assistant_edit(&mut self, response_id: String, cx: &mut Context<Self>) {
        let Some(editor) = self
            .chat
            .message_editor
            .as_ref()
            .filter(|editor| editor.message_id == response_id)
        else {
            return;
        };
        let content = editor.input.read(cx).text().to_string();
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
        response.content = content;
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
        let measured_height = from_height + f32::from(scroll.max_offset().height);
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
        let delta = event.delta.pixel_delta(window.line_height()).y;
        let distance =
            self.chat.message_scroll.max_offset().height + self.chat.message_scroll.offset().y;
        self.chat.follow_latest = follow_after_scroll(
            self.chat.follow_latest,
            f32::from(delta),
            f32::from(distance),
        );
        cx.notify();
    }

    pub(crate) fn jump_to_latest(&mut self, cx: &mut Context<Self>) {
        self.chat.follow_latest = true;
        self.chat.message_scroll.scroll_to_bottom();
        cx.notify();
    }
}
