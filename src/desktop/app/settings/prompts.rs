use gpui::{AppContext as _, Context, Window};
use gpui_component::{WindowExt as _, input::InputState};

use crate::{
    desktop::{
        app::{DefaultModelRole, DestructiveAction, OneChat, PendingFocus},
        ui::settings::PromptPresetEditor,
    },
    domain::{DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT, SystemPromptPreset},
};

impl OneChat {
    pub(crate) fn select_default_model(
        &mut self,
        role: DefaultModelRole,
        model_id: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if let Some(model_id) = model_id.as_deref() {
            let Some(model) = self
                .data
                .snapshot
                .models
                .iter()
                .find(|model| model.id == model_id)
            else {
                return;
            };
            if let Err(reason) = self.model_availability(model) {
                self.data.error = Some(format!("Model is unavailable: {reason}."));
                cx.notify();
                return;
            }
        } else if role == DefaultModelRole::Primary {
            return;
        }

        let stored_id = match role {
            DefaultModelRole::Primary => &mut self.data.snapshot.settings.primary_model_id,
            DefaultModelRole::TitleGeneration => {
                &mut self.data.snapshot.settings.title_generation_model_id
            }
        };
        if *stored_id == model_id {
            cx.notify();
            return;
        }
        *stored_id = model_id;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn select_default_prompt(&mut self, name: Option<String>, cx: &mut Context<Self>) {
        if name
            .as_deref()
            .is_some_and(|name| self.prompt_preset(name).is_none())
        {
            return;
        }
        if self.data.snapshot.settings.default_system_prompt_preset == name {
            cx.notify();
            return;
        }
        self.data.snapshot.settings.default_system_prompt_preset = name;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn view_prompt_preset(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.prompt_preset(&name).is_none() {
            return;
        }
        self.settings_ui.viewed_prompt_preset = Some(name);
        self.open_prompt_preset_dialog(window, cx);
        cx.notify();
    }

    pub(crate) fn begin_add_prompt_preset(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_prompt_preset_edit(None, window, cx);
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
        self.begin_prompt_preset_edit(Some(preset), window, cx);
    }

    fn begin_prompt_preset_edit(
        &mut self,
        preset: Option<SystemPromptPreset>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let editor = PromptPresetEditor::new(preset, window, cx);
        self.settings_ui.viewed_prompt_preset = None;
        self.settings_ui.prompt_preset_editor = Some(editor);
        self.settings_ui.form_error = None;
        self.navigation.pending_focus = Some(PendingFocus::SettingsPrompt);
        self.open_prompt_preset_dialog(window, cx);
        cx.notify();
    }

    fn open_prompt_preset_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.chat.text_selection.clear(window);
        let app = cx.entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            crate::desktop::ui::settings::prompt_preset_dialog(dialog, app.clone(), window, cx)
        });
    }

    pub(crate) fn cancel_prompt_preset_edit(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.prompt_preset_editor = None;
        self.settings_ui.form_error = None;
        cx.notify();
    }

    pub(crate) fn save_prompt_preset(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(editor) = self.settings_ui.prompt_preset_editor.as_ref() else {
            return false;
        };
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
            && settings.default_system_prompt_preset.as_deref() == Some(original_name)
        {
            settings.default_system_prompt_preset = Some(preset.name.clone());
        }
        self.data.snapshot.settings = settings.clone();
        self.settings_ui.prompt_preset_editor = None;
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
        self.request_destructive_action(DestructiveAction::DeletePromptPreset { name }, window, cx);
    }

    pub(crate) fn delete_prompt_preset(&mut self, name: String, cx: &mut Context<Self>) {
        let mut settings = self.data.snapshot.settings.clone();
        if settings.default_system_prompt_preset.as_deref() == Some(&name) {
            settings.default_system_prompt_preset = None;
        }
        self.data.snapshot.settings = settings.clone();
        self.settings_ui.viewed_prompt_preset = None;
        self.settings_ui.prompt_preset_editor = None;
        self.mutate_and_reload(
            move |storage| {
                storage.delete_prompt_preset(&name)?;
                storage.save_settings(&settings)
            },
            cx,
        );
    }

    pub(crate) fn reload_prompt_presets(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.viewed_prompt_preset = None;
        self.reload_snapshot(cx);
    }

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
