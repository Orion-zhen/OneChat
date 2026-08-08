use super::*;

impl OneChat {
    pub(crate) fn update_background_opacity(&mut self, opacity: f32, cx: &mut Context<Self>) {
        let opacity = rounded_background_opacity(opacity);
        if (self.data.snapshot.settings.background_opacity - opacity).abs() < f32::EPSILON {
            return;
        }
        self.data.snapshot.settings.background_opacity = opacity;
        cx.notify();
    }

    pub(crate) fn update_message_width_ratio(&mut self, ratio: f32, cx: &mut Context<Self>) {
        let ratio = rounded_message_width_ratio(ratio);
        if (self.data.snapshot.settings.message_width_ratio - ratio).abs() < f32::EPSILON {
            return;
        }
        self.data.snapshot.settings.message_width_ratio = ratio;
        cx.notify();
    }

    pub(crate) fn toggle_auto_title_enabled(&mut self, cx: &mut Context<Self>) {
        self.data.snapshot.settings.auto_title_enabled =
            !self.data.snapshot.settings.auto_title_enabled;
        self.save_settings(cx);
        cx.notify();
    }

    pub(crate) fn select_settings_section(
        &mut self,
        section: SettingsSection,
        cx: &mut Context<Self>,
    ) {
        let reload_prompts = section == SettingsSection::SystemPrompts;
        if self.settings_ui.section == section {
            if reload_prompts {
                self.reload_snapshot(cx);
            }
            return;
        }
        self.settings_ui.section = section;
        self.settings_ui.viewed_prompt_preset = None;
        self.settings_ui.provider_editor = None;
        self.settings_ui.model_editor = None;
        self.settings_ui.prompt_preset_editor = None;
        self.settings_ui.title_prompt_editor = None;
        self.settings_ui.form_error = None;
        if reload_prompts {
            self.reload_snapshot(cx);
        }
        cx.notify();
    }

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

    pub(crate) fn begin_add_provider(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.settings_ui.section = SettingsSection::NewProvider;
        self.install_provider_editor(ProviderEditor::new(None, window, cx), window, cx);
        self.settings_ui.model_editor = None;
        self.settings_ui.form_error = None;
        cx.notify();
    }

