use super::*;

impl OneChat {
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
        let reasoning_format = editor.reasoning.format_select.clone();
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
        cx.subscribe_in(
            &reasoning_format,
            window,
            |this, _, event: &SelectEvent<Vec<KnownReasoningFormatItem>>, window, cx| {
                let SelectEvent::Confirm(Some(format)) = event else {
                    return;
                };
                if let Some(editor) = &mut this.settings_ui.model_editor {
                    editor.reasoning.set_format(*format, window, cx);
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

    pub(crate) fn set_model_reasoning_enabled(&mut self, enabled: bool, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.set_enabled(enabled);
            cx.notify();
        }
    }

    pub(crate) fn set_model_reasoning_mode(
        &mut self,
        mode: ReasoningEditorMode,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.set_mode(mode, window, cx);
            cx.notify();
        }
    }

    pub(crate) fn toggle_known_reasoning_preset(
        &mut self,
        level: ReasoningLevel,
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.toggle_known_preset(level, enabled);
            cx.notify();
        }
    }

    pub(crate) fn set_known_reasoning_default(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.set_known_default(id);
            cx.notify();
        }
    }

    pub(crate) fn add_custom_reasoning_preset(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.add_custom_preset(window, cx);
            cx.notify();
        }
    }

    pub(crate) fn remove_custom_reasoning_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.remove_custom_preset(index);
            cx.notify();
        }
    }

    pub(crate) fn move_custom_reasoning_preset(
        &mut self,
        index: usize,
        offset: isize,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.move_custom_preset(index, offset);
            cx.notify();
        }
    }

    pub(crate) fn set_custom_reasoning_default(
        &mut self,
        index: Option<usize>,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.set_custom_default(index);
            cx.notify();
        }
    }

    pub(crate) fn add_reasoning_parameter(
        &mut self,
        preset: usize,
        scope: ReasoningParameterScope,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.add_parameter(preset, scope, window, cx);
            cx.notify();
        }
    }

    pub(crate) fn remove_reasoning_parameter(
        &mut self,
        preset: usize,
        scope: ReasoningParameterScope,
        parameter: usize,
        cx: &mut Context<Self>,
    ) {
        if let Some(editor) = &mut self.settings_ui.model_editor {
            editor.reasoning.remove_parameter(preset, scope, parameter);
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

    pub(crate) fn delete_model(&mut self, id: String, cx: &mut Context<Self>) {
        self.mutate_and_reload(move |storage| storage.delete_model(&id), cx);
    }
}
