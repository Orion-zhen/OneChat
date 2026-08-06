use super::*;

impl OneChat {
    pub(crate) fn cycle_theme(&mut self, cx: &mut Context<Self>) {
        self.data.snapshot.settings.theme = self.data.snapshot.settings.theme.next();
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn expand_system_prompt(&mut self, cx: &mut Context<Self>) {
        self.chat.system_prompt_mode = SystemPromptMode::Expanded;
        cx.notify();
    }

    pub(crate) fn collapse_system_prompt(&mut self, cx: &mut Context<Self>) {
        self.chat.system_prompt_mode = SystemPromptMode::Compact;
        cx.notify();
    }

    pub(crate) fn begin_edit_system_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(conversation) = self.current_conversation() else {
            return;
        };
        let editor = cx.new(|cx| {
            Composer::multiline(
                conversation.system_prompt.content.clone(),
                "Describe how the assistant should respond",
                cx,
            )
        });
        cx.subscribe(&editor, |this, _, event, cx| {
            if matches!(event, ComposerEvent::Cancel) {
                this.cancel_system_prompt_edit(cx);
            }
        })
        .detach();
        self.chat.system_prompt_editor = Some(editor);
        self.chat.system_prompt_mode = SystemPromptMode::Editing;
        self.navigation.pending_focus = Some(PendingFocus::SystemPrompt);
        cx.notify();
    }

    pub(crate) fn cancel_system_prompt_edit(&mut self, cx: &mut Context<Self>) {
        self.chat.system_prompt_editor = None;
        self.chat.system_prompt_mode = SystemPromptMode::Compact;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        cx.notify();
    }

    pub(crate) fn save_system_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.chat.system_prompt_editor.as_ref() else {
            return;
        };
        let content = editor.read(cx).text().trim().to_string();
        let Some(mut conversation) = self.current_conversation().cloned() else {
            return;
        };
        conversation.system_prompt.content = content;
        conversation.system_prompt.source = SystemPromptSource::Custom;
        conversation.updated_at = now_timestamp();
        self.chat.system_prompt_editor = None;
        self.chat.system_prompt_mode = SystemPromptMode::Compact;
        self.navigation.pending_focus = Some(PendingFocus::Composer);
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn copy_system_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(content) = self
            .current_conversation()
            .map(|conversation| conversation.system_prompt.content.clone())
            .filter(|content| !content.trim().is_empty())
        else {
            return;
        };
        cx.write_to_clipboard(ClipboardItem::new_string(content));
    }

    pub(crate) fn begin_edit_default_system_prompt(&mut self, cx: &mut Context<Self>) {
        let editor = cx.new(|cx| {
            Composer::multiline(
                self.data.snapshot.settings.default_system_prompt.clone(),
                "Copied into each new conversation",
                cx,
            )
        });
        cx.subscribe(&editor, |this, _, event, cx| {
            if matches!(event, ComposerEvent::Cancel) {
                this.cancel_default_system_prompt_edit(cx);
            }
        })
        .detach();
        self.settings_ui.default_system_prompt_editor = Some(editor);
        self.navigation.pending_focus = Some(PendingFocus::DefaultSystemPrompt);
        cx.notify();
    }

    pub(crate) fn cancel_default_system_prompt_edit(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.default_system_prompt_editor = None;
        cx.notify();
    }

    pub(crate) fn save_default_system_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.settings_ui.default_system_prompt_editor.as_ref() else {
            return;
        };
        self.data.snapshot.settings.default_system_prompt =
            editor.read(cx).text().trim().to_string();
        self.settings_ui.default_system_prompt_editor = None;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn save_generation_config(&mut self, cx: &mut Context<Self>) {
        let Some(mut conversation) = self.current_conversation().cloned() else {
            return;
        };
        let Some(editor) = self.chat.generation_config_editor.as_ref() else {
            return;
        };
        let config = match editor.build(&conversation.generation_config, cx) {
            Ok(config) => config,
            Err(error) => {
                self.chat.parameter_error = Some(error);
                cx.notify();
                return;
            }
        };
        conversation.generation_config = config;
        conversation.updated_at = now_timestamp();
        self.chat.parameter_error = None;
        self.mutate_and_reload(
            move |storage| storage.update_conversation(&conversation),
            cx,
        );
    }

    pub(crate) fn request_clear_current_context(&mut self, cx: &mut Context<Self>) {
        let Some(conversation_id) = self.current_conversation().map(|value| value.id.clone())
        else {
            return;
        };
        if self.chat.generations.is_active(&conversation_id) {
            self.data.error = Some("Stop the active generation before clearing context.".into());
            cx.notify();
            return;
        }
        self.overlays.destructive_action =
            Some(DestructiveAction::ClearContext { conversation_id });
        self.navigation.pending_focus = Some(PendingFocus::Root);
        cx.notify();
    }

    pub(super) fn clear_current_context(
        &mut self,
        conversation_id: String,
        cx: &mut Context<Self>,
    ) {
        self.mutate_and_reload(
            move |storage| storage.clear_conversation_context(&conversation_id),
            cx,
        );
    }
}