    pub(crate) fn begin_edit_provider(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let provider = self
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == id)
            .cloned();
        if let Some(provider) = provider {
            self.settings_ui.section = SettingsSection::Provider(provider.id.clone());
            self.install_provider_editor(
                ProviderEditor::new(Some(provider), window, cx),
                window,
                cx,
            );
            self.settings_ui.model_editor = None;
            self.settings_ui.form_error = None;
            cx.notify();
        }
    }

    fn install_provider_editor(
        &mut self,
        editor: ProviderEditor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let kind = editor.kind.clone();
        self.settings_ui.provider_editor = Some(editor);
        cx.subscribe_in(
            &kind,
            window,
            |this,
             _,
             event: &SelectEvent<Vec<crate::desktop::ui::settings::ProviderKindItem>>,
             window,
             cx| {
                let SelectEvent::Confirm(Some(kind)) = event else {
                    return;
                };
                if let Some(editor) = &mut this.settings_ui.provider_editor {
                    editor.select_kind(*kind, window, cx);
                    cx.notify();
                }
            },
        )
        .detach();
    }

    pub(crate) fn set_provider_enabled(
        &mut self,
        id: String,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(mut provider) = self
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == id)
            .cloned()
        else {
            return;
        };
        if provider.enabled == enabled {
            return;
        }
        provider.enabled = enabled;
        provider.updated_at = now_timestamp();
        self.mutate_and_reload(move |storage| storage.update_provider(&provider), cx);
    }

    pub(crate) fn save_provider(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.settings_ui.provider_editor else {
            return;
        };
        let provider = match editor.build(cx) {
            Ok(provider) => provider,
            Err(error) => {
                self.settings_ui.form_error = Some(error);
                cx.notify();
                return;
            }
        };
        let insert = editor.is_new();
        self.settings_ui.section = SettingsSection::Provider(provider.id.clone());
        self.settings_ui.provider_editor = None;
        self.settings_ui.form_error = None;
        self.mutate_and_reload(
            move |storage| {
                if insert {
                    storage.insert_provider(&provider)
                } else {
                    storage.update_provider(&provider)
                }
            },
            cx,
        );
    }

    pub(crate) fn cancel_provider_editor(&mut self, cx: &mut Context<Self>) {
        if self.settings_ui.section == SettingsSection::NewProvider {
            self.settings_ui.section = SettingsSection::General;
        }
        self.settings_ui.provider_editor = None;
        self.settings_ui.form_error = None;
        cx.notify();
    }

    pub(crate) fn request_delete_provider(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_destructive_action(DestructiveAction::DeleteProvider { id }, window, cx);
    }

    pub(super) fn delete_provider(&mut self, id: String, cx: &mut Context<Self>) {
        self.settings_ui.connection_tests.remove(&id);
        if self.settings_ui.section == SettingsSection::Provider(id.clone()) {
            self.settings_ui.section = SettingsSection::General;
            self.settings_ui.provider_editor = None;
            self.settings_ui.model_editor = None;
        }
        self.mutate_and_reload(move |storage| storage.delete_provider(&id), cx);
    }

    pub(crate) fn begin_add_model(
        &mut self,
        provider_id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(provider_kind) = self
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .map(|provider| provider.kind)
        else {
            self.settings_ui.form_error = Some("Provider not found.".into());
            cx.notify();
            return;
        };
        self.settings_ui.section = SettingsSection::Provider(provider_id.clone());
        self.settings_ui.provider_editor = None;
        self.install_model_editor(
            ModelEditor::new(provider_id.clone(), provider_kind, None, window, cx),
            window,
            cx,
        );
        self.settings_ui.form_error = None;
        self.fetch_available_models(provider_id, cx);
        cx.notify();
    }

    pub(crate) fn begin_edit_model(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let model = self
            .data
            .snapshot
            .models
            .iter()
            .find(|model| model.id == id)
            .cloned();
        if let Some(model) = model {
            let Some(provider_kind) = self
                .data
                .snapshot
                .providers
                .iter()
                .find(|provider| provider.id == model.provider_id)
                .map(|provider| provider.kind)
            else {
                self.settings_ui.form_error = Some("Provider not found.".into());
                cx.notify();
                return;
            };
            self.settings_ui.section = SettingsSection::Provider(model.provider_id.clone());
            self.settings_ui.provider_editor = None;
            let provider_id = model.provider_id.clone();
            self.install_model_editor(
                ModelEditor::new(provider_id.clone(), provider_kind, Some(model), window, cx),
                window,
                cx,
            );
            self.settings_ui.form_error = None;
            self.fetch_available_models(provider_id, cx);
            cx.notify();
        }
    }

    fn install_model_editor(
        &mut self,
        editor: ModelEditor,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let remote_id = editor.remote_id.clone();
        self.settings_ui.model_editor = Some(editor);
        cx.subscribe_in(
            &remote_id,
            window,
            |this,
             _,
             event: &ComboboxEvent<crate::desktop::ui::settings::ModelIdDelegate>,
             window,
             cx| {
                let ComboboxEvent::Change(values) = event else {
                    return;
                };
                let Some(remote_id) = values.last() else {
                    return;
                };
                if let Some(editor) = &mut this.settings_ui.model_editor {
                    editor.select_model(remote_id.clone(), window, cx);
                    cx.notify();
                }
            },
        )
        .detach();
    }

    fn fetch_available_models(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let Some(provider) = self
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .cloned()
        else {
            self.settings_ui.form_error = Some("Provider not found.".into());
            cx.notify();
            return;
        };
        let Some(editor) = &mut self.settings_ui.model_editor else {
            return;
        };
        editor.begin_fetch();
        self.settings_ui.model_fetch_revision =
            self.settings_ui.model_fetch_revision.wrapping_add(1);
        let revision = self.settings_ui.model_fetch_revision;
        let (sender, receiver) = async_channel::bounded(1);
        self.services.runtime.spawn(async move {
            let result: Result<Vec<AvailableModel>, _> = providers::list_models(&provider).await;
            let _ = sender.send(result).await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                if this.settings_ui.model_fetch_revision != revision {
                    return;
                }
                let editing_id = this
                    .settings_ui
                    .model_editor
                    .as_ref()
                    .and_then(ModelEditor::editing_id)
                    .map(str::to_string);
                let configured = this
                    .data
                    .snapshot
                    .models
                    .iter()
                    .filter(|model| model.provider_id == provider_id)
                    .filter(|model| Some(model.id.as_str()) != editing_id.as_deref())
                    .map(|model| model.remote_id.clone())
                    .collect::<HashSet<_>>();
                let Some(editor) = this
                    .settings_ui
                    .model_editor
                    .as_mut()
                    .filter(|editor| editor.provider_id == provider_id)
                else {
                    return;
                };
                match result {
                    Ok(Ok(mut models)) => {
                        models.retain(|model| !configured.contains(&model.id));
                        editor.finish_fetch(models, cx);
                    }
                    Ok(Err(error)) => editor.fail_fetch(error.message),
                    Err(_) => editor.fail_fetch("Model discovery task stopped".into()),
                }
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }

    pub(crate) fn retry_available_models(&mut self, cx: &mut Context<Self>) {
        if let Some(provider_id) = self
            .settings_ui
            .model_editor
            .as_ref()
            .map(|editor| editor.provider_id.clone())
        {
            self.fetch_available_models(provider_id, cx);
        }
    }

    pub(crate) fn set_model_capability(
        &mut self,
        capability: Capability,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.set_capability(capability, enabled);
            cx.notify();
        }
    }

    pub(crate) fn save_model(&mut self, cx: &mut Context<Self>) {
        let Some(editor) = &self.settings_ui.model_editor else {
            return;
        };
        let model = match editor.build(cx) {
            Ok(model) => model,
            Err(error) => {
                self.settings_ui.form_error = Some(error);
                cx.notify();
                return;
            }
        };
        let insert = editor.is_new();
        self.settings_ui.model_editor = None;
        self.settings_ui.form_error = None;
        self.mutate_and_reload(
            move |storage| {
                if insert {
                    storage.insert_model(&model)
                } else {
                    storage.update_model(&model)
                }
            },
            cx,
        );
    }

    pub(crate) fn cancel_model_editor(&mut self, cx: &mut Context<Self>) {
        self.settings_ui.model_editor = None;
        self.settings_ui.form_error = None;
        cx.notify();
    }

    pub(crate) fn request_delete_model(
        &mut self,
        id: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.request_destructive_action(DestructiveAction::DeleteModel { id }, window, cx);
    }

    pub(super) fn delete_model(&mut self, id: String, cx: &mut Context<Self>) {
        self.mutate_and_reload(move |storage| storage.delete_model(&id), cx);
    }

    pub(crate) fn test_provider_connection(&mut self, provider_id: String, cx: &mut Context<Self>) {
        let Some(provider) = self
            .data
            .snapshot
            .providers
            .iter()
            .find(|provider| provider.id == provider_id)
            .cloned()
        else {
            return;
        };
        self.settings_ui
            .connection_tests
            .insert(provider_id.clone(), ConnectionTestStatus::Testing);
        let (sender, receiver) = async_channel::bounded(1);
        self.services.runtime.spawn(async move {
            let _ = sender
                .send(providers::test_connection(&provider).await)
                .await;
        });
        cx.spawn(async move |this, cx| {
            let result = receiver.recv().await;
            let _ = this.update(cx, |this, cx| {
                let status = match result {
                    Ok(Ok(())) => ConnectionTestStatus::Connected,
                    Ok(Err(error)) => ConnectionTestStatus::Failed(error.message),
                    Err(_) => ConnectionTestStatus::Failed("Connection task stopped".into()),
                };
                this.settings_ui
                    .connection_tests
                    .insert(provider_id, status);
                cx.notify();
            });
        })
        .detach();
        cx.notify();
    }
}

