use super::*;

impl OneChat {
    pub(crate) fn copy_assistant(&mut self, message_id: String, cx: &mut Context<Self>) {
        let Some(content) = self
            .data
            .snapshot
            .current_messages
            .iter()
            .find(|message| message.id == message_id && message.role == MessageRole::Assistant)
            .map(|message| message.content.clone())
        else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(content));
    }

    pub(crate) fn assistant_message_editor(&self, message: &Message) -> Option<Entity<Composer>> {
        self.chat
            .message_editor
            .as_ref()
            .filter(|editor| editor.message_id == message.id)
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn active_message_editor(&self) -> Option<Entity<Composer>> {
        self.chat
            .message_editor
            .as_ref()
            .map(|editor| editor.input.clone())
    }

    pub(crate) fn begin_edit_assistant(&mut self, message_id: String, cx: &mut Context<Self>) {
        if self.is_current_generating() || self.chat.message_editor.is_some() {
            return;
        }
        let Some(content) = self
            .data
            .snapshot
            .current_messages
            .iter()
            .find(|message| message.id == message_id && message.role == MessageRole::Assistant)
            .map(|message| message.content.clone())
        else {
            return;
        };
        let input = cx.new(|cx| Composer::multiline(content, "Edit assistant response", cx));
        cx.subscribe(&input, |this, _, event, cx| {
            if matches!(event, ComposerEvent::Cancel) {
                this.cancel_assistant_edit(cx);
            }
        })
        .detach();
        self.chat.message_editor = Some(MessageEditor { message_id, input });
        self.navigation.pending_focus = Some(PendingFocus::MessageEditor);
        cx.notify();
    }

    pub(crate) fn cancel_assistant_edit(&mut self, cx: &mut Context<Self>) {
        self.chat.message_editor = None;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn save_assistant_edit(&mut self, message_id: String, cx: &mut Context<Self>) {
        let Some(editor) = self
            .chat
            .message_editor
            .as_ref()
            .filter(|editor| editor.message_id == message_id)
        else {
            return;
        };
        let content = editor.input.read(cx).text().to_string();
        let Some(mut message) = self
            .data
            .snapshot
            .current_messages
            .iter()
            .find(|message| message.id == message_id && message.role == MessageRole::Assistant)
            .cloned()
        else {
            return;
        };
        message.content = content;
        message.updated_at = now_timestamp();
        self.chat.message_editor = None;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(move |storage| storage.update_message(&message), cx);
    }

    pub(crate) fn inspect_message_request(&mut self, message_id: String, cx: &mut Context<Self>) {
        let request_id = self
            .data
            .snapshot
            .current_messages
            .iter()
            .find(|message| message.id == message_id)
            .and_then(|message| message.request_id.clone());
        if let Some(request_id) = request_id {
            self.chat.selected_request_id = Some(request_id);
            self.navigation.inspector_open = true;
            self.navigation.inspector_tab = InspectorTab::Info;
            cx.notify();
        }
    }

    pub(crate) fn error_detail_expanded(&self, message_id: &str) -> bool {
        self.chat.expanded_error_ids.contains(message_id)
    }

    pub(crate) fn toggle_error_detail(&mut self, message_id: String, cx: &mut Context<Self>) {
        if !self.chat.expanded_error_ids.remove(&message_id) {
            self.chat.expanded_error_ids.insert(message_id);
        }
        cx.notify();
    }

    pub(crate) fn thinking_expanded(&self, message_id: &str) -> bool {
        self.chat.expanded_thinking_ids.contains(message_id)
    }

    pub(crate) fn toggle_thinking(&mut self, message_id: String, cx: &mut Context<Self>) {
        if !self.chat.expanded_thinking_ids.remove(&message_id) {
            self.chat.expanded_thinking_ids.insert(message_id);
        }
        cx.notify();
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

    pub(crate) fn open_inspector(&mut self, tab: InspectorTab, cx: &mut Context<Self>) {
        self.navigation.inspector_open = true;
        self.navigation.inspector_tab = tab;
        if tab == InspectorTab::Model {
            self.sync_generation_config_editor(cx);
        }
        cx.notify();
    }
}
