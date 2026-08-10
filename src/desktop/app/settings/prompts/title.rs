use super::*;

impl OneChat {
    pub(crate) fn begin_edit_title_prompt(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let value = self
            .data
            .snapshot
            .settings
            .title_generation_system_prompt
            .clone();
        self.settings_ui.title_prompt_editor = Some(cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .soft_wrap(true)
                .default_value(value)
                .placeholder("Used to generate automatic conversation titles")
        }));
        self.navigation.pending_focus = Some(PendingFocus::SettingsPrompt);
        cx.notify();
    }

    pub(crate) fn cancel_title_prompt_edit(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.title_prompt_editor = None;
        cx.notify();
    }

    pub(crate) fn save_title_prompt(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.settings_ui.title_prompt_editor.as_ref() else {
            return;
        };
        let content = editor.read(cx).value().trim().to_string();
        if content.is_empty() {
            self.data.error = Some("The title generation prompt cannot be empty.".into());
            cx.notify();
            return;
        }
        self.data.snapshot.settings.title_generation_system_prompt = content;
        self.settings_ui.title_prompt_editor = None;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn reset_title_generation_prompt(&mut self, cx: &mut Context<Self>) {
        self.data.snapshot.settings.title_generation_system_prompt =
            DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT.into();
        self.settings_ui.title_prompt_editor = None;
        self.save_settings(cx);
        cx.notify();
    }
}
