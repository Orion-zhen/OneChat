use super::*;

impl OneChat {
    pub(crate) fn begin_edit_translation_prompts(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let settings = self.settings();
        let system_prompt = settings.translation_system_prompt.clone();
        let user_prompt = settings.translation_user_prompt.clone();
        self.settings_ui.translation_system_prompt_editor = Some(cx.new(|cx| {
            TextareaState::new(window, cx)
                .soft_wrap(true)
                .default_value(system_prompt)
                .placeholder("Default system instructions for translation")
        }));
        self.settings_ui.translation_user_prompt_editor = Some(cx.new(|cx| {
            TextareaState::new(window, cx)
                .soft_wrap(true)
                .default_value(user_prompt)
                .placeholder("Default user message for translation")
        }));
        self.navigation.pending_focus = Some(PendingFocus::SettingsPrompt);
        cx.notify();
    }

    pub(crate) fn cancel_translation_prompt_edit(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.translation_system_prompt_editor = None;
        self.settings_ui.translation_user_prompt_editor = None;
        cx.notify();
    }

    pub(crate) fn save_translation_prompt_defaults(&mut self, cx: &mut Context<Self>) {
        let (Some(system_editor), Some(user_editor)) = (
            self.settings_ui.translation_system_prompt_editor.as_ref(),
            self.settings_ui.translation_user_prompt_editor.as_ref(),
        ) else {
            return;
        };
        let system_prompt = system_editor.read(cx).value().trim().to_string();
        let user_prompt = user_editor.read(cx).value().trim().to_string();
        if !crate::desktop::app::translate::prompts_include_text(&system_prompt, &user_prompt) {
            self.data.error = Some("A translation prompt must include {{text}}.".into());
            cx.notify();
            return;
        }

        let update_current = self.translation.uses_default_prompts(
            &self.settings().translation_system_prompt,
            &self.settings().translation_user_prompt,
        );
        self.data.snapshot.settings.translation_system_prompt = system_prompt.clone();
        self.data.snapshot.settings.translation_user_prompt = user_prompt.clone();
        if update_current {
            self.set_translation_prompts(system_prompt, user_prompt, cx);
        }
        self.cancel_translation_prompt_edit(cx);
        self.save_settings(cx);
    }

    pub(crate) fn reset_translation_prompt_defaults(&mut self, cx: &mut Context<Self>) {
        let update_current = self.translation.uses_default_prompts(
            &self.settings().translation_system_prompt,
            &self.settings().translation_user_prompt,
        );
        self.data.snapshot.settings.translation_system_prompt =
            DEFAULT_TRANSLATION_SYSTEM_PROMPT.into();
        self.data.snapshot.settings.translation_user_prompt =
            DEFAULT_TRANSLATION_USER_PROMPT.into();
        if update_current {
            self.set_translation_prompts(
                DEFAULT_TRANSLATION_SYSTEM_PROMPT.into(),
                DEFAULT_TRANSLATION_USER_PROMPT.into(),
                cx,
            );
        }
        self.cancel_translation_prompt_edit(cx);
        self.save_settings(cx);
    }
}
