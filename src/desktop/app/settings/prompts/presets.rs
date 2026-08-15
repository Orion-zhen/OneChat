use super::*;
use crate::desktop::ui::settings::PromptPresetWorkspace;

impl OneChat {
    pub(crate) fn select_default_prompt(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        if name
            .as_deref()
            .is_some_and(|name| self.prompt_preset(name).is_none())
        {
            return;
        }
        if self.data.snapshot.settings.default_prompt_preset == name {
            cx.notify();
            return;
        }
        self.data.snapshot.settings.default_prompt_preset = name;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn view_prompt_preset(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(preset) = self.prompt_preset(&name).cloned() else {
            return;
        };
        let editor = PromptPresetEditor::new(Some(preset), window, cx);
        self.install_prompt_preset_workspace(PromptPresetWorkspace::view(editor), window, cx);
    }

    pub(crate) fn begin_add_prompt_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let editor = PromptPresetEditor::new(None, window, cx);
        self.install_prompt_preset_workspace(PromptPresetWorkspace::edit(editor), window, cx);
        self.navigation.pending_focus = Some(PendingFocus::SettingsPrompt);
        cx.notify();
    }

    pub(crate) fn begin_edit_prompt_preset(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(preset) = self.prompt_preset(&name).cloned() else {
            return;
        };
        let editor = PromptPresetEditor::new(Some(preset), window, cx);
        self.install_prompt_preset_workspace(PromptPresetWorkspace::edit(editor), window, cx);
        self.navigation.pending_focus = Some(PendingFocus::SettingsPrompt);
        cx.notify();
    }

    pub(crate) fn save_prompt_preset(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(workspace) = self.settings_ui.prompt_preset_workspace.as_ref() else {
            return false;
        };
        if !workspace.is_editing() {
            return false;
        }
        let editor = &workspace.editor;
        let preset = match editor.build(cx) {
            Ok(preset) => preset,
            Err(error) => {
                self.settings_ui.form_error = Some(error);
                cx.notify();
                return false;
            }
        };
        let original_name = editor.original_name().map(str::to_string);
        if original_name.as_deref() != Some(preset.name.as_str())
            && self.prompt_preset(&preset.name).is_some()
        {
            self.settings_ui.form_error = Some(format!(
                "A prompt preset named {} already exists.",
                preset.name
            ));
            cx.notify();
            return false;
        }
        let mut settings = self.data.snapshot.settings.clone();
        if let Some(original_name) = original_name.as_deref()
            && settings.default_prompt_preset.as_deref() == Some(original_name)
        {
            settings.default_prompt_preset = Some(preset.name.clone());
        }
        self.data.snapshot.settings = settings.clone();
        self.settings_ui.prompt_preset_workspace = None;
        self.settings_ui.pending_prompt_preset_exit = None;
        self.settings_ui.form_error = None;
        self.mutate_and_reload(
            move |storage| {
                if let Some(original_name) = original_name {
                    storage.update_prompt_preset(&original_name, &preset)?;
                } else {
                    storage.insert_prompt_preset(&preset)?;
                }
                storage.save_settings(&settings)
            },
            cx,
        );
        true
    }

    pub(crate) fn request_delete_prompt_preset(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.prompt_preset_workspace = None;
        self.settings_ui.pending_prompt_preset_exit = None;
        self.settings_ui.form_error = None;
        self.request_destructive_action(DestructiveAction::DeletePromptPreset { name }, window, cx);
    }

    pub(crate) fn delete_prompt_preset(&mut self, name: String, cx: &mut Context<Self>) {
        let mut settings = self.data.snapshot.settings.clone();
        if settings.default_prompt_preset.as_deref() == Some(&name) {
            settings.default_prompt_preset = None;
        }
        self.data.snapshot.settings = settings.clone();
        self.settings_ui.prompt_preset_workspace = None;
        self.settings_ui.pending_prompt_preset_exit = None;
        self.mutate_and_reload(
            move |storage| {
                storage.delete_prompt_preset(&name)?;
                storage.save_settings(&settings)
            },
            cx,
        );
    }

    pub(crate) fn reload_prompt_presets(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.prompt_preset_workspace = None;
        self.settings_ui.pending_prompt_preset_exit = None;
        self.reload_snapshot(cx);
    }
}
