use std::collections::BTreeMap;

use gpui::{AppContext as _, Context, Window};
use gpui_component::{WindowExt as _, input::InputState};

use crate::{
    application::prompt::{PromptContext, render_prompt},
    desktop::{
        app::{DefaultModelRole, DestructiveAction, OneChat, PendingFocus},
        ui::settings::{
            PromptPresetEditor, PromptVariableEditor, PromptVariableKind, PromptVariableTestStatus,
        },
    },
    domain::{DEFAULT_TITLE_GENERATION_SYSTEM_PROMPT, PromptVariableSource, SystemPromptPreset},
};
use tokio_util::sync::CancellationToken;

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

        let updates_title_reasoning = role == DefaultModelRole::TitleGeneration
            || (role == DefaultModelRole::Primary
                && self
                    .data
                    .snapshot
                    .settings
                    .title_generation_model_id
                    .is_none());
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

        if updates_title_reasoning {
            let requested = self
                .data
                .snapshot
                .settings
                .title_generation_reasoning_preset
                .clone();
            self.data
                .snapshot
                .settings
                .title_generation_reasoning_preset = self
                .title_generation_model()
                .and_then(|model| model.reasoning.as_ref())
                .map(|reasoning| {
                    requested
                        .filter(|requested| {
                            reasoning
                                .preset_options()
                                .iter()
                                .any(|(id, _)| id == requested)
                        })
                        .unwrap_or_else(|| reasoning.default_preset().to_string())
                });
        }

        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn select_title_generation_reasoning_preset(
        &mut self,
        preset: String,
        cx: &mut Context<Self>,
    ) {
        let valid = self
            .title_generation_model()
            .and_then(|model| model.reasoning.as_ref())
            .is_some_and(|reasoning| {
                reasoning
                    .preset_options()
                    .iter()
                    .any(|(id, _)| id == &preset)
            });
        if !valid {
            return;
        }
        if self
            .data
            .snapshot
            .settings
            .title_generation_reasoning_preset
            .as_deref()
            == Some(&preset)
        {
            cx.notify();
            return;
        }
        self.data
            .snapshot
            .settings
            .title_generation_reasoning_preset = Some(preset);
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
        self.settings_ui.viewed_prompt_preset = None;
        self.settings_ui.prompt_preset_editor = None;
        self.settings_ui.form_error = None;
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

    pub(crate) fn begin_add_prompt_variable(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.begin_prompt_variable_edit(None, window, cx);
    }

    pub(crate) fn begin_edit_prompt_variable(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(source) = self
            .data
            .snapshot
            .settings
            .prompt_variables
            .get(&name)
            .cloned()
        else {
            return;
        };
        self.begin_prompt_variable_edit(Some((name, source)), window, cx);
    }

    fn begin_prompt_variable_edit(
        &mut self,
        variable: Option<(String, PromptVariableSource)>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.prompt_variable_test_revision = self
            .settings_ui
            .prompt_variable_test_revision
            .wrapping_add(1);
        self.settings_ui.prompt_variable_editor =
            Some(PromptVariableEditor::new(variable, window, cx));
        self.settings_ui.form_error = None;
        self.navigation.pending_focus = Some(PendingFocus::SettingsPrompt);
        let app = cx.entity();
        window.open_dialog(cx, move |dialog, window, cx| {
            crate::desktop::ui::settings::prompt_variable_dialog(dialog, app.clone(), window, cx)
        });
        cx.notify();
    }

    pub(crate) fn set_prompt_variable_kind(
        &mut self,
        kind: PromptVariableKind,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = self.settings_ui.prompt_variable_editor.as_mut() {
            editor.kind = kind;
            editor.test_status = None;
            self.settings_ui.form_error = None;
            cx.notify();
        }
    }

    pub(crate) fn toggle_prompt_variable_advanced(&mut self, cx: &mut Context<Self>) {
        if let Some(editor) = self.settings_ui.prompt_variable_editor.as_mut() {
            editor.advanced_expanded = !editor.advanced_expanded;
            cx.notify();
        }
    }

    pub(crate) fn toggle_prompt_builtins(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.prompt_builtins_expanded = !self.settings_ui.prompt_builtins_expanded;
        cx.notify();
    }

    pub(crate) fn test_prompt_variable_command(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = self.settings_ui.prompt_variable_editor.as_ref() else {
            return;
        };
        let source = match editor.source(cx) {
            Ok(PromptVariableSource::Command {
                script,
                cwd,
                timeout_ms,
            }) => PromptVariableSource::Command {
                script,
                cwd,
                timeout_ms,
            },
            Ok(_) => return,
            Err(error) => {
                self.settings_ui.form_error = Some(error);
                cx.notify();
                return;
            }
        };

        self.settings_ui.prompt_variable_test_revision = self
            .settings_ui
            .prompt_variable_test_revision
            .wrapping_add(1);
        let revision = self.settings_ui.prompt_variable_test_revision;
        if let Some(editor) = self.settings_ui.prompt_variable_editor.as_mut() {
            editor.test_status = Some(PromptVariableTestStatus::Running);
        }
        self.settings_ui.form_error = None;
        cx.notify();

        let variables = BTreeMap::from([("__test__".to_string(), source)]);
        let (sender, receiver) = async_channel::bounded(1);
        self.services.runtime.spawn(async move {
            let result = render_prompt(
                "{{__test__}}".into(),
                variables,
                PromptContext::default(),
                CancellationToken::new(),
            )
            .await;
            let _ = sender.send(result).await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                if this.settings_ui.prompt_variable_test_revision != revision {
                    return;
                }
                let Some(editor) = this.settings_ui.prompt_variable_editor.as_mut() else {
                    return;
                };
                editor.test_status = Some(match result {
                    Ok(Ok(snapshot)) => PromptVariableTestStatus::Succeeded {
                        duration_ms: snapshot
                            .variables
                            .first()
                            .map_or(0, |evaluation| evaluation.duration_ms),
                        output: snapshot.resolved,
                    },
                    Ok(Err(error)) => PromptVariableTestStatus::Failed(error.to_string()),
                    Err(_) => PromptVariableTestStatus::Failed(
                        "Command test task stopped unexpectedly.".into(),
                    ),
                });
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn cancel_prompt_variable_edit(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.prompt_variable_test_revision = self
            .settings_ui
            .prompt_variable_test_revision
            .wrapping_add(1);
        self.settings_ui.prompt_variable_editor = None;
        self.settings_ui.form_error = None;
        cx.notify();
    }

    pub(crate) fn save_prompt_variable(&mut self, cx: &mut Context<Self>) -> bool {
        let Some(editor) = self.settings_ui.prompt_variable_editor.as_ref() else {
            return false;
        };
        let (name, source) = match editor.build(cx) {
            Ok(variable) => variable,
            Err(error) => {
                self.settings_ui.form_error = Some(error);
                cx.notify();
                return false;
            }
        };
        let original_name = editor.original_name().map(str::to_string);
        if original_name.as_deref() != Some(&name)
            && self
                .data
                .snapshot
                .settings
                .prompt_variables
                .contains_key(&name)
        {
            self.settings_ui.form_error =
                Some(format!("A prompt variable named {name} already exists."));
            cx.notify();
            return false;
        }
        if let Some(original_name) = original_name {
            self.data
                .snapshot
                .settings
                .prompt_variables
                .remove(&original_name);
        }
        self.data
            .snapshot
            .settings
            .prompt_variables
            .insert(name, source);
        self.settings_ui.prompt_variable_editor = None;
        self.settings_ui.form_error = None;
        self.save_settings(cx);
        cx.notify();
        true
    }

    pub(crate) fn request_delete_prompt_variable(
        &mut self,
        name: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.settings_ui.prompt_variable_editor = None;
        self.request_destructive_action(
            DestructiveAction::DeletePromptVariable { name },
            window,
            cx,
        );
    }

    pub(crate) fn delete_prompt_variable(&mut self, name: String, cx: &mut Context<Self>) {
        if self
            .data
            .snapshot
            .settings
            .prompt_variables
            .remove(&name)
            .is_some()
        {
            self.save_settings(cx);
            cx.notify();
        }
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