fn rounded_background_opacity(opacity: f32) -> f32 {
    let opacity = opacity.clamp(
        crate::domain::MIN_BACKGROUND_OPACITY,
        crate::domain::MAX_BACKGROUND_OPACITY,
    );
    (opacity * 100.0).round() / 100.0
}

fn rounded_message_width_ratio(ratio: f32) -> f32 {
    let ratio = ratio.clamp(
        crate::domain::MIN_MESSAGE_WIDTH_RATIO,
        crate::domain::MAX_MESSAGE_WIDTH_RATIO,
    );
    (ratio * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::{rounded_background_opacity, rounded_message_width_ratio};

    #[test]
    fn background_opacity_is_clamped_and_rounded_to_slider_step() {
        assert_eq!(rounded_background_opacity(0.734), 0.73);
        assert_eq!(rounded_background_opacity(0.736), 0.74);
        assert_eq!(rounded_background_opacity(-0.1), 0.0);
        assert_eq!(rounded_background_opacity(1.1), 1.0);
    }

    #[test]
    fn message_width_ratio_is_clamped_and_rounded_to_slider_step() {
        assert_eq!(rounded_message_width_ratio(0.734), 0.73);
        assert_eq!(rounded_message_width_ratio(0.736), 0.74);
        assert_eq!(rounded_message_width_ratio(0.1), 0.5);
        assert_eq!(rounded_message_width_ratio(1.5), 1.0);
    }
}
